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

#[cfg(test)]
mod tests {
    use super::*;

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
