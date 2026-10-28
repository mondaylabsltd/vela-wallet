//! Property tests: invariants that vectors can't pin (round-trips, idempotence,
//! never-panic on adversarial bytes). Grown module by module alongside
//! `conformance.rs`. Inventory: contracts/conformance-vectors.md §Invariants.

use proptest::prelude::*;
use vela_core::primitives as prim;

proptest! {
    // --- primitives: round-trips ------------------------------------------

    #[test]
    fn hex_round_trip(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let unprefixed = prim::to_hex(&data, false);
        let prefixed = prim::to_hex(&data, true);
        prop_assert_eq!(prim::from_hex(&unprefixed).ok(), Some(data.clone()));
        prop_assert_eq!(prim::from_hex(&prefixed).ok(), Some(data));
    }

    #[test]
    fn base64url_round_trip(data in proptest::collection::vec(any::<u8>(), 0..256)) {
        let encoded = prim::to_base64url(&data);
        // Output alphabet is url-safe, unpadded.
        prop_assert!(encoded.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        prop_assert_eq!(prim::from_base64url(&encoded).ok(), Some(data));
    }

    // --- primitives: idempotence ------------------------------------------

    #[test]
    fn checksum_idempotent(bytes in proptest::array::uniform20(any::<u8>())) {
        let addr = prim::to_hex(&bytes, true);
        let first = prim::checksum_address(&addr).ok();
        prop_assert!(first.is_some());
        let first = first.unwrap_or_default();
        let second = prim::checksum_address(&first).ok();
        prop_assert_eq!(Some(first), second);
    }

    #[test]
    fn quantity_idempotent(n in any::<u128>()) {
        let q1 = prim::to_quantity(&n.to_string()).ok();
        prop_assert!(q1.is_some());
        let q1 = q1.unwrap_or_default();
        let q2 = prim::to_quantity(&q1).ok();
        prop_assert_eq!(Some(q1.clone()), q2);
        // Canonical: lowercase, minimal.
        prop_assert!(q1.starts_with("0x"));
        prop_assert!(!q1[2..].starts_with('0') || q1 == "0x0");
    }

    // --- primitives: strict decoders never panic on arbitrary input --------

    #[test]
    fn from_hex_total(s in "\\PC*") {
        let _ = prim::from_hex(&s);
    }

    #[test]
    fn from_base64url_total(s in "\\PC*") {
        let _ = prim::from_base64url(&s);
    }

    #[test]
    fn to_quantity_total(s in "\\PC*") {
        let _ = prim::to_quantity(&s);
    }

    // --- abi: strict decoder is total (Ok or CoreError, never a panic) -----

    #[test]
    fn signature_parse_total(s in "\\PC*") {
        let _ = vela_core::abi::canonicalize_signature(&s);
        let _ = vela_core::abi::compute_selector(&s);
    }

    #[test]
    fn decode_calldata_total(
        sig_junk in "\\PC{0,64}",
        calldata in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        // Arbitrary signature text + arbitrary calldata: must never panic.
        let _ = vela_core::abi::decode_calldata(&sig_junk, &calldata);
        // Well-formed signature + arbitrary calldata: must never panic.
        let _ = vela_core::abi::decode_calldata(
            "exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)",
            &calldata,
        );
        let _ = vela_core::abi::decode_calldata("transfer(address to, uint256 value)", &calldata);
    }

    // --- webauthn: adversarial decoders are total ---------------------------

    #[test]
    fn attestation_extract_total(data in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = vela_core::webauthn::extract_attestation_public_key(&data);
    }

    #[test]
    fn der_parse_total(data in proptest::collection::vec(any::<u8>(), 0..128)) {
        let _ = vela_core::webauthn::der_signature_to_raw_low_s(&data);
    }

    #[test]
    fn low_s_idempotent_on_valid_der(seed in proptest::array::uniform32(1u8..=255), msg in proptest::array::uniform32(any::<u8>())) {
        use p256::ecdsa::signature::hazmat::PrehashSigner as _;
        if let Ok(key) = p256::ecdsa::SigningKey::from_slice(&seed) {
            let sig: p256::ecdsa::Signature = match key.sign_prehash(&msg) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
            let raw1 = vela_core::webauthn::der_signature_to_raw_low_s(sig.to_der().as_bytes());
            prop_assert!(raw1.is_ok());
            // Re-encoding the normalized signature and normalizing again is a fixpoint.
            if let Ok(raw) = raw1 {
                let sig2 = p256::ecdsa::Signature::from_slice(&raw);
                prop_assert!(sig2.is_ok());
                if let Ok(sig2) = sig2 {
                    let raw2 = vela_core::webauthn::der_signature_to_raw_low_s(sig2.to_der().as_bytes());
                    prop_assert_eq!(Ok(raw), raw2);
                }
            }
        }
    }
}

// Recovery round-trip is EC-heavy — keep the case count modest.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    /// RFC-6979 deterministic signing (no RNG) → two assertions from the same
    /// key must recover exactly that key; the same assertion twice must not.
    #[test]
    fn recovery_round_trip(
        seed in proptest::array::uniform32(1u8..=255),
        auth_a in proptest::collection::vec(any::<u8>(), 37..64),
        auth_b in proptest::collection::vec(any::<u8>(), 37..64),
        client_a in proptest::collection::vec(any::<u8>(), 1..96),
        client_b in proptest::collection::vec(any::<u8>(), 1..96),
    ) {
        use p256::ecdsa::signature::hazmat::PrehashSigner as _;
        let Ok(key) = p256::ecdsa::SigningKey::from_slice(&seed) else { return Ok(()); };

        let make = |auth: &[u8], client: &[u8]| -> Result<vela_core::WebAuthnAssertion, ()> {
            let prehash = vela_core::webauthn::webauthn_signing_hash(auth, client);
            let sig: p256::ecdsa::Signature = key.sign_prehash(&prehash).map_err(|_| ())?;
            Ok(vela_core::WebAuthnAssertion {
                authenticator_data: auth.to_vec(),
                client_data_json: client.to_vec(),
                signature_der: sig.to_der().as_bytes().to_vec(),
            })
        };
        let (Ok(a), Ok(b)) = (make(&auth_a, &client_a), make(&auth_b, &client_b)) else { return Ok(()); };
        prop_assume!(a.signature_der != b.signature_der);

        let recovered = vela_core::webauthn::recover_public_key_from_assertions(&a, &b);
        let point = key.verifying_key().to_sec1_point(false);
        let bytes = point.as_bytes();
        prop_assert_eq!(
            recovered,
            Ok(Some(vela_core::P256PublicKey { x: bytes[1..33].to_vec(), y: bytes[33..65].to_vec() }))
        );

        // Same assertion twice is deliberately never enough.
        prop_assert_eq!(vela_core::webauthn::recover_public_key_from_assertions(&a, &a), Ok(None));
    }
}

