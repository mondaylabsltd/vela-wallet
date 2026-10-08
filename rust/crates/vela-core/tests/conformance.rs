//! Conformance corpus runner.
//!
//! Replays `tests/vectors/*.json` (extracted from the production TypeScript
//! implementations by `scripts/dump-vectors/` in the app repo) against this
//! crate. Schema: specs/001-rust-core-bindings/contracts/conformance-vectors.md.
//!
//! A red test here means a byte divergence from shipping wallet behavior —
//! treat as a release blocker, not a flaky test.

use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use vela_core::i18n;

#[derive(Deserialize)]
struct SuiteFile {
    suite: String,
    /// Absent in the bulk identicon suite, which carries `pairs` instead and is
    /// replayed by its own runner below.
    #[serde(default)]
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct BulkSuiteFile {
    pairs: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    #[serde(rename = "fn")]
    func: String,
    input: Value,
    expect: Value,
    #[allow(dead_code)]
    divergence: Option<Value>,
}

/// The corpus is seven suites, discovered by scanning the directory. Every runner
/// asserts that exact set is present: without it, a vector file lost to a bad merge
/// or a partial checkout would make all four surfaces report "green" over a corpus
/// that had silently shrunk — the precise false confidence this feature exists to
/// prevent.
const REQUIRED_SUITES: [&str; 13] = [
    "abi",
    "eip712",
    // `i18n-*` sorts before `identicon`: '1' is 0x31, 'd' is 0x64.
    "i18n-behaviour",
    "i18n-exhaustive",
    "i18n-plural",
    "i18n-plural-legacy",
    "identicon",
    "identicon-bulk",
    // Which logo a token or a network wears (`app::remote_mark`), hand-written
    // from the four shells' rule rather than dumped from a TypeScript oracle.
    "marks",
    "primitives",
    "safe",
    // `safe` before `safe-multi`: a prefix sorts before its extension.
    "safe-multi",
    "webauthn",
];

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors")
}

// ---------------------------------------------------------------------------
// Input/expect helpers
// ---------------------------------------------------------------------------

fn in_str(input: &Value, key: &str) -> Result<String, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string input `{key}`"))
}

fn in_bytes(input: &Value, key: &str) -> Result<Vec<u8>, String> {
    let s = in_str(input, key)?;
    let clean = s.strip_prefix("0x").unwrap_or(&s);
    hex_decode(clean).map_err(|e| format!("bad hex in input `{key}`: {e}"))
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd length".to_owned());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// The expected error code if this case expects an error.
fn expected_error(expect: &Value) -> Option<&str> {
    expect.get("error").and_then(Value::as_str)
}

/// Compare a fallible bytes-returning call against the expectation.
fn check_bytes(
    expect: &Value,
    actual: Result<Vec<u8>, vela_core::CoreError>,
) -> Result<(), String> {
    check_with(expect, actual.map(|b| Value::String(hex_encode(&b))))
}

/// Compare a fallible string-returning call against the expectation.
fn check_string(
    expect: &Value,
    actual: Result<String, vela_core::CoreError>,
) -> Result<(), String> {
    check_with(expect, actual.map(Value::String))
}

/// Core comparison: expectation is either {error: code} or {value: <json>} /
/// an object of named fields (compared by the caller via `check_object`).
fn check_with(expect: &Value, actual: Result<Value, vela_core::CoreError>) -> Result<(), String> {
    match (expected_error(expect), actual) {
        (Some(code), Err(e)) => {
            if e.code() == code {
                Ok(())
            } else {
                Err(format!(
                    "expected error {code}, got error {} ({e})",
                    e.code()
                ))
            }
        }
        (Some(code), Ok(v)) => Err(format!("expected error {code}, got Ok({v})")),
        (None, Err(e)) => Err(format!("expected success, got error {} ({e})", e.code())),
        (None, Ok(v)) => {
            let want = expect
                .get("value")
                .ok_or_else(|| "expectation missing `value`".to_owned())?;
            if values_equal(want, &v) {
                Ok(())
            } else {
                Err(format!("expected {want}, got {v}"))
            }
        }
    }
}

/// Compare a fallible struct-returning call (serialized to JSON) field-by-field
/// against every non-error key in the expectation object.
fn check_object(expect: &Value, actual: Result<Value, vela_core::CoreError>) -> Result<(), String> {
    match (expected_error(expect), actual) {
        (Some(code), Err(e)) => {
            if e.code() == code {
                Ok(())
            } else {
                Err(format!(
                    "expected error {code}, got error {} ({e})",
                    e.code()
                ))
            }
        }
        (Some(code), Ok(v)) => Err(format!("expected error {code}, got Ok({v})")),
        (None, Err(e)) => Err(format!("expected success, got error {} ({e})", e.code())),
        (None, Ok(actual_obj)) => {
            let expect_obj = expect
                .as_object()
                .ok_or_else(|| "expectation is not an object".to_owned())?;
            // An expectation with no fields would pass over ANY result.
            if expect_obj.keys().all(|k| k == "error") {
                return Err("expectation has no fields to check".to_owned());
            }
            for (key, want) in expect_obj {
                let got = actual_obj
                    .get(key)
                    .ok_or_else(|| format!("actual result missing field `{key}`"))?;
                if !values_equal(want, got) {
                    return Err(format!("field `{key}`: expected {want}, got {got}"));
                }
            }
            Ok(())
        }
    }
}

/// Hex strings compare case-insensitively ONLY when both sides are plain
/// lowercase-vs-value mismatches would hide checksum bugs, so: exact string
/// compare, except numbers compare numerically.
fn values_equal(want: &Value, got: &Value) -> bool {
    want == got
}

// ---------------------------------------------------------------------------
// Dispatcher — one arm per contracts/core-api.md function, added module by module
// ---------------------------------------------------------------------------

fn in_bool(input: &Value, key: &str) -> Result<bool, String> {
    input
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("missing bool input `{key}`"))
}

