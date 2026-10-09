//! The words of a key-method row — "This device", "Phone or tablet", "USB
//! security key" and the line under each — decided once here for every
//! shell's create chooser and sign-in chooser (087 F01, F02).
//!
//! Three rows and no fourth (spec 102): the Trusted Signer is where a person
//! reviews and signs, not a place a key lives, so its words moved to the venue
//! rows ([`venue_words`]) — "Review and sign in Vela", "Review and sign on a
//! trusted signing page", and the choosers' "Use a trusted signing page" (D6:
//! Vela's official page, or one the person deployed).
//!
//! Two lines depend on where the row is drawn (083 W16, which fixed them on the
//! desktop alone):
//!
//! - **"This device"** names what unlocks a passkey on the device in hand. An
//!   iPhone 11 read "Touch ID or Windows Hello" (087 F01): it has Face ID, and
//!   no phone has Windows Hello. A shell that can tell names the authenticator
//!   by its own product name, which no catalog translates (Face ID, Touch ID,
//!   Windows Hello); one that cannot — Android, a browser, Linux, an Apple
//!   device without biometrics — draws the corpus's line, which names the
//!   family rather than a product.
//! - **"Phone or tablet"** creates a key on a nearby device in the create
//!   chooser; the sign-in chooser creates nothing (087 F02, "扫码，用附近设备创建"
//!   in 登录), so its line is the scan itself.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::KeyMethod;

/// Which chooser a key-method row is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum KeyChooser {
    /// Minting a founding key — the create flow's "add a key" list.
    Create,
    /// Finding an existing key — the sign-in sheet.
    SignIn,
}

/// What unlocks a passkey on "this device", as far as the shell can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DeviceUnlock {
    /// An iPhone or iPad with Face ID.
    FaceId,
    /// A Mac, iPhone or iPad with Touch ID.
    TouchId,
    /// A Windows PC.
    WindowsHello,
    /// Anything the shell cannot name: an Android phone (fingerprint, face or
    /// screen lock, whichever the person set up), a browser, a Linux desktop,
    /// an Apple device with no biometrics.
    Other,
}

impl DeviceUnlock {
    /// The authenticator's own name, verbatim in every language — `None` when
    /// there is no single product to name.
    #[must_use]
    pub const fn product_name(self) -> Option<&'static str> {
        match self {
            Self::FaceId => Some("Face ID"),
            Self::TouchId => Some("Touch ID"),
            Self::WindowsHello => Some("Windows Hello"),
            Self::Other => None,
        }
    }
}

/// The line under a key-method row's title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum MethodLine {
    /// A corpus key, translated by the shell.
    Key(&'static str),
    /// A product name, drawn as it is — never looked up, never translated.
    Name(&'static str),
}

/// A key-method row's two lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MethodWords {
    /// The title's corpus key.
    pub title_key: &'static str,
    pub line: MethodLine,
}

impl MethodWords {
    /// The line's corpus key, when it is one.
    #[must_use]
    pub const fn line_key(&self) -> Option<&'static str> {
        match self.line {
            MethodLine::Key(key) => Some(key),
            MethodLine::Name(_) => None,
        }
    }

    /// The line's product name, when it is one — drawn as it is.
    #[must_use]
    pub const fn line_name(&self) -> Option<&'static str> {
        match self.line {
            MethodLine::Name(name) => Some(name),
            MethodLine::Key(_) => None,
        }
    }
}

/// "Scan a code" — the phone row's line wherever the phone creates nothing:
/// the sign-in chooser, and the line under a phone's QR in a sign-in.
pub const SCAN_LINE_KEY: &str = "explore.scan";

impl KeyMethod {
    /// This method's row, as `chooser` draws it on a device unlocked by
    /// `unlock`. The title never changes; see the module doc for the two lines
    /// that do.
    #[must_use]
    pub const fn words(self, chooser: KeyChooser, unlock: DeviceUnlock) -> MethodWords {
        let (title_key, line) = match self {
            Self::Platform => (
                "onboarding.create.methodPlatformTitle",
                match unlock.product_name() {
                    Some(name) => MethodLine::Name(name),
                    None => MethodLine::Key("onboarding.create.methodPlatformBody"),
                },
            ),
            Self::Hybrid => (
                "onboarding.create.methodHybridTitle",
                match chooser {
                    KeyChooser::Create => MethodLine::Key("onboarding.create.methodHybridBody"),
                    KeyChooser::SignIn => MethodLine::Key(SCAN_LINE_KEY),
                },
            ),
            Self::SecurityKey => (
                "onboarding.create.methodSecurityKeyTitle",
                MethodLine::Key("onboarding.create.methodSecurityKeyBody"),
            ),
        };
        MethodWords { title_key, line }
    }
}