// ---------------------------------------------------------------------------
// identicon — invariants the corpus cannot express (spec 003-rust-identicon)
// ---------------------------------------------------------------------------

proptest! {
    /// No input panics, and the hash keeps its documented shape.
    #[test]
    fn identicon_hash_never_panics(seed in ".*") {
        let hash = vela_core::identicon::make_hash(&seed);
        prop_assert!((13..=17).contains(&hash.len()), "hash length {} out of range for {seed:?}", hash.len());
        // Stability: the same seed must always give the same hash.
        prop_assert_eq!(hash, vela_core::identicon::make_hash(&seed));
    }

    /// Arbitrary unicode, including astral planes and control characters.
    #[test]
    fn identicon_unicode_never_panics(seed in proptest::collection::vec(any::<char>(), 0..64)) {
        let seed: String = seed.into_iter().collect();
        let hash = vela_core::identicon::make_hash(&seed);
        prop_assert!((13..=17).contains(&hash.len()));

        // Params either succeed or return a typed error — never panic, never a
        // silently-wrong avatar.
        match vela_core::identicon::identicon_params(&seed) {
            Ok(params) => {
                // Every resolved colour is a real palette entry.
                prop_assert!(vela_core::identicon::COLORS.contains(&params.colors.main));
                prop_assert!(vela_core::identicon::BACKGROUND_COLORS.contains(&params.colors.background));
                prop_assert!(vela_core::identicon::COLORS.contains(&params.colors.accent));
                // Assembly is total once params exist.
                let svg = vela_core::identicon::assemble_svg_circular(&params);
                prop_assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>"));
                prop_assert!(!svg.contains("undefined"));
            }
            Err(e) => prop_assert_eq!(e.code(), "InvalidIdenticonSeed"),
        }
    }

    /// Section addressing lands in 1..=21 for ANY index, including i64::MIN.
    #[test]
    fn identicon_section_index_always_resolves(index in any::<i64>()) {
        use vela_core::identicon::{section_svg, Section};
        for section in [Section::Face, Section::Sides, Section::Top, Section::Bottom] {
            let svg = section_svg(section, index);
            prop_assert!(svg.is_ok(), "section {section:?} index {index} failed to resolve");
            prop_assert!(!svg.unwrap_or("").is_empty());
        }
    }

    /// `normalize_seed` is idempotent and never exceeds the cap.
    #[test]
    fn identicon_normalize_seed_idempotent(seed in ".*") {
        let once = vela_core::identicon::normalize_seed(&seed).into_owned();
        let twice = vela_core::identicon::normalize_seed(&once).into_owned();
        prop_assert_eq!(&once, &twice);
        prop_assert!(once.chars().map(char::len_utf16).sum::<usize>() <= vela_core::identicon::SEED_MAX_UTF16_LEN);
        prop_assert!(!once.bytes().any(|b| b.is_ascii_uppercase()));
    }

    /// ryu-js output stays within the buffer the hash tail assumes.
    #[test]
    fn identicon_decimal_form_fits_buffer(bits in any::<u64>()) {
        let x = f64::from_bits(bits);
        if x.is_finite() {
            let mut buf = ryu_js::Buffer::new();
            prop_assert!(buf.format(x).len() <= 32, "Number::toString({x}) exceeded the 32-byte assumption");
        }
    }
}

