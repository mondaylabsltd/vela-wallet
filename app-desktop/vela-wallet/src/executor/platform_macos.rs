//! The macOS platform authenticator — "This device" — through
//! AuthenticationServices: Touch ID / iCloud Keychain passkeys, the same vault
//! Safari uses, reached with `ASAuthorizationPlatformPublicKeyCredentialProvider`.
//!
//! ## What this needs from the bundle
//!
//! The system only serves an app whose signature carries the
//! `com.apple.developer.associated-domains` entitlement naming
//! `webcredentials:getvela.app`, AND whose appID the domain's
//! `apple-app-site-association` lists. The AASA already lists
//! `F9W689P9NE.app.getvela.VelaWallet` — the same bundle id this app ships —
//! so the remaining gate is signing with the real team identity
//! (`VELA_SIGN_IDENTITY` in `build-macos-app.sh`); the ad-hoc dev signature
//! makes every request here fail with the system's own sentence saying the app
//! is not associated with the domain. That failure is surfaced verbatim: it is
//! the actionable truth.
//!
//! ## Threading
//!
//! `ASAuthorizationController` is main-thread machinery: it presents system UI
//! anchored to our NSWindow and calls its delegate back on the main queue. The
//! ceremony runs on a background thread, so each operation is marshalled onto
//! the main queue with `dispatch_async_f`, and the result comes back over an
//! mpsc channel the ceremony thread blocks on. The controller and delegate are
//! deliberately leaked at `performRequests` and reclaimed in the completion
//! callback — the system holds no reference for us, and dropping them early
//! cancels the sheet.

#![allow(unexpected_cfgs)]

use std::ffi::c_void;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability};
use objc2_app_kit::NSApplication;
use objc2_authentication_services::{
    ASAuthorization,
    ASAuthorizationController,
    ASAuthorizationControllerDelegate,
    ASAuthorizationControllerPresentationContextProviding,
    ASAuthorizationPlatformPublicKeyCredentialAssertion,
    ASAuthorizationPlatformPublicKeyCredentialDescriptor,
    ASAuthorizationPlatformPublicKeyCredentialProvider,
    ASAuthorizationPlatformPublicKeyCredentialRegistration,
    // The WebAuthn artefact accessors live on the PROTOCOL traits, not the
    // classes — imported anonymously so their methods resolve.
    ASAuthorizationPublicKeyCredentialAssertion as _,
    ASAuthorizationPublicKeyCredentialRegistration as _,
    ASAuthorizationRequest,
    ASPresentationAnchor,
    ASPublicKeyCredential as _,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSError, NSObject, NSObjectProtocol,
    NSOperatingSystemVersion, NSProcessInfo, NSString,
};

use vela_core::app::{Assertion, Registration};
use vela_core::primitives;

use super::passkey::{PasskeyFailure, RELYING_PARTY};

/// How long the ceremony thread waits for the person to finish with the system
/// sheet — Touch ID, an iCloud approval, or a cancel.
const CEREMONY_TIMEOUT: Duration = Duration::from_secs(300);

const ATTACHMENT_PLATFORM: &str = "platform";
/// What Safari's `getTransports()` reports for an iCloud-keychain passkey: it
/// lives on this device and syncs, so it is reachable both ways.
const TRANSPORTS_PLATFORM: &str = "internal,hybrid";

/// Is the platform-passkey API present at all? The provider class shipped in
/// macOS 12; earlier systems have no "This device" to offer. Whether the
/// SIGNATURE satisfies the domain association is deliberately not probed here —
/// there is no side-effect-free way to ask, and the ceremony's own error names
/// the fix better than a greyed row could.
pub fn available() -> bool {
    let version = NSOperatingSystemVersion {
        majorVersion: 12,
        minorVersion: 0,
        patchVersion: 0,
    };
    unsafe { NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version) }
}

/// What one completed ceremony hands back across the thread boundary — plain
/// bytes only, because the objc objects must die on the main thread.
enum Outcome {
    Registered {
        credential_id: Vec<u8>,
        attestation_object: Vec<u8>,
        client_data_json: Vec<u8>,
    },
    Asserted {
        credential_id: Vec<u8>,
        authenticator_data: Vec<u8>,
        signature: Vec<u8>,
        client_data_json: Vec<u8>,
        user_id: Option<Vec<u8>>,
    },
}

/// One request marshalled onto the main queue.
struct Job {
    kind: JobKind,
    tx: Sender<Result<Outcome, PasskeyFailure>>,
}