fn run_case(case: &Case) -> Result<(), String> {
    use vela_core::identicon as ic;
    use vela_core::primitives as prim;
    let input = &case.input;
    let expect = &case.expect;
    match case.func.as_str() {
        // --- primitives ---
        "keccak256" => check_bytes(expect, Ok(prim::keccak256(&in_bytes(input, "data")?))),
        "sha256" => check_bytes(expect, Ok(prim::sha256(&in_bytes(input, "data")?))),
        "checksum_address" => check_string(
            expect,
            prim::checksum_address(&in_str(input, "address_hex")?),
        ),
        "function_selector" => check_bytes(
            expect,
            prim::function_selector(&in_str(input, "signature")?),
        ),
        "create2_address" => check_string(
            expect,
            prim::create2_address(
                &in_str(input, "deployer_hex")?,
                &in_bytes(input, "salt")?,
                &in_bytes(input, "init_code_hash")?,
            ),
        ),
        "to_hex" => check_string(
            expect,
            Ok(prim::to_hex(
                &in_bytes(input, "data")?,
                in_bool(input, "prefixed")?,
            )),
        ),
        "from_hex" => check_bytes(expect, prim::from_hex(&in_str(input, "s")?)),
        "to_quantity" => check_string(expect, prim::to_quantity(&in_str(input, "value")?)),
        "to_base64url" => check_string(expect, Ok(prim::to_base64url(&in_bytes(input, "data")?))),
        "from_base64url" => check_bytes(expect, prim::from_base64url(&in_str(input, "s")?)),
        "abi_encode_address" => check_bytes(
            expect,
            prim::abi_encode_address(&in_str(input, "address_hex")?),
        ),
        "abi_encode_uint256" => check_bytes(
            expect,
            prim::abi_encode_uint256(&in_str(input, "value_hex")?),
        ),
        "abi_encode_bytes32" => {
            check_bytes(expect, prim::abi_encode_bytes32(&in_bytes(input, "data")?))
        }
        // --- abi ---
        "canonicalize_signature" => check_string(
            expect,
            vela_core::abi::canonicalize_signature(&in_str(input, "sig")?),
        ),
        "compute_selector" => check_string(
            expect,
            vela_core::abi::compute_selector(&in_str(input, "sig")?),
        ),
        "match_selector" => check_with(
            expect,
            vela_core::abi::match_selector(&in_str(input, "sig")?, &in_bytes(input, "calldata")?)
                .map(Value::Bool),
        ),
        "decode_calldata" => check_with(
            expect,
            vela_core::abi::decode_calldata(&in_str(input, "sig")?, &in_bytes(input, "calldata")?)
                .map(|tree| serde_json::to_value(tree).unwrap_or(Value::Null)),
        ),
        // --- eip712 ---
        "hash_typed_data" => check_bytes(
            expect,
            vela_core::eip712::hash_typed_data(&in_str(input, "typed_data_json")?),
        ),
        "encode_type" => check_string(
            expect,
            vela_core::eip712::encode_type(&in_str(input, "typed_data_json")?),
        ),
        // --- safe ---
        "parse_public_key" => check_object(
            expect,
            vela_core::safe::parse_public_key(&in_str(input, "hex")?)
                .map(|k| serde_json::json!({ "x": hex_encode(&k.x), "y": hex_encode(&k.y) })),
        ),
        "compute_safe_address" => check_object(
            expect,
            vela_core::safe::compute_safe_address(&in_bytes(input, "x")?, &in_bytes(input, "y")?)
                .map(|info| {
                    serde_json::json!({
                        "address": info.address,
                        "salt_nonce": hex_encode(&info.salt_nonce),
                        "setup_data": hex_encode(&info.setup_data),
                        "init_code_hash": hex_encode(&info.init_code_hash),
                    })
                }),
        ),
        "compute_safe_address_multi" => {
            let keys = input
                .get("keys")
                .and_then(Value::as_array)
                .ok_or_else(|| "missing array input `keys`".to_owned())?
                .iter()
                .map(|k| {
                    Ok(vela_core::P256PublicKey {
                        x: in_bytes(k, "x")?,
                        y: in_bytes(k, "y")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            check_object(
                expect,
                vela_core::safe::compute_safe_address_multi(&keys).map(|info| {
                    serde_json::json!({
                        "address": info.address,
                        "salt_nonce": hex_encode(&info.salt_nonce),
                        "setup_data": hex_encode(&info.setup_data),
                        "init_code_hash": hex_encode(&info.init_code_hash),
                    })
                }),
            )
        }
        "compute_webauthn_signer_address" => check_object(
            expect,
            vela_core::safe::compute_webauthn_signer_address(
                &in_bytes(input, "x")?,
                &in_bytes(input, "y")?,
            )
            .map(|address| serde_json::json!({ "address": address })),
        ),
        "compute_splitter_address" => check_string(
            expect,
            vela_core::safe::compute_splitter_address(&in_str(input, "treasury_hex")?),
        ),
        "encode_splitter_deploy_call" => check_bytes(
            expect,
            vela_core::safe::encode_splitter_deploy_call(&in_str(input, "treasury_hex")?),
        ),
        "safe_proxy_runtime_code" => {
            check_string(expect, vela_core::safe::safe_proxy_runtime_code())
        }
        // --- webauthn ---
        "extract_attestation_public_key" => check_object(
            expect,
            vela_core::webauthn::extract_attestation_public_key(&in_bytes(
                input,
                "attestation_object",
            )?)
            .map(|k| serde_json::json!({ "x": hex_encode(&k.x), "y": hex_encode(&k.y) })),
        ),
        "der_signature_to_raw_low_s" => check_bytes(
            expect,
            vela_core::webauthn::der_signature_to_raw_low_s(&in_bytes(input, "der")?),
        ),
        "validate_client_data" => {
            let kind = match in_str(input, "kind")?.as_str() {
                "Get" => vela_core::ClientDataKind::Get,
                "Create" => vela_core::ClientDataKind::Create,
                other => return Err(format!("unknown ClientDataKind `{other}`")),
            };
            check_with(
                expect,
                vela_core::webauthn::validate_client_data(
                    kind,
                    &in_bytes(input, "client_data_json")?,
                    &in_bytes(input, "authenticator_data")?,
                )
                .map(|()| Value::Bool(true)),
            )
        }
        "webauthn_signing_hash" => check_bytes(
            expect,
            Ok(vela_core::webauthn::webauthn_signing_hash(
                &in_bytes(input, "authenticator_data")?,
                &in_bytes(input, "client_data_json")?,
            )),
        ),
        "recover_public_key_from_assertions" => {
            let assertion = |key: &str| -> Result<vela_core::WebAuthnAssertion, String> {
                let obj = input
                    .get(key)
                    .ok_or_else(|| format!("missing assertion input `{key}`"))?;
                Ok(vela_core::WebAuthnAssertion {
                    authenticator_data: in_bytes(obj, "authenticator_data")?,
                    client_data_json: in_bytes(obj, "client_data_json")?,
                    signature_der: in_bytes(obj, "signature_der")?,
                })
            };
            check_with(
                expect,
                vela_core::webauthn::recover_public_key_from_assertions(
                    &assertion("a")?,
                    &assertion("b")?,
                )
                .map(|opt| match opt {
                    Some(k) => Value::String(format!(
                        "04{}{}",
                        &hex_encode(&k.x)[2..],
                        &hex_encode(&k.y)[2..]
                    )),
                    None => Value::Null,
                }),
            )
        }
        // --- identicon ---
        "make_hash" => check_with(
            expect,
            Ok(Value::String(
                ic::make_hash(&in_str(input, "seed")?).as_str().to_owned(),
            )),
        ),
        "identicon_params" => check_object(
            expect,
            ic::identicon_params(&in_str(input, "seed")?).map(params_to_json),
        ),
        "identicon_params_js_compat" => check_object(
            expect,
            ic::identicon_params_js_compat(&in_str(input, "seed")?).map(params_to_json),
        ),
        "section_svg" => {
            let section = in_section(input)?;
            let index = in_i64(input, "index")?;
            check_string(expect, ic::section_svg(section, index).map(str::to_owned))
        }
        "identicon_svg" => check_string(expect, ic::identicon_svg(&in_str(input, "seed")?)),
        "identicon_svg_circular" => {
            check_string(expect, ic::identicon_svg_circular(&in_str(input, "seed")?))
        }
        "identicon_data_uri" => {
            check_string(expect, ic::identicon_data_uri(&in_str(input, "seed")?))
        }
        "create_identicon" => check_string(
            expect,
            ic::create_identicon(
                &in_str(input, "seed")?,
                ic::CreateOptions {
                    validate_address: in_bool(input, "validate_address")?,
                    format: match in_str(input, "format")?.as_str() {
                        "svg" => ic::IdenticonFormat::Svg,
                        "image/svg+xml" => ic::IdenticonFormat::DataUri,
                        other => return Err(format!("unknown identicon format `{other}`")),
                    },
                },
            ),
        ),
        "normalize_seed" => check_string(
            expect,
            Ok(ic::normalize_seed(&in_str(input, "seed")?).into_owned()),
        ),
        "nimiq_is_valid_address" => check_with(
            expect,
            Ok(Value::Bool(ic::nimiq_is_valid_address(&in_str(
                input, "input",
            )?))),
        ),
        // Shared SVG fragments, so a typo in a 400-byte constant fails here rather
        // than showing up as a subtly wrong avatar.
        "constants" => check_object(
            expect,
            Ok(serde_json::json!({
                "default_shadow": ic::DEFAULT_SHADOW,
                "default_circle_shape": ic::default_circle_shape("#FC8702"),
                "identicon_placeholder": ic::IDENTICON_PLACEHOLDER,
                "identicon_placeholder_base64": ic::IDENTICON_PLACEHOLDER_BASE64,
                "default_background_shape": ic::DEFAULT_BACKGROUND_SHAPE,
            })),
        ),
        // --- i18n (spec 004, contracts/conformance-vectors.md) ------------------
        //
        // Every arm builds a fresh engine: `I18n` owns an active language, so
        // sharing one across cases would make the corpus order-dependent, and an
        // order-dependent corpus is worse than no corpus.
        "i18n_t" | "i18n_t_legacy_plural" => {
            let mode = if case.func == "i18n_t_legacy_plural" {
                i18n::PluralMode::Legacy
            } else {
                i18n::PluralMode::Cldr
            };
            let owned = i18n_opts(input.get("opts"))?;
            let key = in_key(input, "key")?;
            let lng = in_str(input, "lng").unwrap_or_else(|_| "en".to_owned());
            let mut scratch = i18n::Scratch::default();
            let opts = owned.as_options(&mut scratch);
            check_string(
                expect,
                i18n_engine(&lng, &lng, mode).and_then(|e| e.t(&key, &opts)),
            )
        }
        "i18n_t_keys" => {
            let keys = in_str_list(input, "keys")?;
            let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            let owned = i18n_opts(input.get("opts"))?;
            let mut scratch = i18n::Scratch::default();
            let opts = owned.as_options(&mut scratch);
            check_string(expect, {
                let lng = in_str(input, "lng").unwrap_or_else(|_| "en".to_owned());
                i18n_engine(&lng, &lng, i18n::PluralMode::Cldr)
                    .and_then(|e| e.t_first(&refs, &opts))
            })
        }
        // Per-call `{lng}` is a DIFFERENT code path in i18next from
        // init/changeLanguage — proved by the corpus: `zh_TW`, `zh-Hant`,
        // `zh-Hant-TW` and `es-AR` resolve to a real locale through
        // changeLanguage but fall through to `en` here. Two arms, deliberately.
        "i18n_t_lng_option" => {
            let owned = i18n_opts(input.get("opts"))?;
            let key = in_key(input, "key")?;
            let mut scratch = i18n::Scratch::default();
            let opts = owned.as_options(&mut scratch);
            // The per-call tag's catalog must be resident, but the ACTIVE language
            // stays `en` — that is what `init({lng:'en'}) + t(k, {lng})` does.
            // The per-call path canonicalises a hyphenated tag (`zh-tw` -> `zh-TW`)
            // but has no recovery ladder, so an unsupported tag stays unsupported.
            let target = owned
                .lng
                .as_deref()
                .map(i18n::canonical_tag)
                .unwrap_or_else(|| "en".to_owned());
            let resident = if i18n::SUPPORTED.contains(&target.as_str()) {
                target
            } else {
                "en".to_owned()
            };
            check_string(
                expect,
                i18n_engine(&resident, "en", i18n::PluralMode::Cldr).and_then(|e| e.t(&key, &opts)),
            )
        }
        "i18n_interpolate" => {
            let owned = i18n_opts(input.get("opts"))?;
            let mut scratch = i18n::Scratch::default();
            let opts = owned.as_options(&mut scratch);
            check_string(
                expect,
                i18n::interpolate(&in_str(input, "template")?, &opts),
            )
        }
        "i18n_plural_suffix" => check_string(
            expect,
            Ok(i18n::plural_suffix(
                &in_str(input, "lng")?,
                in_count_f64(input)?,
            )),
        ),
        "i18n_plural_suffixes" => check_with(
            expect,
            Ok(json_strings(i18n::plural_suffixes(&in_str(input, "lng")?))),
        ),
        "i18n_plural_suffix_legacy" => {
            check_string(expect, Ok(i18n::plural_suffix_legacy(in_count_f64(input)?)))
        }
        "i18n_plural_suffixes_legacy" => {
            check_with(expect, Ok(json_strings(i18n::plural_suffixes_legacy())))
        }
        // The only two i18n arms returning objects, hence the only two routed
        // through check_object. This is why the raw TypeError text lives in
        // `divergence.ts_behavior` and never as a sibling key inside `expect`:
        // check_object demands a matching result field for EVERY non-`error`
        // expectation key, so a stray `ts_throw` would make the case unrunnable
        // and read like a port bug.
        "i18n_resolve_language" | "i18n_change_language" => {
            let state = i18n::resolve_language(&in_str(input, "requested")?);
            check_object(
                expect,
                Ok(serde_json::json!({
                    "language": state.language,
                    "resolved_language": state.resolved_language,
                    "languages": state.languages,
                })),
            )
        }
        // --- marks (app::remote_mark; the module needs the `crux` feature) ---
        #[cfg(feature = "crux")]
        "token_mark" => {
            let address = in_opt_str(input, "token_address")?;
            let mark = vela_core::app::remote_mark::token_mark(
                &in_str(input, "ethereum_data_url")?,
                in_u32(input, "chain_id")?,
                &in_str(input, "symbol")?,
                address.as_deref(),
                &in_str_list(input, "named")?,
            );
            check_object(expect, Ok(serde_json::to_value(mark).unwrap_or_default()))
        }
        #[cfg(feature = "crux")]
        "chain_mark" => {
            let mark = vela_core::app::remote_mark::chain_mark(
                &in_str(input, "ethereum_data_url")?,
                in_u32(input, "chain_id")?,
                &in_str(input, "native_symbol")?,
            );
            check_object(expect, Ok(serde_json::to_value(mark).unwrap_or_default()))
        }
        #[cfg(feature = "crux")]
        "chain_logo_url" => check_with(
            expect,
            Ok(vela_core::app::remote_mark::chain_logo_url(
                &in_str(input, "ethereum_data_url")?,
                in_u32(input, "chain_id")?,
            )
            .map_or(Value::Null, Value::String)),
        ),
        other => Err(format!(
            "no dispatch arm for fn `{other}` — add it to conformance.rs"
        )),
    }
}

/// A string input that may be JSON `null` (an absent value the function
/// takes as `None`). Missing altogether is a malformed vector, not `None`.
#[cfg(feature = "crux")]
fn in_opt_str(input: &Value, key: &str) -> Result<Option<String>, String> {
    match input.get(key) {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Null) => Ok(None),
        _ => Err(format!("missing string-or-null input `{key}`")),
    }
}

#[cfg(feature = "crux")]
fn in_u32(input: &Value, key: &str) -> Result<u32, String> {
    input
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("missing u32 input `{key}`"))
}

// ---------------------------------------------------------------------------
// i18n helpers
// ---------------------------------------------------------------------------

/// Build an engine with `catalog_lng` resident and `change_lng` active.
///
/// The two are separate because the corpus exercises two different upstream
/// functions: `change_language(lng) + t(key)` and `t(key, {lng})`. In the second,
/// i18next's *active* language stays whatever `init` set (`en`) while resolution
/// runs against the per-call tag — so the engine must have that tag's catalog
/// resident without having switched to it.
fn i18n_engine(
    catalog_lng: &str,
    change_lng: &str,
    mode: i18n::PluralMode,
) -> Result<i18n::I18n, vela_core::CoreError> {
    let mut engine = i18n::I18n::embedded()?.with_plural_mode(mode);
    if catalog_lng != "en" {
        let _displaced = engine.load_catalog(i18n::Catalog::embedded(catalog_lng)?);
    }
    engine.change_language(change_lng);
    Ok(engine)
}

fn json_strings(v: Vec<String>) -> Value {
    Value::Array(v.into_iter().map(Value::String).collect())
}

/// i18next coerces a non-string key with `String(key)` (`i18next.js:547`), so a
/// numeric or null key is a legitimate lookup, not a malformed vector.
fn in_key(input: &Value, key: &str) -> Result<String, String> {
    match input.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Null) => Ok("null".to_owned()),
        Some(v) => Ok(v.to_string()),
        None => Err(format!("missing input `{key}`")),
    }
}