// ---------------------------------------------------------------------------
// i18n (spec 004-rust-i18n, FR-026)
// ---------------------------------------------------------------------------

mod i18n_props {
    use proptest::prelude::*;
    use vela_core::i18n::{Catalog, Count, I18n, Options, OwnedVar, Scratch, Var};

    fn engine(lng: &str) -> I18n {
        let en = match Catalog::embedded("en") {
            Ok(c) => c,
            Err(e) => unreachable!("i18n-en must be enabled: {e}"),
        };
        let mut i = match I18n::new(en) {
            Ok(i) => i,
            Err(e) => unreachable!("{e}"),
        };
        if lng != "en" {
            if let Ok(c) = Catalog::embedded(lng) {
                i.load_catalog(c);
            }
        }
        i.change_language(lng);
        i
    }

    /// Keys deliberately including the shapes that have historically broken
    /// resolvers: namespace colons, unbalanced braces, `$t()` self-reference,
    /// natural-language punctuation and a bare dot.
    fn any_key() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(String::new()),
            Just(".".to_owned()),
            Just("::".to_owned()),
            Just("common.cancel".to_owned()),
            Just("home".to_owned()),
            Just("{{".to_owned()),
            Just("$t(zz.self)".to_owned()),
            Just("Hello, world. How are you?".to_owned()),
            ".{0,40}",
            "[a-z]{1,8}\\.[a-z]{1,8}",
        ]
    }

    fn any_locale() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("en".to_owned()),
            Just("ru".to_owned()),
            Just("zh-TW".to_owned()),
            Just("zh_TW".to_owned()),
            Just("ZH".to_owned()),
            Just("es-AR".to_owned()),
            Just(String::new()),
            "[a-zA-Z]{0,5}(-[a-zA-Z]{0,4})?",
        ]
    }

    fn any_count() -> impl Strategy<Value = Option<Count>> {
        prop_oneof![
            Just(None),
            Just(Some(Count::Num(f64::NAN))),
            Just(Some(Count::Num(f64::INFINITY))),
            Just(Some(Count::Num(f64::NEG_INFINITY))),
            Just(Some(Count::Num(-0.0))),
            Just(Some(Count::Null)),
            Just(Some(Count::Object)),
            Just(Some(Count::Str("3".to_owned()))),
            Just(Some(Count::BigInt(5))),
            (-1e9f64..1e9).prop_map(|n| Some(Count::Num(n))),
        ]
    }

    proptest! {
        /// FR-008: no input panics. This is the property the whole typed-error
        /// design exists to make true — a panic inside `t()` on a signing screen
        /// takes the screen down, and the inputs here include the two shapes
        /// i18next itself throws on.
        #[test]
        fn t_never_panics(key in any_key(), lng in any_locale(), count in any_count()) {
            let e = engine("ru");
            let owned = vela_core::i18n::OwnedOptions {
                count,
                lng: Some(lng),
                vars: vec![("v".to_owned(), OwnedVar::Undefined)],
                ..Default::default()
            };
            let mut scratch = Scratch::default();
            let opts = owned.as_options(&mut scratch);
            // Either outcome is fine; not returning is not.
            let _ = e.t(&key, &opts);
            let _ = e.exists(&key, &opts);
        }

        /// Resolution is a pure function of (engine state, key, options).
        #[test]
        fn t_is_stable_across_repeated_calls(key in any_key(), n in -1000i32..1000) {
            let e = engine("fr");
            let vars = [("v", Var::Str("x"))];
            let opts = Options {
                count: Some(Count::Num(f64::from(n))),
                vars: &vars,
                ..Options::default()
            };
            let a = e.t(&key, &opts);
            let b = e.t(&key, &opts);
            prop_assert_eq!(a.is_ok(), b.is_ok());
            if let (Ok(x), Ok(y)) = (a, b) {
                prop_assert_eq!(x, y);
            }
        }

        /// FR-016: a locale that is not resident falls through to the pinned `en`
        /// catalog. Never a panic, never a partial read, and never a *third*
        /// language — the failure mode a mid-flight language switch could produce.
        #[test]
        fn non_resident_locale_falls_back_to_en(lng in any_locale()) {
            // Only `en` is resident; the engine's active language is something else.
            let en = match Catalog::embedded("en") {
                Ok(c) => c,
                Err(e) => unreachable!("{e}"),
            };
            let mut e = match I18n::new(en) {
                Ok(i) => i,
                Err(err) => unreachable!("{err}"),
            };
            e.change_language(&lng);
            let got = e.t("common.cancel", &Options::default());
            prop_assert_eq!(got.as_deref(), Ok("Cancel"));
            prop_assert!(e.is_resident("en"));
            prop_assert!(e.resident_locales().len() <= 2);
        }

        /// Residency is bounded by the type, not by discipline (FR-012/FR-013).
        #[test]
        fn residency_never_exceeds_two(seq in prop::collection::vec(any_locale(), 0..12)) {
            let mut e = engine("en");
            for lng in seq {
                if let Ok(c) = Catalog::embedded(&lng) {
                    e.load_catalog(c);
                }
                e.change_language(&lng);
                prop_assert!(e.resident_locales().len() <= 2);
                prop_assert!(e.resident_locales().contains(&"en"));
                let _ = e.release_catalog(&lng);
            }
        }

        /// Interpolation never loses or invents a placeholder delimiter on input
        /// it cannot substitute.
        #[test]
        fn interpolate_never_panics(template in ".{0,80}") {
            let vars = [("v", Var::Str("X"))];
            let opts = Options { vars: &vars, ..Options::default() };
            let _ = vela_core::i18n::interpolate(&template, &opts);
        }
    }

    /// The data-model's structural invariants over the generated tables. These are
    /// exhaustive rather than sampled — the tables are finite, so there is no
    /// reason to guess.
    #[test]
    fn generated_tables_hold_their_structural_invariants() {
        // Every locale's catalog must load, and every corpus key must resolve to a
        // non-empty string in every locale (falling through to `en` where absent).
        for lng in vela_core::i18n::SUPPORTED {
            let e = engine(lng);
            for key in [
                "common.cancel",
                "send.recipientCount_other",
                "home.totalBalance",
            ] {
                let got = e.t(key, &Options::default());
                assert!(
                    got.as_deref().is_ok_and(|s| !s.is_empty()),
                    "{lng}::{key} resolved to {got:?}"
                );
            }
            // A growth guard, no longer a correctness proxy. The offset width is
            // chosen PER LOCALE by the generator (`u16` while the blob fits
            // 64 KiB, `u32` past it), so a locale over 64 KiB is legal — `ru`
            // became one when spec 017 added the erase-this-device copy, and
            // pinning the corpus to the narrowest common width was what made the
            // largest locale everyone's ceiling. What must still fail loudly is a
            // corpus that doubled unnoticed; 128 KiB is close to twice today's
            // largest (`ru`, 71,637), and only the desktop build compiles these
            // arrays in at all. That every key still renders byte-for-byte in
            // every locale is proved exhaustively, not sampled, by
            // `i18n_exhaustive_corpus` in conformance.rs. Raised to 192 KiB on
            // 2026-10-28, when the exploratory test's fixes added ~70 phrases
            // to every locale and `ru` reached 132,469 bytes: still a guard
            // against a corpus that doubles unnoticed.
            if let Ok(c) = Catalog::embedded(lng) {
                assert!(
                    c.resident_bytes() < 196_608,
                    "{lng}: {} resident bytes — the corpus grew unexpectedly",
                    c.resident_bytes()
                );
            }
        }
    }
}