/// [`KeyMethod::words`] over wire names (`"platform"`, `"sign_in"`,
/// `"face_id"`…), for the shells that hold their own copies of the enums.
/// `None` for a name this core does not know.
#[must_use]
pub fn method_words(method: &str, chooser: &str, unlock: &str) -> Option<MethodWords> {
    fn wire<T: for<'de> Deserialize<'de>>(name: &str) -> Option<T> {
        serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
    }
    let method: KeyMethod = wire(method)?;
    Some(method.words(wire(chooser)?, wire(unlock)?))
}

/// [`method_words`] as the flat JSON every non-Rust shell reads:
/// `{"title_key", "line_key", "line_name"}`, exactly one of the last two set.
#[must_use]
pub fn method_words_json(method: &str, chooser: &str, unlock: &str) -> Option<String> {
    let words = method_words(method, chooser, unlock)?;
    serde_json::to_string(&serde_json::json!({
        "title_key": words.title_key,
        "line_key": words.line_key(),
        "line_name": words.line_name(),
    }))
    .ok()
}

/// A venue row: the two venues of "Where you review and sign", and the
/// choosers' advanced entry that creates or signs into a wallet on a trusted
/// signing page — Vela's official one, or one the person deployed (spec 102,
/// D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum VenueRow {
    /// Vela's own signing sheet.
    InVela,
    /// A trusted signing page.
    Page,
    /// The create / sign-in choosers' "Use a trusted signing page" — its list
    /// is Vela's official signing page and the self-hosted ones; a wallet made
    /// on a self-hosted page keeps its keys on that page's domain.
    /// [`venue_words`] also reads it as `own_page`, its name before D6.
    SigningPage,
}

/// A venue row's two lines, as corpus keys.
#[must_use]
pub const fn venue_row_words(row: VenueRow) -> MethodWords {
    let (title_key, line) = match row {
        VenueRow::InVela => ("settings.venue.inVela", "settings.venue.inVelaBody"),
        VenueRow::Page => ("settings.venue.page", "settings.venue.pageBody"),
        VenueRow::SigningPage => (
            "onboarding.create.signingPageTitle",
            "onboarding.create.signingPageBody",
        ),
    };
    MethodWords {
        title_key,
        line: MethodLine::Key(line),
    }
}

/// [`venue_row_words`] over a wire name (`"in_vela"`, `"page"`,
/// `"signing_page"`, or `"own_page"` as before D6); `None` for one this core
/// does not know.
#[must_use]
pub fn venue_words(row: &str) -> Option<MethodWords> {
    // D6 renamed the chooser's entry; a shell built before still asks for it
    // by its old name.
    let row = if row == "own_page" {
        "signing_page"
    } else {
        row
    };
    let row: VenueRow = serde_json::from_value(serde_json::Value::String(row.to_owned())).ok()?;
    Some(venue_row_words(row))
}