enum JobKind {
    Register {
        name: String,
        user_id: Vec<u8>,
        challenge: Vec<u8>,
    },
    Assert {
        challenge: Vec<u8>,
        credential_id: Option<Vec<u8>>,
    },
}

/// `RegisterPasskey` on the platform vault. `user_id` is the `name‖NUL‖uuid`
/// handle the whole product mints (it is where the wallet's name survives a
/// device wipe), and the challenge is fresh randomness — attestation is "none"
/// on this path, exactly like the browser flow.
pub fn register(
    name: &str,
    user_id: Vec<u8>,
    challenge: Vec<u8>,
) -> Result<Registration, PasskeyFailure> {
    let outcome = run(JobKind::Register {
        name: name.to_owned(),
        user_id,
        challenge,
    })?;
    match outcome {
        Outcome::Registered {
            credential_id,
            attestation_object,
            client_data_json,
        } => Ok(Registration {
            credential_id: primitives::to_hex(&credential_id, false),
            attestation_object_hex: primitives::to_hex(&attestation_object, false),
            client_data_json_hex: primitives::to_hex(&client_data_json, false),
            authenticator_attachment: ATTACHMENT_PLATFORM.to_owned(),
            transports: TRANSPORTS_PLATFORM.to_owned(),
        }),
        Outcome::Asserted { .. } => Err(PasskeyFailure::other(
            "the platform authenticator answered the wrong ceremony",
        )),
    }
}

/// One assertion on the platform vault. `credential_id` pins the allow list
/// (a proof, recovery's second signature); `None` asks for any discoverable
/// credential (sign-in).
pub fn assert(challenge: &[u8], credential_id: Option<&str>) -> Result<Assertion, PasskeyFailure> {
    let pinned = credential_id.and_then(|id| primitives::from_hex(id).ok());
    let outcome = run(JobKind::Assert {
        challenge: challenge.to_vec(),
        credential_id: pinned,
    })?;
    match outcome {
        Outcome::Asserted {
            credential_id,
            authenticator_data,
            signature,
            client_data_json,
            user_id,
        } => Ok(Assertion {
            credential_id: primitives::to_hex(&credential_id, false),
            signature_der_hex: primitives::to_hex(&signature, false),
            authenticator_data_hex: primitives::to_hex(&authenticator_data, false),
            client_data_json_hex: primitives::to_hex(&client_data_json, false),
            // Absent, not empty: the core's name resolution branches on it.
            user_id_hex: user_id
                .filter(|bytes| !bytes.is_empty())
                .map(|bytes| primitives::to_hex(&bytes, false)),
            authenticator_attachment: ATTACHMENT_PLATFORM.to_owned(),
        }),
        Outcome::Registered { .. } => Err(PasskeyFailure::other(
            "the platform authenticator answered the wrong ceremony",
        )),
    }
}

/// Marshal one job onto the main queue and wait for its outcome.
fn run(kind: JobKind) -> Result<Outcome, PasskeyFailure> {
    let (tx, rx) = channel();
    let job = Box::new(Job { kind, tx });
    unsafe {
        dispatch_async_f(
            std::ptr::addr_of!(_dispatch_main_q).cast(),
            Box::into_raw(job).cast(),
            perform_on_main,
        );
    }
    match rx.recv_timeout(CEREMONY_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(PasskeyFailure::other(
            "the passkey sheet did not answer in time",
        )),
    }
}

// The libdispatch C surface — stable ABI, no crate needed.
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

