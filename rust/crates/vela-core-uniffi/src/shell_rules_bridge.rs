//! Small rules every shell asks at the edges of a screen (perf lanes,
//! core-perf-1): when the launch animation plays, and the ENS nodes a name
//! lookup starts from. The wasm twins are `vela-core-wasm/src/shell_rules.rs`.

use crate::CoreError;

/// Does the launch animation play on this cold start? `stored_last_played_ms`:
/// what is stored under [`launch_animation_storage_key`] (`None` when nothing,
/// or nothing readable, is); `disabled`: turned off, or a start that never
/// shows it. Write `now_ms` under the key when it plays.
#[uniffi::export]
pub fn launch_animation_due(
    stored_last_played_ms: Option<f64>,
    now_ms: f64,
    disabled: bool,
) -> bool {
    vela_core::launch::launch_animation_due(stored_last_played_ms, now_ms, disabled)
}

/// Where the time the launch animation last played is stored (epoch ms).
#[uniffi::export]
pub fn launch_animation_storage_key() -> String {
    vela_core::launch::LAUNCH_ANIMATION_STORAGE_KEY.to_owned()
}

/// EIP-137 namehash of `name`, `0x`-prefixed lower-case hex.
#[uniffi::export]
pub fn ens_namehash(name: String) -> String {
    vela_core::ens::ens_namehash(&name)
}

/// The reverse node of `address` (`<addr>.addr.reverse`), `0x`-prefixed.
#[uniffi::export]
pub fn ens_reverse_node(address: String) -> Result<String, CoreError> {
    Ok(vela_core::ens::ens_reverse_node(&address)?)
}

/// r9-17: the networks an account's incoming scan reads, and where its
/// native balance is watched — `incoming_scan::scan_plan`. `facts_json` is a
/// `TrustScanFacts`; the answer an `IncomingScanPlan`, both JSON. The scan
/// itself plans this from the same facts (`token_trust`'s `ScanRequested`);
/// a shell asks only to prime the registry and the native watch.
#[uniffi::export]
#[must_use]
pub fn incoming_scan_plan(facts_json: String) -> String {
    vela_core::app::incoming_scan::scan_plan_json(&facts_json)
}

/// r9-17: one poll's reads on one network — `incoming_scan::scan_windows`
/// (`IncomingScanWindowsAsk` JSON in, `IncomingScanWindows` JSON out). For
/// the shared vectors (`tests/vectors/incoming-scan.json`); the scan runs
/// them inside `token_trust`.
#[uniffi::export]
#[must_use]
pub fn incoming_scan_windows(ask_json: String) -> String {
    vela_core::app::incoming_scan::scan_windows_json(&ask_json)
}

/// r9-17 / r10: the wait before the next incoming scan — shown or not, and
/// how many scans in a row found nothing (the back-off's step) —
/// `incoming_scan::next_scan_ms`. The feed's `next_scan` already carries
/// it; this is for the shared vectors.
#[uniffi::export]
#[must_use]
pub fn incoming_next_scan_ms(visible: bool, quiet_scans: u32) -> u32 {
    vela_core::app::incoming_scan::next_scan_ms(visible, quiet_scans)
}

/// r13-1: where an account used on `chain_ids` lives when none of its
/// networks is in the space in front — `network_space::lives_elsewhere`, the
/// one rule every home draws its other-space note from (the wasm twin is
/// `networkLivesElsewhere`, the same JSON). `args_json`: `{chain_ids, here,
/// other, front}` — `here` / `other` the front space's and the other space's
/// networks on the person's lists, `front` as stored ("mainnet" /
/// "testnet"). The answer: an `Elsewhere` (`{chain_ids, space, line_key,
/// heading_key, switch_key}`), or `null` — nothing to say.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn network_lives_elsewhere(args_json: String) -> String {
    vela_core::app::network_space::lives_elsewhere_json(&args_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// r13-1: the natives' other-space rule is the core's, through this
    /// export — every case of the shared vector, as Android and iOS replay it.
    #[test]
    fn the_other_space_vectors_hold_through_the_export() {
        let doc: serde_json::Value = serde_json::from_str(include_str!(
            "../../vela-core/tests/vectors/multisig/other-space.json"
        ))
        .unwrap();
        let cases = doc["cases"].as_array().unwrap();
        assert!(cases.len() >= 6);
        for case in cases {
            let args = serde_json::json!({
                "chain_ids": case["chain_ids"],
                "here": case["here"],
                "other": case["other"],
                "front": case["front"],
            });
            let got: serde_json::Value =
                serde_json::from_str(&network_lives_elsewhere(args.to_string())).unwrap();
            assert_eq!(got, case["expect"], "{}", case["name"]);
        }
        // Unreadable arguments are nothing to say, never a panic.
        assert_eq!(network_lives_elsewhere("not json".to_owned()), "null");
    }

    #[test]
    fn the_incoming_scan_exports_are_the_core_rules() {
        let plan = incoming_scan_plan(
            r#"{"account_networks":[{"chain_id":5042002}],"space_chain_ids":[11155111,5042002]}"#
                .to_owned(),
        );
        assert_eq!(
            plan,
            r#"{"scan_chain_ids":[5042002],"native_watch_chain_ids":[5042002]}"#
        );
        assert_eq!(incoming_next_scan_ms(false, 0), 120_000);
        assert_eq!(incoming_next_scan_ms(true, 9), 120_000);
        assert!(incoming_scan_windows("{}".to_owned()).contains("\"live\""));
    }

    #[test]
    fn the_exports_are_the_core_rules() {
        assert!(launch_animation_due(None, 0.0, false));
        assert!(!launch_animation_due(Some(0.0), 1.0, false));
        assert!(!launch_animation_due(None, 0.0, true));
        assert_eq!(launch_animation_storage_key(), "vela.launch.played");
        assert_eq!(
            ens_namehash("eth".to_owned()),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert!(ens_reverse_node("not an address".to_owned()).is_err());
    }
}