fn in_str_list(input: &Value, key: &str) -> Result<Vec<String>, String> {
    input
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array input `{key}`"))?
        .iter()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Null => "null".to_owned(),
            other => other.to_string(),
        })
        .map(Ok)
        .collect()
}

/// `count` for the standalone plural arms, which only ever pass a plain number.
fn in_count_f64(input: &Value) -> Result<f64, String> {
    input
        .get("count")
        .and_then(Value::as_f64)
        .ok_or_else(|| "missing numeric input `count`".to_owned())
}

/// Decode the tagged option encoding the dumper emits for values JSON cannot
/// hold: `{"__t": "undefined"|"nan"|"infinity"|"bigint"|"date"|"fn"}`.
/// See contracts/conformance-vectors.md §6.
/// `Array.prototype.join(",")` semantics, which flatten nested arrays:
/// `[[1],[2]].join(",")` is `"1,2"`, not `"[1],[2]"`.
fn js_join(v: &Value) -> String {
    match v {
        Value::Array(a) => a.iter().map(js_join).collect::<Vec<_>>().join(","),
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Whether an option value stringifies through host semantics Rust cannot reach —
/// a JS `Date`, a callable, or an object carrying its own `toString`. These are
/// rejected rather than approximated (spec FR-008 / T035), and the corpus expects
/// `I18nUnsupportedOption` with the JS text recorded in `divergence.ts_behavior`.
fn is_host_only(v: &Value) -> bool {
    match v {
        Value::Object(o) => {
            matches!(o.get("__t").and_then(Value::as_str), Some("date" | "fn"))
                || o.values().any(is_host_only)
        }
        _ => false,
    }
}

fn i18n_var(v: &Value) -> i18n::OwnedVar {
    match v {
        Value::Null => i18n::OwnedVar::Null,
        Value::Bool(b) => i18n::OwnedVar::Bool(*b),
        Value::Number(n) => i18n::OwnedVar::Num(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => i18n::OwnedVar::Str(s.clone()),
        Value::Array(_) => i18n::OwnedVar::Array(js_join(v)),
        Value::Object(o) => match o.get("__t").and_then(Value::as_str) {
            Some("undefined") => i18n::OwnedVar::Undefined,
            Some("nan") => i18n::OwnedVar::Num(f64::NAN),
            Some("infinity") => {
                i18n::OwnedVar::Num(if o.get("sign").and_then(Value::as_i64).unwrap_or(1) < 0 {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                })
            }
            Some("bigint") => i18n::OwnedVar::Str(
                o.get("v")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
            _ => i18n::OwnedVar::Object,
        },
    }
}

/// Flatten a nested option object into dotted variable names, so `{{a.b.c}}`
/// resolves. i18next walks the options object by path; a flat name list needs the
/// dotted forms materialised. The object itself is kept too, because `{{a}}` must
/// still render `[object Object]`.
fn push_var(out: &mut Vec<(String, i18n::OwnedVar)>, name: &str, v: &Value) {
    out.push((name.to_owned(), i18n_var(v)));
    if let Value::Object(o) = v {
        if o.contains_key("__t") {
            return;
        }
        for (k, inner) in o {
            push_var(out, &format!("{name}.{k}"), inner);
        }
    }
}

/// Split an `opts` object into the reserved i18next names and the interpolation
/// variables. The reserved set is `i18next.js:880` plus `count`.
fn i18n_opts(opts: Option<&Value>) -> Result<i18n::OwnedOptions, String> {
    let mut out = i18n::OwnedOptions::default();
    let Some(Value::Object(map)) = opts else {
        return Ok(out);
    };
    // `const data = options.replace && !isString(options.replace) ? options.replace : options`
    // (`i18next.js:1180`): when `replace` is an object it REPLACES the options as
    // the interpolation source, so a top-level `v` is shadowed rather than merged.
    let replace_source = map.get("replace").filter(|v| v.is_object());
    for (k, v) in map {
        if is_host_only(v) {
            out.unsupported.push(k.clone());
            continue;
        }
        match k.as_str() {
            "count" => {
                out.count = Some(match v {
                    Value::Null => i18n::Count::Null,
                    Value::Number(n) => i18n::Count::Num(n.as_f64().unwrap_or(f64::NAN)),
                    // A STRING count silently disables plural resolution in
                    // i18next — the raw key comes back. Coercing it would be
                    // helpful and wrong.
                    Value::String(s) => i18n::Count::Str(s.clone()),
                    Value::Object(o) if o.get("__t").and_then(Value::as_str) == Some("bigint") => {
                        i18n::Count::BigInt(
                            o.get("v")
                                .and_then(Value::as_str)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0),
                        )
                    }
                    Value::Object(o) if o.get("__t").and_then(Value::as_str) == Some("nan") => {
                        i18n::Count::Num(f64::NAN)
                    }
                    Value::Object(o)
                        if o.get("__t").and_then(Value::as_str) == Some("infinity") =>
                    {
                        i18n::Count::Num(
                            if o.get("sign").and_then(Value::as_i64).unwrap_or(1) < 0 {
                                f64::NEG_INFINITY
                            } else {
                                f64::INFINITY
                            },
                        )
                    }
                    // An own property that is `undefined` is NOT a count at all:
                    // `count !== undefined` is false, so no plural candidate is
                    // built and the key echoes.
                    Value::Object(o)
                        if o.get("__t").and_then(Value::as_str) == Some("undefined") =>
                    {
                        out.count = None;
                        continue;
                    }
                    Value::Object(_) => i18n::Count::Object,
                    Value::Bool(b) => i18n::Count::Num(if *b { 1.0 } else { 0.0 }),
                    Value::Array(_) => i18n::Count::Object,
                });
            }
            "context" => out.context = v.as_str().map(str::to_owned),
            "lng" => out.lng = v.as_str().map(str::to_owned),
            "ns" => out.ns = v.as_str().map(str::to_owned),
            "ordinal" => out.ordinal = v.as_bool().unwrap_or(false),
            // `defaultValue: null` is IGNORED (the key comes back) while
            // `defaultValue: ''` is HONOURED. Two adjacent falsy values, opposite
            // outcomes — so map Null to None rather than to Some("").
            "defaultValue" => match v {
                Value::Null => out.default_value = None,
                Value::String(s) => out.default_value = Some(s.clone()),
                Value::Number(n) => out.default_value = Some(n.to_string()),
                Value::Bool(b) => out.default_value = Some(b.to_string()),
                // An object or array default is itself a non-string, so i18next
                // answers with the same diagnostic a branch node produces —
                // UNLESS joinArrays is also set, which makes it return the array.
                Value::Array(_) => {
                    if map.contains_key("joinArrays") {
                        out.unsupported.push("joinArrays".to_owned());
                    } else {
                        out.default_value_object = true;
                    }
                }
                Value::Object(o) => {
                    if o.get("__t").and_then(Value::as_str) == Some("undefined") {
                        // An own property that is `undefined` is ignored, exactly
                        // like an absent one — the key echoes.
                        out.default_value = None;
                    } else {
                        out.default_value_object = true;
                    }
                }
            },
            // i18next returns a non-string for these; a Rust `t()` is string-typed
            // by construction, so they are typed errors rather than silent coercions.
            "returnObjects" | "returnDetails" => out.unsupported.push(k.clone()),
            // `joinArrays` alone is harmless — on a branch node it still yields the
            // object diagnostic. It only produces a non-string when paired with an
            // ARRAY defaultValue, which the `defaultValue` arm already flags.
            "joinArrays" => {}
            // `false` disables the separator; any other value is an override this
            // engine does not model.
            "keySeparator" => {
                if v.as_bool() == Some(false) {
                    out.key_separator_off = true;
                } else {
                    out.unsupported.push(k.clone());
                }
            }
            "nsSeparator" => {
                if v.as_bool() == Some(false) {
                    out.ns_separator_off = true;
                } else {
                    out.unsupported.push(k.clone());
                }
            }
            _ if k.starts_with("defaultValue_") => {
                out.default_value_variants.push((
                    k.trim_start_matches("defaultValue_").to_owned(),
                    v.as_str().unwrap_or_default().to_owned(),
                ));
            }
            // `replace` is the interpolation source when it is an object, not a
            // variable in its own right.
            "replace" if replace_source.is_some() => {}
            _ => {
                if replace_source.is_none() {
                    push_var(&mut out.vars, k, v);
                }
            }
        }
    }
    if let Some(Value::Object(r)) = replace_source {
        for (k, v) in r {
            push_var(&mut out.vars, k, v);
        }
    }
    Ok(out)
}

fn in_i64(input: &Value, key: &str) -> Result<i64, String> {
    input
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing integer input `{key}`"))
}

fn in_section(input: &Value) -> Result<vela_core::identicon::Section, String> {
    use vela_core::identicon::Section;
    match in_str(input, "section")?.as_str() {
        "face" => Ok(Section::Face),
        "sides" => Ok(Section::Sides),
        "top" => Ok(Section::Top),
        "bottom" => Ok(Section::Bottom),
        other => Err(format!("unknown identicon section `{other}`")),
    }
}

/// Params expectations carry section INDICES rather than the full 2 KB artwork, so
/// the corpus stays reviewable. That is only trustworthy because the `section-table`
/// group pins all 84 fragments by full text against the package — both ends are
/// anchored to the oracle, so resolving an index through the same table here is not
/// circular.
fn params_to_json(params: vela_core::identicon::IdenticonParams) -> Value {
    use vela_core::identicon::{section_svg, Section};
    let index_of = |section: Section, svg: &str| -> Value {
        for n in 1..=vela_core::identicon::SECTION_COUNT {
            #[allow(clippy::cast_possible_wrap)]
            if section_svg(section, n as i64 - 1).is_ok_and(|s| s == svg) {
                return Value::from(n);
            }
        }
        Value::Null
    };
    serde_json::json!({
        "main": params.colors.main,
        "background": params.colors.background,
        "accent": params.colors.accent,
        "face": index_of(Section::Face, params.sections.face),
        "top": index_of(Section::Top, params.sections.top),
        "sides": index_of(Section::Sides, params.sections.sides),
        "bottom": index_of(Section::Bottom, params.sections.bottom),
    })
}

#[test]
fn conformance_corpus() {
    let dir = vectors_dir();
    if !dir.exists() {
        panic!(
            "no conformance corpus at {} — regenerate it with `npm run dump:vectors`",
            dir.display()
        );
    }
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut seen_suites: Vec<String> = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map(|it| it.flatten().collect::<Vec<_>>())
        .unwrap_or_default();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: unreadable: {e}", path.display()));
                continue;
            }
        };
        let suite: SuiteFile = match serde_json::from_str(&raw) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: bad schema: {e}", path.display()));
                continue;
            }
        };
        seen_suites.push(suite.suite.clone());
        // The marks rule lives in `app::remote_mark`, which only exists with
        // the `crux` feature. Every CI run has it (the workspace build turns it
        // on through vela-core-uniffi and vela-core-wasm); a crate-only run
        // without it says so instead of counting cases it could not check.
        if suite.suite == "marks" && !cfg!(feature = "crux") {
            println!(
                "conformance: the marks suite ({} cases) needs the `crux` feature — skipped",
                suite.cases.len()
            );
            continue;
        }
        for case in &suite.cases {
            total += 1;
            if let Err(e) = run_case(case) {
                failures.push(format!(
                    "{}::{} [{}] — {e}",
                    suite.suite, case.name, case.func
                ));
            }
        }
    }
    seen_suites.sort();
    assert_eq!(
        seen_suites, REQUIRED_SUITES,
        "conformance corpus is not the expected set of suites (a dropped or renamed \
         vector file would otherwise pass silently with fewer cases)"
    );
    assert!(
        failures.is_empty(),
        "{} of {total} conformance cases FAILED:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("conformance: {total} cases green");
}

