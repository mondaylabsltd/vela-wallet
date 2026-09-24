//! Turning what Windows returned into what the core reads.
//!
//! It is four `to_hex` calls, and it lives here rather than in the shell for
//! one reason: the shell cannot be compiled for Windows on a machine without a
//! mingw toolchain, so a mapping written there could only ever be checked by a
//! Windows build. `vela-core` cross-checks cleanly, so putting the mapping
//! beside the call means every line of the Windows path is type-checked from
//! any machine.
//!
//! **These are the same structs the CTAP2 path produces.** From here down the
//! wallet cannot tell which platform a key came from, which is the property
//! that makes one authenticator derive one address everywhere.

use vela_core::app::{Assertion, Registration};
use vela_core::primitives;

use crate::{Asserted, Registered};

pub fn registration_from(registered: Registered) -> Registration {
    Registration {
        credential_id: primitives::to_hex(&registered.credential_id, false),
        attestation_object_hex: primitives::to_hex(&registered.attestation_object, false),
        client_data_json_hex: primitives::to_hex(registered.client_data_json.as_bytes(), false),
        authenticator_attachment: registered.attachment,
        transports: registered.transport,
        // Spec 075: only a key minted ON the Trusted Signer's page records that
        // page. Windows Hello is this device's own authenticator, so there is
        // no origin to remember — `None`, never an empty string, because "no
        // page" and "a page that named itself with nothing" are different facts.
        signer_origin: None,
    }
}

pub fn assertion_from(asserted: Asserted) -> Assertion {
    Assertion {
        credential_id: primitives::to_hex(&asserted.credential_id, false),
        // `signature_der_hex`, not `signature_hex`: Windows hands back DER and
        // the core normalises it (including low-S) itself. Naming it otherwise
        // would invite a shell to "helpfully" convert first.
        signature_der_hex: primitives::to_hex(&asserted.signature, false),
        authenticator_data_hex: primitives::to_hex(&asserted.authenticator_data, false),
        client_data_json_hex: primitives::to_hex(asserted.client_data_json.as_bytes(), false),
        // Absent, not empty: no user handle is a different fact from an empty
        // one, and the core's name resolution branches on it.
        user_id_hex: asserted
            .user_id
            .as_deref()
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| primitives::to_hex(bytes, false)),
        authenticator_attachment: String::new(),
        // As above: this assertion came from Windows Hello, not from a page.
        signer_origin: None,
    }
}
