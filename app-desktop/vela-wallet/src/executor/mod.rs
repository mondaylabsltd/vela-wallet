//! The only place onboarding touches the outside world.
//!
//! Each `ShellOperation` the core declares maps to exactly one call — a passkey
//! ceremony, a storage read or write, a registry request, a timer, a prompt.
//! There is no branching on business meaning here: **if this file ever grows an
//! `if` that decides what happens next, that decision belongs in the Rust
//! machine instead.**
//!
//! ## Failure contract
//!
//! Nothing propagates a failure outward. Every operation answers with the
//! result variant it owes, including for its failures — which is what lets the
//! core own classification instead of pattern-matching error strings.
//!
//! The web executor needs a second function for this (`operationFailure`),
//! because a thrown promise arrives without saying which operation it belonged
//! to. Rust has no such gap: every arm below returns a `ShellResult` on both
//! paths, so the mapping cannot go missing for an operation and cannot drift
//! from it. Adding an operation to the core stops this `match` from compiling,
//! which is the intended way to find out.
//!
//! ## Where the work happens
//!
//! [`perform`] BLOCKS: it opens a USB device, waits for a finger, does TLS.
//! Callers run it on gpui's background executor and hand the answer back to the
//! core on the main thread. Two of the eighteen operations are not performed
//! here at all — `Prompt` and `CompleteOnboarding` belong to the screen, which
//! is why [`Performed`] exists.

pub mod abi;
pub mod activity_feed;
pub mod approval_guard;
pub mod balance_dashboard;
pub mod balances;
pub mod batch;
pub mod browser_history;
pub mod camera;
pub mod chain;
pub mod chain_tokens;
pub mod chainlink;
pub mod clear_signing;
pub mod contacts;
pub mod custom_tokens;
pub mod dapp_rpc;
pub mod display_currency;
pub mod explore_sites;
pub mod fee;
pub mod format_prefs;
pub mod gpui_http;
pub mod identity;
pub mod manage_tokens;
pub mod network_admin;
pub mod passkey;
pub mod payment_request;
#[cfg(target_os = "macos")]
mod platform_macos;
pub mod pool;
pub mod proxy;
pub mod qr;
pub mod receive_watch;
pub mod registry;
pub mod relay;
pub mod send;
/// The signing panel's seven operations.
///
pub mod sign_request;
pub mod sim;
pub mod storage;
pub mod token_trust;
pub mod tracker;
pub mod user_op;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vela_core::app::FailureKind;
use vela_core::app::session::{SessionOperation, SessionShellResult};
use vela_core::app::shell::{ProofPurpose, ShellOperation, ShellResult};
use vela_core::l10n::datetime::Civil;
use vela_core::primitives;
use vela_core::registry_proof::{build_member_proof, group_public_key_from_seed};

use passkey::{Ceremony, PasskeyFailure};

/// What performing an operation produced.
pub enum Performed {
    /// An answer for the core.
    Now(Box<ShellResult>),
    /// Not this module's business. The screen shows the prompt or performs the
    /// hand-off, and answers when it has — those are the two operations whose
    /// "outside world" is the user interface itself.
    Screen,
}

