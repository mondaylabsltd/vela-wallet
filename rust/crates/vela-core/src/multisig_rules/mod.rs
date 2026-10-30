//! The multisig's pure rules a shell asks one at a time — what a screen
//! must not decide for itself, outside any machine: the relay's token
//! schedule, a member's label, the order networks are offered in, an
//! executed proposal's Activity rows, which calls a review reads.
//!
//! One door for every shell: `multisig_link::call_json` (wasm
//! `multisigLinkCall`, uniffi `multisig_link_call`) hands any method it does
//! not know here, as `<area>.<method>` (`relay.usable`, `names.label`, …).
//! Each area is its own module with its own `call`; the arguments are one
//! JSON object, the answer JSON.

use crate::CoreError;

/// A method this door knows, answered; `None` when no area claims it.
pub fn call_json(method: &str, args: &serde_json::Value) -> Option<Result<String, CoreError>> {
    let (area, name) = method.split_once('.')?;
    match area {
        _ => {
            let _ = (name, args);
            None
        }
    }
}

/// An argument by name, read as `T`.
#[allow(dead_code)]
pub(crate) fn arg<T: serde::de::DeserializeOwned>(
    args: &serde_json::Value,
    name: &str,
) -> Result<T, CoreError> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| CoreError::Multisig(format!("argument {name}: {e}")))
}

/// An answer as JSON.
#[allow(dead_code)]
pub(crate) fn to_json<T: serde::Serialize>(value: &T) -> Result<String, CoreError> {
    serde_json::to_string(value).map_err(|e| CoreError::Multisig(format!("answer: {e}")))
}