/// [`venue_words`] as the same flat JSON [`method_words_json`] answers.
#[must_use]
pub fn venue_words_json(row: &str) -> Option<String> {
    let words = venue_words(row)?;
    serde_json::to_string(&serde_json::json!({
        "title_key": words.title_key,
        "line_key": words.line_key(),
        "line_name": words.line_name(),
    }))
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [KeyMethod; 3] = KeyMethod::ALL;
    const UNLOCKS: [DeviceUnlock; 4] = [
        DeviceUnlock::FaceId,
        DeviceUnlock::TouchId,
        DeviceUnlock::WindowsHello,
        DeviceUnlock::Other,
    ];

    /// 087 F01: "this device" names the authenticator the device has — never
    /// a product from another platform — or, when the shell cannot tell, the
    /// corpus's family line.
    #[test]
    fn this_device_names_only_its_own_authenticator() {
        for chooser in [KeyChooser::Create, KeyChooser::SignIn] {
            let line = |unlock| KeyMethod::Platform.words(chooser, unlock).line;
            assert_eq!(line(DeviceUnlock::FaceId), MethodLine::Name("Face ID"));
            assert_eq!(line(DeviceUnlock::TouchId), MethodLine::Name("Touch ID"));
            assert_eq!(
                line(DeviceUnlock::WindowsHello),
                MethodLine::Name("Windows Hello")
            );
            assert_eq!(
                line(DeviceUnlock::Other),
                MethodLine::Key("onboarding.create.methodPlatformBody")
            );
        }
    }

    /// 087 F02: the sign-in chooser's phone row creates nothing — its line is
    /// the scan; the create chooser keeps "create it on a nearby device".
    #[test]
    fn the_sign_in_phone_row_never_says_create() {
        for unlock in UNLOCKS {
            assert_eq!(
                KeyMethod::Hybrid.words(KeyChooser::SignIn, unlock).line,
                MethodLine::Key(SCAN_LINE_KEY)
            );
            assert_eq!(
                KeyMethod::Hybrid.words(KeyChooser::Create, unlock).line,
                MethodLine::Key("onboarding.create.methodHybridBody")
            );
        }
    }

    /// Titles never move, and the other two rows read the same in both
    /// choosers on every device.
    #[test]
    fn titles_and_the_other_rows_are_the_same_everywhere() {
        for method in ALL {
            for unlock in UNLOCKS {
                let create = method.words(KeyChooser::Create, unlock);
                let sign_in = method.words(KeyChooser::SignIn, unlock);
                assert_eq!(create.title_key, sign_in.title_key, "{method:?}");
                if method == KeyMethod::SecurityKey {
                    assert_eq!(create, sign_in, "{method:?}");
                }
            }
        }
    }

    /// Every key a row can name is a string in all 15 languages, so no
    /// shell ever draws a bare key.
    #[test]
    fn every_key_is_in_every_language() {
        let mut keys = Vec::new();
        for method in ALL {
            for chooser in [KeyChooser::Create, KeyChooser::SignIn] {
                for unlock in UNLOCKS {
                    let words = method.words(chooser, unlock);
                    keys.push(words.title_key);
                    if let MethodLine::Key(key) = words.line {
                        keys.push(key);
                    }
                }
            }
        }
        for row in [VenueRow::InVela, VenueRow::Page, VenueRow::SigningPage] {
            let words = venue_row_words(row);
            keys.push(words.title_key);
            if let MethodLine::Key(key) = words.line {
                keys.push(key);
            }
        }
        keys.sort_unstable();
        keys.dedup();
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/locales");
        let mut checked = 0;
        for entry in std::fs::read_dir(root).unwrap_or_else(|e| unreachable!("locales: {e}")) {
            let dir = entry.unwrap_or_else(|e| unreachable!("entry: {e}")).path();
            if !dir.is_dir() {
                continue;
            }
            for key in &keys {
                let mut parts = key.split('.');
                let namespace = parts.next().unwrap_or_default();
                // A namespace has its own file, or lives in the locale's
                // top-level one (`settings` does: `<lang>.json`).
                let own = dir.join(format!("{namespace}.json"));
                let path = if own.exists() {
                    own
                } else {
                    dir.with_extension("json")
                };
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| unreachable!("{}: {e}", path.display()));
                let mut node: serde_json::Value =
                    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("json: {e}"));
                node = node[namespace].clone();
                for part in parts {
                    node = node[part].clone();
                }
                assert!(
                    !node.as_str().unwrap_or("").trim().is_empty(),
                    "{}: {key} is missing",
                    dir.display()
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 15, "every language checked");
    }

    /// Spec 102: three places and no fourth. The Trusted Signer's wire name
    /// is not a key method any more — a shell that still asks for its row
    /// gets nothing to draw, rather than a place a key cannot live.
    #[test]
    fn there_is_no_fourth_row() {
        for chooser in ["create", "sign_in"] {
            assert_eq!(method_words("trusted_signer", chooser, "other"), None);
        }
        assert_eq!(KeyMethod::ALL.len(), 3);
    }

    /// The key label's place (`signing_venue::place_title_key`, which needs no
    /// `crux`) names each place exactly as the choosers' rows do.
    #[test]
    fn a_key_label_names_its_place_as_the_chooser_does() {
        for method in ALL {
            assert_eq!(
                crate::signing_venue::place_title_key(method.name()),
                method
                    .words(KeyChooser::Create, DeviceUnlock::Other)
                    .title_key
            );
        }
    }

    #[test]
    fn the_venue_rows_have_their_own_words() {
        assert_eq!(
            venue_words("in_vela").map(|w| w.title_key),
            Some("settings.venue.inVela")
        );
        assert_eq!(
            venue_words("page").map(|w| w.line),
            Some(MethodLine::Key("settings.venue.pageBody"))
        );
        assert_eq!(
            venue_words_json("signing_page").as_deref(),
            Some(
                r#"{"line_key":"onboarding.create.signingPageBody","line_name":null,"title_key":"onboarding.create.signingPageTitle"}"#
            )
        );
        // D6 renamed the row; a shell still asking by its old name gets the
        // same words.
        assert_eq!(venue_words("own_page"), venue_words("signing_page"));
        assert_eq!(venue_words("trusted_signer"), None);
    }

    #[test]
    fn wire_names_read_and_unknown_ones_do_not() {
        assert_eq!(
            method_words("platform", "sign_in", "face_id"),
            Some(KeyMethod::Platform.words(KeyChooser::SignIn, DeviceUnlock::FaceId))
        );
        assert_eq!(
            method_words("hybrid", "sign_in", "other").map(|w| w.line),
            Some(MethodLine::Key(SCAN_LINE_KEY))
        );
        assert_eq!(
            method_words_json("platform", "create", "touch_id").as_deref(),
            Some(
                r#"{"line_key":null,"line_name":"Touch ID","title_key":"onboarding.create.methodPlatformTitle"}"#
            )
        );
        assert_eq!(method_words("carrier_pigeon", "create", "other"), None);
        assert_eq!(method_words("platform", "browse", "other"), None);
        assert_eq!(method_words("platform", "create", "iris"), None);
    }
}