/// Perform one onboarding operation. Never fails outward; see the module note.
pub fn perform(operation: &ShellOperation, ceremony: &Ceremony) -> Performed {
    let result = match operation {
        ShellOperation::CheckPasskeySupport => ShellResult::PasskeySupport {
            supported: passkey::supported(),
        },

        ShellOperation::RegisterPasskey {
            name,
            exclude_credential_ids,
            method,
        } => match passkey::register(name, exclude_credential_ids, *method, ceremony) {
            Ok(registration) => ShellResult::PasskeyRegistered {
                registration,
                now_iso: now_iso(),
            },
            Err(failure) => passkey_failed(failure),
        },

        ShellOperation::SignProof {
            credential_id,
            // This client runs the ceremony itself — a cable, or Windows Hello
            // — so it routes on its own build rather than on a transport hint.
            // Named to keep the match exhaustive and the omission deliberate.
            transports: _,
            // The route the person signed in with. A caBLE sign-in's recovery
            // second signature must go back over caBLE (pinned to the same
            // credential), not down the USB path — which would find no key and
            // show no QR.
            method,
            purpose,
        } => match passkey::assert(
            &challenge_for(*purpose),
            Some(credential_id),
            *method,
            ceremony,
        ) {
            Ok(assertion) => ShellResult::ProofSigned {
                assertion,
                now_iso: now_iso(),
            },
            Err(failure) => passkey_failed(failure),
        },

        ShellOperation::GenerateGroupKey => {
            // The one-time software group key — the only randomness in the flow
            // that is not a challenge, and it stays in the shell. The core only
            // echoes it into the final publish.
            let seed_hex = primitives::to_hex(&passkey::random(32), false);
            match group_public_key_from_seed(&seed_hex) {
                Ok(group_public_key_hex) => ShellResult::GroupKeyGenerated {
                    seed_hex,
                    group_public_key_hex,
                },
                // A seed the P-256 scalar field rejects is a CSPRNG that is not
                // one. It is reported as a storage failure because that is the
                // variant this operation owes; the message says what happened.
                Err(error) => ShellResult::StorageFailed {
                    message: format!("could not derive the group key: {error}"),
                },
            }
        }

        ShellOperation::SignMemberProof {
            credential_id,
            public_key_hex,
            attestation_hex,
            // `transports` steers a platform that routes for itself; this shell
            // is its own CTAP client, so it routes on `method` instead.
            transports: _,
            method,
            group_public_key_hex,
        } => {
            // Mixed failure modes: the challenge fetch and the ceremony can each
            // fail, and the core branches differently on the two. Classify by
            // what actually failed rather than by which operation it was.
            match registry::member_challenge(group_public_key_hex, public_key_hex, attestation_hex)
            {
                Err(error) => index_failed(error),
                Ok(challenge) => match primitives::from_hex(&challenge) {
                    Err(error) => ShellResult::IndexFailed {
                        message: format!("the registry challenge is not hex: {error}"),
                        network: false,
                    },
                    Ok(bytes) => {
                        match passkey::assert(&bytes, Some(credential_id), *method, ceremony) {
                            Err(failure) => passkey_failed(failure),
                            Ok(assertion) => match build_member_proof(
                                &assertion.authenticator_data_hex,
                                &assertion.client_data_json_hex,
                                &assertion.signature_der_hex,
                            ) {
                                Ok(proof) => ShellResult::MemberProofSigned { proof },
                                Err(error) => ShellResult::IndexFailed {
                                    message: format!(
                                        "could not assemble the member proof: {error}"
                                    ),
                                    network: false,
                                },
                            },
                        }
                    }
                },
            }
        }

        ShellOperation::LookupLegacyName { credential_id } => ShellResult::LegacyName {
            name: registry::legacy_name(credential_id),
        },

        // The method is the person's sign-in choice, and now it routes: the scan
        // method signs in through a phone over caBLE, every other method through
        // the plugged-in security key. `passkey::assert` owns the branch.
        ShellOperation::AuthenticatePasskey { method } => {
            match passkey::assert(&passkey::random(32), None, *method, ceremony) {
                Ok(assertion) => ShellResult::PasskeyAuthenticated {
                    assertion,
                    now_iso: now_iso(),
                },
                Err(failure) => passkey_failed(failure),
            }
        }

        ShellOperation::LoadAccounts => match storage::load_accounts() {
            Ok(accounts) => ShellResult::AccountsLoaded { accounts },
            Err(error) => ShellResult::StorageFailed {
                message: error.to_string(),
            },
        },

        ShellOperation::SaveAccount { account } => match storage::save_account(account) {
            Ok(()) => ShellResult::AccountSaved,
            Err(error) => ShellResult::StorageFailed {
                message: error.to_string(),
            },
        },

        ShellOperation::SavePendingUpload { record } => {
            match storage::save_pending_upload(record) {
                Ok(()) => ShellResult::PendingUploadSaved,
                Err(error) => ShellResult::StorageFailed {
                    message: error.to_string(),
                },
            }
        }

        ShellOperation::RemovePendingUpload { credential_id } => {
            match storage::remove_pending_upload(credential_id) {
                Ok(()) => ShellResult::PendingUploadRemoved,
                Err(error) => ShellResult::StorageFailed {
                    message: error.to_string(),
                },
            }
        }

        ShellOperation::RegistryPublish {
            metadata_hex,
            members,
            group_seed_hex,
            group_public_key_hex,
            method,
        } => match registry::publish(
            metadata_hex,
            members,
            group_seed_hex,
            group_public_key_hex,
            *method,
            ceremony,
        ) {
            Ok(()) => ShellResult::RegistryPublished,
            Err(error) => index_failed(error),
        },

        ShellOperation::RegistryQueryByPublicKey { public_key_hex } => {
            match registry::query_by_public_key(public_key_hex) {
                Ok(status) => ShellResult::RegistryKeyStatus {
                    registered: status.registered,
                    unit_ids: status.unit_ids,
                },
                Err(error) => index_failed(error),
            }
        }

        ShellOperation::RegistryQueryUnit { unit_id } => match registry::query_unit(*unit_id) {
            Ok(unit) => ShellResult::RegistryUnit {
                metadata_hex: unit.metadata_hex,
                members: unit.members,
            },
            Err(error) => index_failed(error),
        },

        ShellOperation::ProbeIndexHealth => match registry::probe_health() {
            registry::Probe::Reachable => ShellResult::IndexHealth { ok: true },
            registry::Probe::Down => ShellResult::IndexHealth { ok: false },
            // Every route out of this machine refused: the person's network,
            // not our service, and the screen says so (spec 038).
            registry::Probe::Local => ShellResult::IndexTransportFailed,
        },

        ShellOperation::Wait { ms } => {
            // `wait` is the core's only clock, and it is a real sleep on this
            // task's own background thread. It is not cancellable and does not
            // need to be: the machines keep one operation in flight per
            // pipeline and drop a superseded answer by attempt, so a timer that
            // fires after the screen moved on is discarded on arrival (see
            // `core_host.rs`).
            std::thread::sleep(Duration::from_millis(u64::from(*ms)));
            ShellResult::Waited
        }

        // The two the screen owns.
        ShellOperation::Prompt { .. } | ShellOperation::CompleteOnboarding { .. } => {
            return Performed::Screen;
        }
    };
    Performed::Now(Box::new(result))
}

