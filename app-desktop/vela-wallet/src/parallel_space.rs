//! The parallel space, on the desktop.
//!
//! The standing test environment every Vela client has: the REAL app — chains,
//! relay, registry, storage, every screen — with exactly one substitution. Where
//! a passkey would sign, `vela-core`'s fixed keyset signs. The web and Expo
//! clients enter it through a route; the desktop enters it through an
//! environment variable, because that is where its other dev switches live
//! (`VELA_GALLERY`, `VELA_PAGE`, `VELA_THEME`).
//!
//! ## Two gates, and a badge that answers to neither
//!
//! - **Compile-time**: the `dev-fixtures` cargo feature. Without it this module
//!   has no signer in it and [`active`] is a constant `false` — a release build
//!   carries neither the test scalars nor a path to them.
//! - **Runtime**: `VELA_PARALLEL_SPACE=1`. A dev build that does not set it is
//!   the real app, reaching real authenticators.
//!
//! The badge renders whenever the space is active, on every screen,
//! unconditionally. That is the fix for an audited P0 on another client: a
//! runtime-unlocked build once showed the fixture wallet with no marker — a
//! test wallet wearing the real one's face. Its colours are deliberately not
//! theme tokens; it must read as foreign to the product.
//!
//! ## What the seam replaces, and what it does not
//!
//! [`crate::executor::passkey::register`] and [`crate::executor::passkey::assert`]
//! consult [`active`] first and, when it holds, answer from the keyset without
//! touching a cable. Everything after the assertion is unchanged: the same
//! `Registration` and `Assertion` shapes, parsed by the same core, publishing
//! to the same registry, deriving the same Safe. A wallet created here is the
//! wallet the web's parallel space created — same credential ids, same keys,
//! same address.
//!
//! ## Which fixture answers
//!
//! A real authenticator would ask the person. The fixture decides by rule, so
//! a test is deterministic: a registration mints the first fixture NOT already
//! among the wallet's founding keys (the machine's own exclude list drives a
//! three-key creation through all three), and an assertion answers with the
//! credential the machine named, or — for the discoverable "who are you?"
//! ceremony — with fixture #1, unless `VELA_PARALLEL_SIGNER=n` prefers
//! another (the web's `vela.parallel.signWith(n)`).

use gpui::{Div, FontWeight, ParentElement as _, Styled as _, Window, div, px, rgb};

/// Is the parallel space active? Both gates, in order.
pub fn active() -> bool {
    #[cfg(feature = "dev-fixtures")]
    {
        std::env::var("VELA_PARALLEL_SPACE").as_deref() == Ok("1")
    }
    #[cfg(not(feature = "dev-fixtures"))]
    {
        false
    }
}

/// The marker, over whichever screen `root` is. A no-op when the space is
/// not active, so every root can call it without a branch of its own.
pub fn overlay(root: Div, _window: &Window) -> Div {
    if !active() {
        return root;
    }
    root.relative().child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .px(px(12.))
                    .pt(px(4.))
                    .pb(px(5.))
                    .rounded_b(px(8.))
                    // Not a token, on purpose — see the module note.
                    .bg(rgb(0x7c3aed))
                    .text_color(rgb(0xffffff))
                    .text_size(px(10.))
                    .child(div().font_weight(FontWeight::BOLD).child("PARALLEL SPACE"))
                    .child(div().child("fixture passkey · test wallet")),
            ),
    )
}

/// The software signer, behind the compile-time gate.
#[cfg(feature = "dev-fixtures")]
pub mod signer {
    use vela_core::app::{Assertion, Registration};
    use vela_core::dev_fixtures as fixtures;

    use crate::executor::passkey::PasskeyFailure;

    /// `VELA_PARALLEL_SIGNER=n` — which fixture answers a discoverable
    /// ceremony, or a multi-key allow list. Unset means fixture #1.
    pub fn preferred() -> Option<usize> {
        std::env::var("VELA_PARALLEL_SIGNER").ok()?.parse().ok()
    }

    /// `RegisterPasskey`, in the space: the first fixture not yet founding
    /// this wallet. The name the person typed labels the key row in the
    /// machine's own state, exactly as it does for a real key; the fixture's
    /// own name is not consulted.
    pub fn register(exclude_credential_ids: &[String]) -> Result<Registration, PasskeyFailure> {
        register_with(exclude_credential_ids).map_err(PasskeyFailure::other)
    }

    /// One assertion, in the space. `credential_id` is the machine's allow
    /// list (one entry, or none for the discoverable ceremony).
    pub fn assert(
        challenge: &[u8],
        credential_id: Option<&str>,
    ) -> Result<Assertion, PasskeyFailure> {
        assert_with(challenge, credential_id, preferred()).map_err(PasskeyFailure::other)
    }

    fn normalize(id: &str) -> String {
        id.strip_prefix("0x").unwrap_or(id).to_ascii_lowercase()
    }