/// The bulk identicon suite: 20,000 `[seed, hash]` pairs in a compact form.
///
/// Separate from `conformance_corpus` because it uses a different file schema and
/// because a hash mismatch here is a different diagnosis — the float pipeline, not
/// an assembly or palette bug. See specs/003-rust-identicon/contracts/conformance-vectors.md.
#[test]
fn identicon_bulk_corpus() {
    let path = vectors_dir().join("identicon-bulk.json");
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) => panic!(
            "no bulk identicon corpus at {} ({e}) — regenerate with `npm run dump:vectors`",
            path.display()
        ),
    };
    let suite: BulkSuiteFile = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => panic!("{}: bad schema: {e}", path.display()),
    };

    assert!(
        suite.pairs.len() >= 20_000,
        "bulk corpus shrank to {} pairs — a truncated corpus would report green over \
         a fraction of the coverage SC-001 claims",
        suite.pairs.len()
    );

    let mut failed = 0_usize;
    let mut failures = Vec::new();
    for (seed, expected) in &suite.pairs {
        let got = vela_core::identicon::make_hash(seed);
        if got.as_str() != expected {
            failed += 1;
            if failures.len() < 10 {
                failures.push(format!(
                    "seed({} chars) {seed:?}: expected {expected}, got {got}",
                    seed.chars().count()
                ));
            }
        }
    }
    assert!(
        failed == 0,
        "{failed} of {} identicon bulk hashes FAILED (showing up to 10):\n{}",
        suite.pairs.len(),
        failures.join("\n")
    );
    println!("identicon bulk: {} hashes green", suite.pairs.len());
}