/// Perform one session operation.
///
/// Five of the seven are best effort by contract: the session is already in the
/// state the write was meant to record, and a failed write cannot put it back.
/// That is why so many arms here discard their error — deliberately, and only
/// where the contract says the shell swallows it.
pub fn perform_session(operation: &SessionOperation) -> SessionShellResult {
    match operation {
        SessionOperation::LoadAccounts => match storage::load_accounts() {
            Ok(accounts) => SessionShellResult::AccountsLoaded { accounts },
            Err(_) => SessionShellResult::AccountsUnavailable,
        },
        SessionOperation::LoadActiveIndex => SessionShellResult::ActiveIndexLoaded {
            index: storage::load_active_index(),
        },
        SessionOperation::SaveAccount { account } => {
            let _ = storage::save_account(account);
            SessionShellResult::AccountSaved
        }
        SessionOperation::SaveActiveIndex { index } => {
            let _ = storage::save_active_index(*index);
            SessionShellResult::ActiveIndexSaved
        }
        SessionOperation::CheckPendingUploads => match storage::has_pending_uploads() {
            Ok(has_pending) => SessionShellResult::PendingUploads { has_pending },
            // Fail closed: the sign-out dialog simply does not open, so no
            // unwarned logout path appears.
            Err(_) => SessionShellResult::PendingUploadsUnavailable,
        },
        SessionOperation::ClearSignedInWallet => {
            let _ = storage::clear_signed_in_wallet();
            SessionShellResult::SignedInWalletCleared
        }
        SessionOperation::ClearExtensionCache => {
            // A no-op wherever no extension exists, which is every desktop: the
            // Safari extension's account snapshot is an iOS artifact. Answered
            // rather than skipped, because the core is waiting for the ack.
            SessionShellResult::ExtensionCacheCleared
        }
    }
}