    pub(super) fn register_with(exclude_credential_ids: &[String]) -> Result<Registration, String> {
        let excluded: Vec<String> = exclude_credential_ids
            .iter()
            .map(|id| normalize(id))
            .collect();
        let account = fixtures::accounts()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|account| !excluded.contains(&account.credential_id_hex))
            .ok_or_else(|| {
                format!(
                    "every fixture key already founds this wallet (the keyset holds {})",
                    fixtures::count()
                )
            })?;
        let minted = fixtures::build_registration(&account, fixtures::RP_ID, fixtures::ORIGIN)
            .map_err(|error| error.to_string())?;
        Ok(Registration {
            credential_id: minted.credential_id_hex,
            attestation_object_hex: minted.attestation_object_hex,
            client_data_json_hex: minted.client_data_json_hex,
            authenticator_attachment: fixtures::AUTHENTICATOR_ATTACHMENT.to_owned(),
            transports: fixtures::TRANSPORTS.to_owned(),
        })
    }

    pub(super) fn assert_with(
        challenge: &[u8],
        credential_id: Option<&str>,
        preferred: Option<usize>,
    ) -> Result<Assertion, String> {
        let allow: Vec<String> = credential_id.map(str::to_owned).into_iter().collect();
        let account =
            fixtures::resolve_signer(&allow, preferred).map_err(|error| error.to_string())?;
        let signed =
            fixtures::build_assertion(&account, challenge, fixtures::RP_ID, fixtures::ORIGIN)
                .map_err(|error| error.to_string())?;
        Ok(Assertion {
            credential_id: signed.credential_id_hex,
            signature_der_hex: signed.signature_der_hex,
            authenticator_data_hex: signed.authenticator_data_hex,
            client_data_json_hex: signed.client_data_json_hex,
            user_id_hex: None,
            authenticator_attachment: fixtures::AUTHENTICATOR_ATTACHMENT.to_owned(),
        })
    }
}

#[cfg(all(test, feature = "dev-fixtures"))]
mod tests {
    use super::signer::{assert_with, register_with};
    use vela_core::dev_fixtures as fixtures;
    use vela_core::primitives;
    use vela_core::webauthn::{extract_attestation_public_key, validate_client_data};
    use vela_core::{ClientDataKind, safe::parse_public_key};

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error}"))
    }

    fn ids() -> Vec<String> {
        ok(fixtures::accounts())
            .into_iter()
            .map(|account| account.credential_id_hex)
            .collect()
    }

    /// A three-key creation excludes what it already minted; the seam walks
    /// the keyset in order and refuses a fourth.
    #[test]
    fn a_registration_mints_the_first_fixture_not_yet_founding() {
        let ids = ids();
        assert_eq!(ok(register_with(&[])).credential_id, ids[0]);
        assert_eq!(ok(register_with(&ids[..1])).credential_id, ids[1]);
        // The machine's exclude list may carry a prefix or upper case.
        let shouted = vec![format!("0x{}", ids[0].to_uppercase()), ids[1].clone()];
        assert_eq!(ok(register_with(&shouted)).credential_id, ids[2]);
        assert!(register_with(&ids).is_err(), "no fourth fixture exists");
    }

    /// The registration is the shape the onboarding machine parses: the
    /// attestation yields the fixture's public key, and the client data
    /// passes the create-side rules.
    #[test]
    fn a_registration_parses_like_a_real_one() {
        let minted = ok(register_with(&[]));
        let attestation = ok(primitives::from_hex(&minted.attestation_object_hex));
        let parsed = ok(extract_attestation_public_key(&attestation));
        let expected = ok(parse_public_key(&ok(fixtures::account(0)).public_key_hex));
        assert_eq!(parsed, expected);
        let client_data = ok(primitives::from_hex(&minted.client_data_json_hex));
        ok(validate_client_data(
            ClientDataKind::Create,
            &client_data,
            &[],
        ));
        assert_eq!(minted.authenticator_attachment, "platform");
        assert_eq!(minted.transports, "internal");
    }

    /// An assertion answers with the credential the machine named, or with
    /// fixture #1 (or the preferred one) when it named none.
    #[test]
    fn an_assertion_answers_with_the_named_credential() {
        let ids = ids();
        let challenge = [0xab_u8; 32];
        let pinned = ok(assert_with(&challenge, Some(&ids[1]), None));
        assert_eq!(pinned.credential_id, ids[1]);
        let discoverable = ok(assert_with(&challenge, None, None));
        assert_eq!(discoverable.credential_id, ids[0]);
        let preferred = ok(assert_with(&challenge, None, Some(2)));
        assert_eq!(preferred.credential_id, ids[2]);

        let auth_data = ok(primitives::from_hex(&pinned.authenticator_data_hex));
        let client_data = ok(primitives::from_hex(&pinned.client_data_json_hex));
        ok(validate_client_data(
            ClientDataKind::Get,
            &client_data,
            &auth_data,
        ));
        assert_eq!(pinned.authenticator_attachment, "platform");
        assert!(pinned.user_id_hex.is_none());
    }
}