/// The exhaustive i18n suite: every key path × every locale, in a columnar
/// encoding (`{locales, keys, values}`).
///
/// Separate from `conformance_corpus` for the same reason `identicon_bulk_corpus`
/// is: a different file schema, and a different diagnosis when it goes red — a
/// mismatch here is the resolver or a catalog, not an option-handling bug.
///
/// Columnar rather than one case per resolution because it was measured at
/// 703,619 bytes against 1,557,623 for flat triples (2.21×) and 3,785,013 for full
/// `VectorCase` objects (5.38×) — `JSON.stringify(doc, null, 1)` spends five lines
/// of framing per triple and re-serialises the 1,141 key strings fifteen times.
///
/// See specs/004-rust-i18n/contracts/conformance-vectors.md.
#[test]
fn i18n_exhaustive_corpus() {
    #[derive(Deserialize)]
    struct ExhaustiveFile {
        locales: Vec<String>,
        keys: Vec<String>,
        /// locale -> one resolved string per entry of `keys`, index-aligned.
        values: std::collections::BTreeMap<String, Vec<String>>,
    }

    let path = vectors_dir().join("i18n-exhaustive.json");
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) => panic!(
            "no exhaustive i18n corpus at {} ({e}) — regenerate with `npm run dump:vectors`",
            path.display()
        ),
    };
    let suite: ExhaustiveFile = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => panic!("{}: bad schema: {e}", path.display()),
    };

    // Shrink guards. Without the per-column alignment check a truncated column
    // silently tests fewer keys and still reports green — the precise false
    // confidence the REQUIRED_SUITES pin exists to prevent, one level down.
    assert_eq!(
        suite.locales.len(),
        15,
        "locale set shrank to {} — the corpus must cover every shipped language",
        suite.locales.len()
    );
    assert!(
        suite.keys.len() >= 1141,
        "key inventory shrank to {} — expected at least the 1,141-path union",
        suite.keys.len()
    );

    let mut total = 0usize;
    let mut failed = 0usize;
    let mut failures = Vec::new();
    for lng in &suite.locales {
        let column = match suite.values.get(lng) {
            Some(c) => c,
            None => panic!("i18n-exhaustive: no value column for locale `{lng}`"),
        };
        assert_eq!(
            column.len(),
            suite.keys.len(),
            "column `{lng}` is not key-aligned ({} values for {} keys)",
            column.len(),
            suite.keys.len()
        );

        let engine = match i18n_engine(lng, lng, i18n::PluralMode::Cldr) {
            Ok(e) => e,
            Err(e) => {
                // One line per locale, not per key: 17,115 identical "not
                // implemented" lines would bury every other diagnosis.
                failed += column.len();
                total += column.len();
                if failures.len() < 20 {
                    failures.push(format!("{lng}: engine unavailable — {e}"));
                }
                continue;
            }
        };
        let opts = i18n::Options::default();
        for (i, key) in suite.keys.iter().enumerate() {
            total += 1;
            let got = engine.t(key, &opts);
            let ok = matches!(&got, Ok(s) if s == &column[i]);
            if !ok {
                failed += 1;
                if failures.len() < 20 {
                    failures.push(format!(
                        "{lng}::{key}: expected {:?}, got {:?}",
                        column[i], got
                    ));
                }
            }
        }
    }

    assert!(
        failed == 0,
        "{failed} of {total} i18n resolutions FAILED (showing up to 20):\n{}",
        failures.join("\n")
    );
    println!("i18n exhaustive: {total} resolutions green");
}