// ---------------------------------------------------------------------------
// The small conversions
// ---------------------------------------------------------------------------

fn passkey_failed(failure: PasskeyFailure) -> ShellResult {
    ShellResult::PasskeyFailed {
        kind: failure.kind,
        // A classified failure's copy comes from the classification; only
        // `other` — and the desktop's own `not_supported`, which has to be able
        // to say "plug in your key" — carries words, and those are forwarded
        // verbatim because they go into the bug report.
        message: match failure.kind {
            FailureKind::Other | FailureKind::NotSupported | FailureKind::NotDiscoverable => {
                failure.message
            }
            FailureKind::Cancelled => None,
        },
    }
}

fn index_failed(error: registry::RegistryError) -> ShellResult {
    // The one place every registry failure funnels through; without this line a
    // publish that fails after three signatures vanishes without a trace (the
    // recovery flow deliberately enters the wallet anyway).
    eprintln!(
        "[vela-registry] index operation failed (network={}, local={}): {}",
        error.network, error.local, error.message
    );
    ShellResult::IndexFailed {
        message: error.message,
        network: error.network,
    }
}

/// The challenge a proof purpose signs over.
///
/// The label strings are preserved verbatim from the shipping clients — they
/// are part of the wire, not decoration. The two recovery purposes share a
/// label on purpose: what must differ between the two signatures is the
/// challenge BYTES, and the millisecond tail supplies that. The invariant is
/// not trusted to the shell either way — `recover_public_key_from_assertions`
/// returns nothing unless the two assertions pin down exactly one key, so a
/// repeated challenge fails closed in the core.
fn challenge_for(purpose: ProofPurpose) -> Vec<u8> {
    let label = match purpose {
        ProofPurpose::Verify => "vela-verify-",
        ProofPurpose::RecoverFirst | ProofPurpose::RecoverSecond => "vela-recover-",
    };
    format!("{label}{}", unix_millis()).into_bytes()
}

/// The device's UTC offset, in seconds, right now.
///
/// The core hands the shell every date decision that depends on where the
/// machine is — `day_start_ms` is its words: "computed by the shell, which owns
/// the device timezone". `vela-core` deliberately ships **no timezone
/// database**, so this is the one fact it cannot derive and must be told.
///
/// `localtime_r` rather than a crate: the offset must include daylight saving
/// *as of this instant*, which a fixed offset read once at startup would get
/// wrong twice a year.
#[cfg(unix)]
fn local_utc_offset_seconds() -> i64 {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let time = unix_millis() / 1000;
    let time: libc::time_t = time;
    // SAFETY: `localtime_r` writes into a `tm` this call owns and reads a
    // `time_t` it owns. It is the reentrant form precisely so it needs no
    // global state and is safe to call from any thread.
    let filled = unsafe { libc::localtime_r(&raw const time, &raw mut tm) };
    if filled.is_null() {
        return 0;
    }
    tm.tm_gmtoff
}