// --- safe: multi-passkey invariants ----------------------------------------

fn arb_key() -> impl Strategy<Value = vela_core::P256PublicKey> {
    (
        proptest::array::uniform32(any::<u8>()),
        proptest::array::uniform32(any::<u8>()),
    )
        .prop_map(|(x, y)| vela_core::P256PublicKey {
            x: x.to_vec(),
            y: y.to_vec(),
        })
}

proptest! {
    /// One key through the multi path is byte-identical to the legacy
    /// single-key path — for ANY 32-byte coordinates, not just the vectors.
    #[test]
    fn multi_one_key_equals_single(key in arb_key()) {
        let single = vela_core::safe::compute_safe_address(&key.x, &key.y).ok();
        prop_assert!(single.is_some());
        let multi = vela_core::safe::compute_safe_address_multi(std::slice::from_ref(&key)).ok();
        prop_assert_eq!(single, multi);
    }

    /// keys[1..] enumeration order never moves the wallet: any rotation plus
    /// end-swap of the rest yields the identical SafeAddressInfo.
    #[test]
    fn multi_rest_order_immaterial(
        keys in proptest::collection::vec(arb_key(), 3..=vela_core::safe::MAX_MULTI_KEYS),
        rot in any::<usize>(),
    ) {
        let mut keys = keys;
        let first = keys.remove(0);
        keys.sort_by(|a, b| a.x.cmp(&b.x).then_with(|| a.y.cmp(&b.y)));
        keys.dedup_by(|a, b| a.x == b.x && a.y == b.y);
        keys.retain(|k| !(k.x == first.x && k.y == first.y));
        prop_assume!(keys.len() >= 2);

        let mut ordered = vec![first.clone()];
        ordered.extend(keys.iter().cloned());
        let base = vela_core::safe::compute_safe_address_multi(&ordered).ok();
        prop_assert!(base.is_some());

        let len = keys.len();
        keys.rotate_left(rot % len);
        keys.swap(0, len - 1);
        let mut permuted = vec![first];
        permuted.extend(keys);
        prop_assert_eq!(base, vela_core::safe::compute_safe_address_multi(&permuted).ok());
    }

    /// A duplicated key is rejected wherever the copy lands in caller order.
    #[test]
    fn multi_duplicates_always_rejected(
        keys in proptest::collection::vec(arb_key(), 2..=6),
        pick in any::<usize>(),
        at in any::<usize>(),
    ) {
        let dup = keys[pick % keys.len()].clone();
        let mut with_dup = keys;
        let at = at % (with_dup.len() + 1);
        with_dup.insert(at, dup);
        prop_assert!(vela_core::safe::compute_safe_address_multi(&with_dup).is_err());
    }

    /// The cap is total: any list over MAX_MULTI_KEYS errs, valid or not.
    #[test]
    fn multi_cap_total(
        keys in proptest::collection::vec(
            arb_key(),
            vela_core::safe::MAX_MULTI_KEYS + 1..=vela_core::safe::MAX_MULTI_KEYS + 4,
        )
    ) {
        prop_assert!(vela_core::safe::compute_safe_address_multi(&keys).is_err());
    }

    /// Total function on adversarial coordinate lengths — Ok or CoreError,
    /// never a panic.
    #[test]
    fn multi_total_on_arbitrary_coords(
        coords in proptest::collection::vec(
            (
                proptest::collection::vec(any::<u8>(), 0..40),
                proptest::collection::vec(any::<u8>(), 0..40),
            ),
            0..4,
        )
    ) {
        let keys: Vec<vela_core::P256PublicKey> = coords
            .into_iter()
            .map(|(x, y)| vela_core::P256PublicKey { x, y })
            .collect();
        let _ = vela_core::safe::compute_safe_address_multi(&keys);
    }
}