/// The exhaustive suite again, but every catalog built by `Catalog::from_json`
/// instead of compiled in — so all 17,115 resolutions run through `Values::Owned`.
///
/// This is the cheapest available proof that the two representations agree. They
/// have genuinely different code: `Static` indexes a `&'static [u16]` offset table
/// and consults the shared `IS_BRANCH` bitmap, while `Owned` carries `Vec<u32>`
/// offsets, its OWN branch bitmap and an `extra` overflow list for paths the
/// shared table never saw. A divergence between them would show up in production
/// as "the web build renders a different string from native", which no
/// single-representation suite can catch.
///
/// It also closes the D2 gap from the other side: the branch bitmap that T024's
/// unit tests exercise on a fixture is exercised here against the whole corpus.
#[test]
fn i18n_exhaustive_corpus_via_runtime_json() {
    #[derive(Deserialize)]
    struct ExhaustiveFile {
        locales: Vec<String>,
        keys: Vec<String>,
        values: std::collections::BTreeMap<String, Vec<String>>,
    }

    let path = vectors_dir().join("i18n-exhaustive.json");
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) => panic!("no exhaustive i18n corpus at {} ({e})", path.display()),
    };
    let suite: ExhaustiveFile = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => panic!("{}: bad schema: {e}", path.display()),
    };

    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/locales");
    // Merge a locale exactly as the generated asset does, so this reads the corpus
    // itself rather than a fixture that could drift away from it.
    let merged = |lng: &str| -> Vec<u8> {
        let mut out = serde_json::Map::new();
        let mut absorb = |p: PathBuf| {
            if let Ok(txt) = fs::read_to_string(&p) {
                if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&txt) {
                    out.extend(m);
                }
            }
        };
        absorb(corpus.join(format!("{lng}.json")));
        for ns in [
            "home",
            "send",
            "receive",
            "assets",
            "addToken",
            "tokenDetail",
            "history",
            "onboarding",
            "connect",
            "about",
            "clearSigning",
            "componentsTx",
            "componentsUi",
            "settingsModals",
            "contacts",
            "explore",
        ] {
            absorb(corpus.join(lng).join(format!("{ns}.json")));
        }
        Value::Object(out).to_string().into_bytes()
    };

    let mut failed = 0usize;
    let mut total = 0usize;
    let mut failures = Vec::new();

    for lng in &suite.locales {
        let column = match suite.values.get(lng) {
            Some(c) => c,
            None => panic!("no value column for `{lng}`"),
        };
        let mut engine =
            match i18n::I18n::new(match i18n::Catalog::from_json("en", &merged("en")) {
                Ok(c) => c,
                Err(e) => panic!("en must parse: {e}"),
            }) {
                Ok(e) => e,
                Err(e) => panic!("engine must construct: {e}"),
            };
        if lng != "en" {
            match i18n::Catalog::from_json(lng, &merged(lng)) {
                Ok(c) => {
                    engine.load_catalog(c);
                }
                Err(e) => panic!("{lng} must parse from the corpus: {e}"),
            }
        }
        engine.change_language(lng);

        let opts = i18n::Options::default();
        for (i, key) in suite.keys.iter().enumerate() {
            total += 1;
            let got = engine.t(key, &opts);
            if !matches!(&got, Ok(s) if s == &column[i]) {
                failed += 1;
                if failures.len() < 20 {
                    failures.push(format!(
                        "{lng}::{key}: expected {:?}, got {got:?}",
                        column[i]
                    ));
                }
            }
        }
    }
    assert!(
        failed == 0,
        "{failed} of {total} runtime-JSON resolutions FAILED (showing up to 20):\n{}",
        failures.join("\n")
    );
    println!("i18n exhaustive via runtime JSON: {total} resolutions green");
}