/// Windows has no `localtime_r`; `GetTimeZoneInformation` is the equivalent.
///
/// Until spec 032 phase 11 this returned `0`, so the activity feed grouped by
/// UTC day on Windows and a late-evening transaction filed under tomorrow —
/// somebody's feed reading wrong for part of every day.
///
/// **This call cannot be compiled here.** The app's dependency tree builds C
/// (ThorVG, resvg, hidapi), so `--target x86_64-pc-windows-gnu` dies in a
/// build script long before it reaches Rust — the reason `check-windows.sh`
/// checks a separate crate. It was verified by lifting exactly these lines
/// into an isolated crate and cross-checking them, which is how the missing
/// `TIME_ZONE_ID_UNKNOWN` export was found. The ARITHMETIC is a separate,
/// unsafe-free function below with tests that run on every platform.
// `windows` rather than `not(unix)`: the body needs `windows-sys`, which is a
// dependency only under that same cfg. A target that is neither would now fail
// to find this function at all — louder, and better, than silently grouping a
// third platform by UTC.
#[cfg(windows)]
fn local_utc_offset_seconds() -> i64 {
    use windows_sys::Win32::System::Time::{
        GetTimeZoneInformation, TIME_ZONE_ID_INVALID, TIME_ZONE_INFORMATION,
    };
    // windows-sys 0.59 exports only this one of the four ids; the other three
    // are matched by value in `windows_offset_seconds`. Pin what is exported,
    // so a version that renumbered them would fail to build rather than
    // quietly shift everyone's day boundary.
    const _: () = assert!(TIME_ZONE_ID_INVALID == u32::MAX);

    let mut info: TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: `GetTimeZoneInformation` fills a struct this call owns and reads
    // no global state of ours. A failure returns TIME_ZONE_ID_INVALID and
    // leaves the struct meaningless, which the arithmetic below refuses rather
    // than computes.
    let id = unsafe { GetTimeZoneInformation(&raw mut info) };
    windows_offset_seconds(id, info.Bias, info.StandardBias, info.DaylightBias)
}

/// Win32's three biases as one UTC offset in seconds.
///
/// The sign is the trap: Win32 defines **UTC = local + bias**, so the offset a
/// person east of Greenwich lives at is the NEGATIVE of their bias. Berlin's
/// winter bias is −60 and its offset is +3600.
///
/// Which seasonal bias applies is the id's to say, and the two must not be
/// crossed: adding `StandardBias` while the zone is on daylight time is an
/// hour's error in the direction that looks plausible.
///
/// Split out and free of `unsafe` on purpose — this half is compiled and
/// tested on every platform, so the part of the Windows path that can be
/// wrong arithmetically is the part that is not Windows-only.
#[cfg(any(windows, test))]
fn windows_offset_seconds(id: u32, bias: i32, standard_bias: i32, daylight_bias: i32) -> i64 {
    let seasonal = match id {
        // TIME_ZONE_ID_STANDARD
        1 => standard_bias,
        // TIME_ZONE_ID_DAYLIGHT
        2 => daylight_bias,
        // TIME_ZONE_ID_UNKNOWN — the zone has no seasonal rule at all.
        0 => 0,
        // TIME_ZONE_ID_INVALID, or anything undocumented: the struct was never
        // filled, so every field is garbage. Fall back to UTC rather than
        // compute a day boundary out of it.
        _ => return 0,
    };
    -(i64::from(bias) + i64::from(seasonal)) * 60
}

/// Local midnight for an instant, as epoch milliseconds.
///
/// The activity feed groups by DAY, and a day is a local idea. Computing this
/// in UTC would put a 20:00 transaction in Tokyo under tomorrow's heading for
/// anybody east of Greenwich, and under yesterday's for anybody west — visibly
/// wrong for part of every day rather than subtly wrong all of it.
pub fn day_start_ms(timestamp_ms: f64) -> f64 {
    const DAY_MS: f64 = 86_400_000.0;
    #[allow(clippy::cast_precision_loss, reason = "an offset is at most 14 hours")]
    let offset_ms = (local_utc_offset_seconds() * 1000) as f64;
    let local = timestamp_ms + offset_ms;
    (local / DAY_MS).floor() * DAY_MS - offset_ms
}