/// Runs on the main queue: build the request, the controller and the delegate,
/// present, and leak the pair until the delegate callback reclaims them.
extern "C" fn perform_on_main(context: *mut c_void) {
    // Safety: the pointer is the Box leaked in `run`, delivered exactly once.
    let job = unsafe { Box::from_raw(context.cast::<Job>()) };
    let Some(mtm) = MainThreadMarker::new() else {
        let _ = job.tx.send(Err(PasskeyFailure::other(
            "the passkey sheet must run on the main thread",
        )));
        return;
    };

    let provider: Retained<ASAuthorizationPlatformPublicKeyCredentialProvider> = unsafe {
        msg_send_id![
            ASAuthorizationPlatformPublicKeyCredentialProvider::alloc(),
            initWithRelyingPartyIdentifier: &*NSString::from_str(RELYING_PARTY)
        ]
    };

    let request: Retained<ASAuthorizationRequest> = match &job.kind {
        JobKind::Register {
            name,
            user_id,
            challenge,
        } => {
            let request: Retained<AnyObject> = unsafe {
                msg_send_id![
                    &provider,
                    createCredentialRegistrationRequestWithChallenge: &*NSData::with_bytes(challenge),
                    name: &*NSString::from_str(name),
                    userID: &*NSData::with_bytes(user_id)
                ]
            };
            // Safety: the provider returns an ASAuthorizationRequest subclass.
            unsafe { Retained::cast(request) }
        }
        JobKind::Assert {
            challenge,
            credential_id,
        } => {
            let request: Retained<AnyObject> = unsafe {
                msg_send_id![
                    &provider,
                    createCredentialAssertionRequestWithChallenge: &*NSData::with_bytes(challenge)
                ]
            };
            if let Some(id) = credential_id {
                let descriptor: Retained<ASAuthorizationPlatformPublicKeyCredentialDescriptor> = unsafe {
                    msg_send_id![
                        ASAuthorizationPlatformPublicKeyCredentialDescriptor::alloc(),
                        initWithCredentialID: &*NSData::with_bytes(id)
                    ]
                };
                let allowed = NSArray::from_slice(&[&*descriptor]);
                let _: () =
                    unsafe { objc2::msg_send![&*request, setAllowedCredentials: &*allowed] };
            }
            // Safety: as above.
            unsafe { Retained::cast(request) }
        }
    };

    let delegate = Delegate::new(mtm, job.tx.clone());
    let requests = NSArray::from_slice(&[&*request]);
    let controller: Retained<ASAuthorizationController> = unsafe {
        msg_send_id![
            ASAuthorizationController::alloc(),
            initWithAuthorizationRequests: &*requests
        ]
    };
    let delegate_proto: &ProtocolObject<dyn ASAuthorizationControllerDelegate> =
        ProtocolObject::from_ref(&*delegate);
    let anchor_proto: &ProtocolObject<dyn ASAuthorizationControllerPresentationContextProviding> =
        ProtocolObject::from_ref(&*delegate);
    unsafe {
        controller.setDelegate(Some(delegate_proto));
        controller.setPresentationContextProvider(Some(anchor_proto));
        controller.performRequests();
    }

    // The system keeps no strong reference to either; the delegate's completion
    // callback rebuilds and drops these two retains (see `reclaim`).
    let controller_ptr = Retained::into_raw(controller);
    let delegate_ptr = Retained::into_raw(delegate);
    if let Ok(mut slot) = delegate_holder().lock() {
        slot.push(Held {
            controller: controller_ptr.cast(),
            delegate: delegate_ptr.cast(),
        });
    }
}

/// One leaked (controller, delegate) pair awaiting its completion callback.
/// Raw pointers because `Retained` is not `Send`; the pointers are only ever
/// created and consumed on the main thread — the Mutex is there for the static,
/// not for real cross-thread traffic.
struct Held {
    controller: *mut c_void,
    delegate: *mut c_void,
}
// Safety: see above — main-thread-only traffic behind a static.
unsafe impl Send for Held {}

fn delegate_holder() -> &'static std::sync::Mutex<Vec<Held>> {
    static HOLDER: std::sync::OnceLock<std::sync::Mutex<Vec<Held>>> = std::sync::OnceLock::new();
    HOLDER.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Drop the retains `perform_on_main` leaked for this delegate. Main thread
/// only (the delegate callbacks run there).
fn reclaim(delegate: *const c_void) {
    let Ok(mut slot) = delegate_holder().lock() else {
        return;
    };
    if let Some(index) = slot
        .iter()
        .position(|held| std::ptr::eq(held.delegate, delegate.cast_mut()))
    {
        let held = slot.swap_remove(index);
        // Safety: exactly the two pointers `Retained::into_raw` produced.
        unsafe {
            let _ = Retained::<ASAuthorizationController>::from_raw(held.controller.cast());
            let _ = Retained::<Delegate>::from_raw(held.delegate.cast());
        }
    }
}

/// What the delegate needs: where to send the outcome.
struct Ivars {
    tx: Sender<Result<Outcome, PasskeyFailure>>,
}

