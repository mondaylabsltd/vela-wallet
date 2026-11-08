//! VELA-2026-002: the parent-Safe guard's pure rule
//! (`vela_core::app::parent_safe_guard`) for the phones' vector replay
//! (Android JVM, iOS XCTest). The wasm twin is
//! `vela-core-wasm/src/parent_safe_guard.rs`. The sign machine applies the
//! rule itself; a shell only answers its `read_safe_owner` operation.

/// `parent_safe_guard::check` over JSON: a `CheckInput` in, a
/// `ParentVerdict` out, or `{ "error": … }` for an input that does not read.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn parent_safe_check(input_json: String) -> String {
    vela_core::app::parent_safe_guard::check_json(&input_json)
        .unwrap_or_else(|error| serde_json::json!({ "error": error }).to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_vector_answers_through_this_door() {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../vela-core/tests/vectors/rules/parent-safe-guard.json"
        ))
        .unwrap();
        for case in v["cases"].as_array().unwrap() {
            let got: serde_json::Value =
                serde_json::from_str(&parent_safe_check(case["input"].to_string())).unwrap();
            assert_eq!(got, case["expect"], "{}", case["name"]);
        }
        let bad: serde_json::Value = serde_json::from_str(&parent_safe_check("{".into())).unwrap();
        assert!(bad["error"].is_string());
    }
}