/// An epoch stamp as the wall clock the person is actually reading.
///
/// The offset comes from the same `localtime_r` [`day_start_ms`] uses, so a
/// timestamp and the day heading it files under can never disagree about which
/// zone this machine is in — and it includes daylight saving as of that
/// instant, which a value read once at startup would be wrong about twice a
/// year.
#[must_use]
pub fn local_civil(epoch_ms: f64) -> Civil {
    #[allow(clippy::cast_possible_truncation, reason = "an epoch in milliseconds")]
    let ms = epoch_ms as i64;
    let offset_minutes = i32::try_from(local_utc_offset_seconds() / 60).unwrap_or(0);
    Civil::from_unix_millis(ms, offset_minutes)
}

/// The same wall clock, as epoch milliseconds — what the wallet-state machines
/// stamp their mutations with. Public since spec 030: an event carries the time
/// the SHELL observed, so the core stays a pure function of its inputs.
pub fn now_ms() -> f64 {
    unix_millis() as f64
}

fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// The wall clock this shell observed, as the core's clients all spell it.
///
/// It travels WITH the observation rather than being asked for separately,
/// which is what keeps the core a pure function of its inputs: no clock effect,
/// and no clock in any core test. UTC, because a stored `created_at_iso` that
/// carries a local offset is a record that means something different when the
/// laptop moves.
pub fn now_iso() -> String {
    let civil = Civil::from_unix_millis(unix_millis(), 0);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        civil.year,
        civil.month,
        civil.day,
        civil.hour,
        civil.minute,
        civil.second,
        unix_millis().rem_euclid(1_000)
    )
}

#[cfg(test)]
mod timezone_tests {
    use super::windows_offset_seconds;

    /// Win32 defines UTC = local + bias, so the offset is the bias NEGATED.
    /// Getting this backwards puts Berlin at UTC−1 and New York at UTC+5,
    /// which is the whole day-boundary bug in the other direction.
    #[test]
    fn the_offset_is_the_bias_negated() {
        // Berlin in winter: bias −60, on standard time.
        assert_eq!(windows_offset_seconds(1, -60, 0, -60), 3_600);
        // New York in winter: bias 300, on standard time.
        assert_eq!(windows_offset_seconds(1, 300, 0, -60), -18_000);
    }

    /// Daylight time takes the DAYLIGHT bias. Reaching for the standard one
    /// while the zone is on summer time is an hour out, in the direction that
    /// looks entirely plausible on screen.
    #[test]
    fn summer_time_uses_its_own_bias() {
        // Berlin in summer: −60 base, −60 more for daylight ⇒ UTC+2.
        assert_eq!(windows_offset_seconds(2, -60, 0, -60), 7_200);
        // New York in summer ⇒ UTC−4, not UTC−5.
        assert_eq!(windows_offset_seconds(2, 300, 0, -60), -14_400);
    }

    /// A zone with no seasonal rule reports UNKNOWN and carries only a bias.
    /// India is the half-hour case, which a whole-hour assumption would lose.
    #[test]
    fn a_zone_without_a_season_uses_its_bias_alone() {
        assert_eq!(windows_offset_seconds(0, -330, 0, 0), 19_800);
        // Even if the struct carries seasonal biases, UNKNOWN ignores them.
        assert_eq!(windows_offset_seconds(0, -60, -30, -90), 3_600);
    }

    /// The call FAILED and the struct is meaningless. Computing a day boundary
    /// out of uninitialised fields would be worse than the UTC grouping this
    /// replaces, because it would be wrong unpredictably rather than
    /// consistently.
    #[test]
    fn a_failed_call_falls_back_to_utc_rather_than_to_garbage() {
        assert_eq!(windows_offset_seconds(u32::MAX, 999, 999, 999), 0);
        assert_eq!(windows_offset_seconds(7, -60, 0, -60), 0);
    }
}