declare_class!(
    struct Delegate;

    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "VelaPlatformPasskeyDelegate";
    }

    impl DeclaredClass for Delegate {
        type Ivars = Ivars;
    }

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl ASAuthorizationControllerDelegate for Delegate {
        #[method(authorizationController:didCompleteWithAuthorization:)]
        fn did_complete(
            &self,
            _controller: &ASAuthorizationController,
            authorization: &ASAuthorization,
        ) {
            let result = extract(authorization);
            let _ = self.ivars().tx.send(result);
            reclaim((self as *const Self).cast());
        }

        #[method(authorizationController:didCompleteWithError:)]
        fn did_fail(&self, _controller: &ASAuthorizationController, error: &NSError) {
            // ASAuthorizationErrorCanceled = 1001: the person closed the sheet.
            let failure = if error.code() == 1001 {
                PasskeyFailure::cancelled()
            } else {
                {
                    let description = error.localizedDescription().to_string();
                    // ASAuthorization refuses a process with no application
                    // identifier — a bare `cargo run` binary, not a signed
                    // bundle. That is not a missing fingerprint, and the sheet
                    // must not say it is (spec 038 finding 9). The fix is the
                    // README's: the parallel space for development, or the
                    // signed bundle from scripts/build-macos-app.sh.
                    if description.contains("does not have an application identifier") {
                        PasskeyFailure::classified(
                            vela_core::app::FailureKind::NotSupported,
                            format!(
                                "This binary is not a signed app bundle, so macOS will not hand it \
                                 the platform authenticator. Run the app from the bundle \
                                 (scripts/build-macos-app.sh) or use the parallel space \
                                 (VELA_PARALLEL_SPACE=1). {description}"
                            ),
                        )
                    } else {
                        PasskeyFailure::other(description)
                    }
                }
            };
            let _ = self.ivars().tx.send(Err(failure));
            reclaim((self as *const Self).cast());
        }
    }

    unsafe impl ASAuthorizationControllerPresentationContextProviding for Delegate {
        #[method_id(presentationAnchorForAuthorizationController:)]
        fn anchor(&self, _controller: &ASAuthorizationController) -> Retained<ASPresentationAnchor> {
            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);
            // The app has exactly one window; key beats main only when they
            // differ (a sheet already up), and either anchors correctly. The
            // crate aliases `ASPresentationAnchor` to NSObject (it does not
            // link AppKit itself), so the NSWindow is upcast for the return.
            let window = unsafe { app.keyWindow().or_else(|| app.mainWindow()) }
                .expect("the app has a window while onboarding is on screen");
            unsafe { Retained::cast(window) }
        }
    }
);

impl Delegate {
    fn new(mtm: MainThreadMarker, tx: Sender<Result<Outcome, PasskeyFailure>>) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(Ivars { tx });
        unsafe { msg_send_id![super(this), init] }
    }
}

/// Pull the raw WebAuthn artefacts out of a completed authorization.
fn extract(authorization: &ASAuthorization) -> Result<Outcome, PasskeyFailure> {
    let credential: Retained<AnyObject> = unsafe { msg_send_id![authorization, credential] };

    let is_registration: bool = unsafe {
        objc2::msg_send![
            &*credential,
            isKindOfClass: ASAuthorizationPlatformPublicKeyCredentialRegistration::class()
        ]
    };
    if is_registration {
        let registration: &ASAuthorizationPlatformPublicKeyCredentialRegistration =
            unsafe { &*(&*credential as *const AnyObject).cast() };
        let attestation = unsafe { registration.rawAttestationObject() }.ok_or_else(|| {
            PasskeyFailure::other("the platform vault returned no attestation object")
        })?;
        return Ok(Outcome::Registered {
            credential_id: unsafe { registration.credentialID() }.bytes().to_vec(),
            attestation_object: attestation.bytes().to_vec(),
            client_data_json: unsafe { registration.rawClientDataJSON() }.bytes().to_vec(),
        });
    }

    let is_assertion: bool = unsafe {
        objc2::msg_send![
            &*credential,
            isKindOfClass: ASAuthorizationPlatformPublicKeyCredentialAssertion::class()
        ]
    };
    if is_assertion {
        let assertion: &ASAuthorizationPlatformPublicKeyCredentialAssertion =
            unsafe { &*(&*credential as *const AnyObject).cast() };
        return Ok(Outcome::Asserted {
            credential_id: unsafe { assertion.credentialID() }.bytes().to_vec(),
            authenticator_data: unsafe { assertion.rawAuthenticatorData() }.bytes().to_vec(),
            signature: unsafe { assertion.signature() }.bytes().to_vec(),
            client_data_json: unsafe { assertion.rawClientDataJSON() }.bytes().to_vec(),
            user_id: Some(unsafe { assertion.userID() }.bytes().to_vec()),
        });
    }

    Err(PasskeyFailure::other(
        "the platform vault answered with an unexpected credential type",
    ))
}
