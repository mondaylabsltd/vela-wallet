//! Localization access — the only module that touches `vela_core::i18n`
//! (spec 007 FR-009).
//!
//! Desktop compiles all 15 catalogs in (`i18n-all`, research.md D6), so language
//! selection is synchronous and infallible: the engine always holds the pinned
//! `en` fallback, plus the resolved locale's catalog when that locale isn't `en`.

use gpui::SharedString;
use vela_core::i18n::{Catalog, Count, I18n, Options, Var};

pub struct Loc {
    engine: I18n,
}

impl Loc {
    /// Build the engine for the language in force: the `VELA_LANG` pin, the
    /// language chosen in Settings (spec 072), else `LC_ALL` → `LC_MESSAGES`
    /// → `LANG` → `en` (spec 007 FR-007) — resolved through the same ladder
    /// i18next uses (`resolve_language`).
    pub fn from_env() -> Self {
        Self::for_language(&requested_tag())
    }

    /// The engine for one requested tag, whatever the environment says —
    /// what a test that asserts a language's own words builds.
    #[cfg(test)]
    pub fn for_tag(requested: &str) -> Self {
        Self::for_language(requested)
    }

    /// The engine for one requested language tag, resolved the same way —
    /// what [`Self::from_env`] builds for the language in force.
    pub(crate) fn for_language(requested: &str) -> Self {
        let mut engine = match I18n::embedded() {
            Ok(engine) => engine,
            // `i18n-en` is a compile-time feature of this binary; construction
            // can only fail if the crate was built without it, which the
            // dependency declaration makes impossible. Render keys as-is rather
            // than crash the welcome screen if that invariant is ever broken.
            Err(_) => return Self::key_echo(),
        };

        let state = engine.change_language(requested);
        if let Some(resolved) = state.resolved_language.as_deref()
            && resolved != "en"
            && let Ok(catalog) = Catalog::embedded(resolved)
        {
            engine.load_catalog(catalog);
        }
        Self { engine }
    }

    /// Every language the app ships, each its own engine: for a test that
    /// must hold in all of them, whatever language the machine running it is
    /// set to.
    #[cfg(test)]
    pub(crate) fn every_language() -> impl Iterator<Item = (&'static str, Self)> {
        vela_core::i18n::SUPPORTED
            .into_iter()
            .map(|tag| (tag, Self::for_language(tag)))
    }

    /// An engine with only the `en` catalog missing-in-action: `t()` echoes
    /// keys. Never reached in a correctly built binary (see `from_env`).
    fn key_echo() -> Self {
        // Catalog::from_json with an empty object gives a valid empty engine.
        let empty = Catalog::from_json("en", b"{}").and_then(I18n::new);
        match empty {
            Ok(engine) => Self { engine },
            Err(_) => unreachable!("empty en catalog construction is infallible"),
        }
    }

    /// Resolve `key` with default options. A missing key echoes the key —
    /// i18next's contract — which the visual checks treat as a failure signal
    /// (SC-004), not something to hide.
    pub fn t(&self, key: &str) -> SharedString {
        self.engine
            .t(key, &Options::default())
            .unwrap_or_else(|_| key.to_owned())
            .into()
    }

    /// The core's compact relative time — "now", "2m", "3h", a weekday, a
    /// date — for an instant in epoch milliseconds, in this machine's zone and
    /// the person's date format (issue #443's "Updated {{ago}}").
    pub fn relative_time(&self, at_ms: f64, now_ms: f64) -> SharedString {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "epoch milliseconds, well inside i64"
        )]
        let (at_sec, now) = ((at_ms / 1000.0).floor() as i64, now_ms as i64);
        self.engine
            .format_relative_time(
                at_sec,
                now,
                crate::executor::local_utc_offset_minutes(),
                crate::executor::format_prefs::current().date,
            )
            .unwrap_or_default()
            .into()
    }

    /// `t` with numeric interpolation variables (`{{seconds}}`, `{{count}}`,
    /// `{{current}}/{{total}}` — spec 014 flow copy). Numbers only and no
    /// `count` plural option: interpolation stays a pure text substitution,
    /// which is all the flow keys use.
    pub fn t_vars(&self, key: &str, vars: &[(&str, f64)]) -> SharedString {
        let vars: Vec<(&str, Var<'_>)> = vars.iter().map(|(k, v)| (*k, Var::Num(*v))).collect();
        let opts = Options {
            vars: &vars,
            ..Options::default()
        };
        self.engine
            .t(key, &opts)
            .unwrap_or_else(|_| key.to_owned())
            .into()
    }

    /// `t` for a PLURAL key: `count` chooses the form — `_one`, `_few`,
    /// `_many` or `_other`, by the language's CLDR rule, in the core — and
    /// fills `{{count}}`.
    ///
    /// [`Self::t_vars`] cannot stand in for this. It fills `{{count}}` as a
    /// plain variable and never selects a form, so a plural key handed to it
    /// has no value at all (issue #409: the done screen's "Any of your 1 keys"
    /// was one sentence for every count; once it became plural, the variable
    /// route would have echoed the key). Never pick the suffix here either:
    /// `count == 1` is not Russian's rule, nor Chinese's.
    pub fn t_count(&self, key: &str, count: usize) -> SharedString {
        // Founding sets are capped at 7; the clamp only keeps the conversion
        // lossless without a cast.
        let count = u32::try_from(count).map_or(f64::from(u32::MAX), f64::from);
        let opts = Options {
            count: Some(Count::Num(count)),
            ..Options::default()
        };
        self.engine
            .t(key, &opts)
            .unwrap_or_else(|_| key.to_owned())
            .into()
    }

    /// `t` with ONE text variable.
    ///
    /// Separate from [`Self::t_vars`] rather than folded into it because the
    /// only strings that take text are the ones naming a piece of hardware —
    /// the product string a USB device reports about itself. Everything else
    /// interpolates numbers, and keeping the two apart means a caller cannot
    /// accidentally put a device's own words where a count belongs.
    pub fn t_text(&self, key: &str, name: &str, value: &str) -> SharedString {
        let vars = [(name, Var::Str(value))];
        let opts = Options {
            vars: &vars,
            ..Options::default()
        };
        self.engine
            .t(key, &opts)
            .unwrap_or_else(|_| key.to_owned())
            .into()
    }

    /// `t` with SEVERAL text variables.
    ///
    /// [`Self::t_text`]'s reason for existing — text variables name hardware,
    /// and only one at a time — stopped being the whole truth in spec 075: the
    /// sentence under a narrowed key picker names a page, that page's relying
    /// party and the wallet's, and they are three facts of one sentence. Still
    /// no `count` option, so this stays pure text substitution.
    pub fn t_texts(&self, key: &str, vars: &[(&str, &str)]) -> SharedString {
        let vars: Vec<(&str, Var<'_>)> = vars.iter().map(|(k, v)| (*k, Var::Str(v))).collect();
        let opts = Options {
            vars: &vars,
            ..Options::default()
        };
        self.engine
            .t(key, &opts)
            .unwrap_or_else(|_| key.to_owned())
            .into()
    }

    /// The BCP-47 tag actually resolved (used only for logging).
    pub fn language(&self) -> &str {
        self.engine.language()
    }
}

/// Every language the app ships, for [`Loc::for_language`].
#[cfg(test)]
pub(crate) const LANGUAGES: [&str; 15] = vela_core::i18n::SUPPORTED;

/// The tag the strings resolve from: the `VELA_LANG` pin, else the language
/// the person chose in Settings (spec 072: `vela.language`), else what the
/// system asks for ([`system_language`]).
pub(crate) fn requested_tag() -> String {
    pick_tag(
        pinned_tag(),
        crate::executor::preferences::pinned_language(),
        system_language,
    )
}

/// The environment's locale variables, in POSIX precedence.
const ENV_LOCALE: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

/// The machine's locale: `VELA_LANG` → `LC_ALL` → `LC_MESSAGES` → `LANG` →
/// `en`, as a BCP-47-shaped tag (`zh_CN.UTF-8` → `zh-CN`). What the format
/// presets' "Automatic" and the display currency's region read (spec 038
/// #E3) — the web's `auto` formats read the platform too, never the app's
/// chosen language. The STRINGS' "follow the system" is
/// [`system_language`], which on macOS asks the system itself.
pub(crate) fn system_tag() -> String {
    pinned_tag()
        .or_else(|| env_tag(&ENV_LOCALE))
        .unwrap_or_else(|| "en".to_owned())
}

/// The shipped language "follow the system" means (spec 095).
///
/// macOS: the person's preferred languages (System Settings → Language &
/// Region), through the core's one rule — the same `system_language` iOS and
/// Android use. A Finder or Dock launch carries no `LANG`, so reading the
/// environment there gave English to everybody who had not picked a language
/// in Settings. Windows and Linux: the environment chain, resolved by the
/// engine, exactly as before.
pub(crate) fn system_language() -> String {
    language_from(pinned_tag(), preferred_languages(), || env_tag(&ENV_LOCALE))
}

/// [`system_language`] with the platform's answers handed in: the developer
/// pin, the system's preferred languages (`None` where the platform keeps
/// none — Windows, Linux), and the environment chain.
fn language_from(
    pin: Option<String>,
    preferred: Option<Vec<String>>,
    env: impl FnOnce() -> Option<String>,
) -> String {
    if let Some(pin) = pin {
        return vela_core::i18n::resolve_language(&pin).language;
    }
    if let Some(preferred) = preferred.filter(|list| !list.is_empty()) {
        return vela_core::i18n::system_language(&preferred).to_owned();
    }
    vela_core::i18n::resolve_language(&env().unwrap_or_else(|| "en".to_owned())).language
}

/// `CFLocaleCopyPreferredLanguages`: the person's languages, most preferred
/// first, as BCP-47 (`zh-Hans-CN`). Allowed in the App Sandbox.
#[cfg(target_os = "macos")]
fn preferred_languages() -> Option<Vec<String>> {
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;

    // SAFETY: a Copy-rule function: the array is owned here and released by
    // `wrap_under_create_rule`'s drop; null is checked first.
    let raw = unsafe { core_foundation_sys::locale::CFLocaleCopyPreferredLanguages() };
    if raw.is_null() {
        return None;
    }
    let array: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(raw) };
    let languages: Vec<String> = array
        .iter()
        .filter_map(|item| item.downcast::<CFString>().map(|text| text.to_string()))
        .collect();
    (!languages.is_empty()).then_some(languages)
}

#[cfg(not(target_os = "macos"))]
fn preferred_languages() -> Option<Vec<String>> {
    None
}

/// `VELA_LANG` — a developer build's language pin (spec 095: release builds
/// do not read it).
fn pinned_tag() -> Option<String> {
    crate::dev_env::var!("VELA_LANG")
        .filter(|v| !v.is_empty())
        .map(|raw| normalize_posix_tag(&raw))
}

fn env_tag(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .map(|raw| normalize_posix_tag(&raw))
}

/// A developer's pin, then the person's choice, then the machine.
fn pick_tag(
    pin: Option<String>,
    chosen: Option<String>,
    system: impl FnOnce() -> String,
) -> String {
    pin.or(chosen).unwrap_or_else(system)
}

/// `zh_CN.UTF-8` → `zh-CN`; strips the encoding suffix and maps `_` → `-`.
/// `resolve_language` takes it from there (including `C`/`POSIX` → `en` via
/// its unsupported-tag fallback).
fn normalize_posix_tag(raw: &str) -> String {
    let no_encoding = raw.split('.').next().unwrap_or(raw);
    no_encoding.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 095: on macOS "follow the system" is the person's preferred
    /// languages through the core's rule — whatever the environment says.
    #[test]
    fn the_systems_preferred_languages_pick_the_language() {
        let preferred = |tags: &[&str]| Some(tags.iter().map(|t| (*t).to_owned()).collect());
        let no_env = || None;
        for (tags, want) in [
            (&["zh-Hans-CN"][..], "zh"),
            (&["zh-Hant-HK"], "zh-HK"),
            (&["pt-BR"], "pt-BR"),
            (&["fr-CA"], "fr"),
            (&["ar-SA"], "en"),
            (&["ar-SA", "ja-JP"], "ja"),
        ] {
            assert_eq!(
                language_from(None, preferred(tags), no_env),
                want,
                "{tags:?}"
            );
        }
        assert_eq!(
            language_from(None, preferred(&["zh-Hans-CN"]), || Some(
                "en-US".to_owned()
            )),
            "zh",
            "the system's list, not LANG, on macOS"
        );
        assert_eq!(
            language_from(Some("ja".to_owned()), preferred(&["zh-Hans-CN"]), no_env),
            "ja",
            "a developer's pin first"
        );
    }

    /// Spec 095: where the platform keeps no preferred list (Windows, Linux)
    /// or it is empty, the environment chain resolves as it always has.
    #[test]
    fn without_a_preferred_list_the_environment_decides_as_before() {
        for (env, want) in [
            (Some("zh-TW"), "zh-TW"),
            (Some("zh-CN"), "zh"),
            (Some("pt-PT"), "pt-BR"),
            (Some("de-DE"), "de"),
            (Some("C"), "en"),
            (None, "en"),
        ] {
            let from_env = || env.map(str::to_owned);
            assert_eq!(language_from(None, None, from_env), want, "{env:?}");
            assert_eq!(
                language_from(None, Some(Vec::new()), from_env),
                want,
                "{env:?}"
            );
            // What the strings resolved to before spec 095: the raw tag handed
            // to the engine.
            let before = Loc::for_tag(env.unwrap_or("en")).language().to_owned();
            assert_eq!(before, want, "{env:?}");
        }
        #[cfg(not(target_os = "macos"))]
        assert_eq!(preferred_languages(), None);
    }

    /// The live macOS read answers something the core can resolve.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_reports_preferred_languages() {
        let preferred = preferred_languages().expect("macOS keeps a preferred-language list");
        assert!(!preferred.is_empty());
        let _ = vela_core::i18n::system_language(&preferred);
    }

    /// Spec 095: the Mac bundle declares exactly the corpus's locales, as the
    /// core names them for Apple — no more (a language the app cannot speak),
    /// no fewer (one the store would not list).
    #[test]
    fn the_bundle_declares_the_corpus_locales() {
        let plist = include_str!("../packaging/macos/Info.plist.in");
        let key = plist
            .find("<key>CFBundleLocalizations</key>")
            .unwrap_or_else(|| unreachable!("Info.plist.in declares no localizations"));
        let array = &plist[key..];
        let array = &array[..array.find("</array>").unwrap_or(array.len())];
        let declared: Vec<&str> = array
            .split("<string>")
            .skip(1)
            .filter_map(|rest| rest.split("</string>").next())
            .collect();
        assert_eq!(declared, vela_core::i18n::apple_localizations());
        assert!(plist.contains("<key>CFBundleDevelopmentRegion</key> <string>en</string>"));
    }

    /// Every key the welcome screen renders, including the 13 added by spec 007.
    const WELCOME_KEYS: [&str; 16] = [
        "onboarding.welcome.desktopTagline",
        "onboarding.welcome.createWallet",
        "onboarding.welcome.alreadyHaveWallet",
        "onboarding.welcome.featureNoMnemonicTitle",
        "onboarding.welcome.featureNoMnemonicBody",
        "onboarding.welcome.featureOneAddressTitle",
        "onboarding.welcome.featureOneAddressBody",
        "onboarding.welcome.featureOpenSourceTitle",
        "onboarding.welcome.featureOpenSourceBody",
        "onboarding.welcome.featureKeyCustodyTitle",
        "onboarding.welcome.featureKeyCustodyBody",
        "onboarding.welcome.featureSafeContractTitle",
        "onboarding.welcome.featureSafeContractBody",
        "onboarding.welcome.featureStablecoinGasTitle",
        "onboarding.welcome.featureStablecoinGasBody",
        "onboarding.welcome.tagline",
    ];

    /// Every corpus key this client renders outside the wallet fixtures — the
    /// create flow, the failure sheet, the welcome page and its modals, and the
    /// sign-out confirmation.
    ///
    /// Rewritten for the v2 design: spec 014's list named the keys ITS
    /// container used, and eleven of those are now unreachable from any screen
    /// while forty-six are new. A stale entry would keep asserting that copy
    /// nobody renders is translated; a missing one would let a real hole ship.
    /// So the list is the client's surface, maintained with it.
    ///
    /// Var-bearing keys (`{{seconds}}` …) resolve with the placeholder left in
    /// place under default options — still a non-echo, non-empty value, which
    /// is all this sweep asserts about them.
    const FLOW_KEYS: [&str; 118] = [
        "common.cancel",
        "onboarding.create.keyUnreadableTitle",
        "onboarding.create.keyUnreadableBody",
        "onboarding.create.touchSelectBody",
        "onboarding.create.touchTitle",
        "onboarding.create.touchBody",
        "onboarding.create.touchFingerprintBody",
        "onboarding.login.pickTitle",
        "onboarding.login.pickBody",
        "onboarding.login.pickUnnamed",
        "onboarding.create.pinTitle",
        "onboarding.create.pinBody",
        "onboarding.create.pinLabel",
        "onboarding.create.pinAttemptsLeft",
        "onboarding.create.pinRejected",
        "settings.signOut.button",
        "settings.signOut.title",
        "settings.signOut.keeps",
        "settings.signOut.warning",
        "settings.signOut.anyway",
        "settings.signOut.cancel",
        "onboarding.common.back",
        "onboarding.common.close",
        "onboarding.common.confirmInPrompt",
        "onboarding.common.copied",
        "onboarding.common.editIndexEndpoint",
        "onboarding.common.incompatibleBody",
        "onboarding.common.incompatibleTitle",
        "onboarding.common.networkBody",
        "onboarding.common.networkTitle",
        "onboarding.common.notDiscoverableBody",
        "onboarding.common.notDiscoverableTitle",
        "onboarding.common.reportError",
        "onboarding.common.retry",
        "onboarding.common.serverBody",
        "onboarding.common.serverTitle",
        "onboarding.common.timeoutBody",
        "onboarding.common.timeoutTitle",
        "onboarding.common.unknownBody",
        "onboarding.common.unknownTitle",
        "onboarding.common.unsupportedBody",
        "onboarding.common.unsupportedTitle",
        "onboarding.create.accountNamePlaceholder",
        "onboarding.create.ack0",
        "onboarding.create.ack1",
        "onboarding.create.ack2",
        "onboarding.create.ack2And",
        "onboarding.create.ack2Period",
        "onboarding.create.ack2PrivacyPolicy",
        "onboarding.create.ack2Terms",
        "onboarding.create.addKeyBtn",
        "onboarding.create.addMethodLabel",
        "onboarding.create.addSecondKeyBtn",
        "onboarding.create.confirmKeyBtn",
        "onboarding.create.createWalletBtn",
        "onboarding.create.enterWalletBtn",
        "onboarding.create.finishVerifyBtn",
        "onboarding.create.headerDefault",
        "onboarding.create.keyCount",
        "onboarding.create.keyDeviceOnlyBadge",
        "onboarding.create.keyLimitReached",
        "onboarding.create.keySyncedBadge",
        "onboarding.create.keysHint",
        "onboarding.create.keysLabel",
        "onboarding.create.keysSubtitle",
        "onboarding.create.keysSubtitleBlocked",
        "onboarding.create.keysSubtitleFull",
        "onboarding.create.keysTitle",
        "onboarding.create.keysTitleBlocked",
        "onboarding.create.methodBlockedHint",
        "onboarding.create.methodBlockedSigner",
        "onboarding.create.methodHybridTitle",
        "onboarding.create.methodHybridUnavailable",
        "onboarding.create.methodPlatformTitle",
        "onboarding.create.methodSecurityKeyBody",
        "onboarding.create.methodSecurityKeyTitle",
        "onboarding.create.nameTitle",
        "onboarding.create.nameTooLong",
        "onboarding.create.needSecondKeyHint",
        "onboarding.create.nextBtn",
        "onboarding.create.progressSubtitle",
        "onboarding.create.progressTitle",
        "onboarding.create.providerGeneric",
        "onboarding.create.providerPlatform",
        "onboarding.create.providerSecurityKey",
        "onboarding.create.retryUploadBtn",
        "onboarding.create.securityKeyRequiredBody",
        "onboarding.create.securityKeyRequiredTitle",
        "onboarding.create.startOverBtn",
        "onboarding.create.statusComputingAddress",
        "onboarding.create.statusExtractingKey",
        "onboarding.create.statusSettingUpIdentity",
        "onboarding.create.statusSetupCancelled",
        "onboarding.create.statusSyncingKey",
        "onboarding.create.statusVerifyCancelled",
        "onboarding.create.statusVerifyingIdentity",
        "onboarding.create.successTitle",
        "onboarding.create.syncFailedHint",
        "onboarding.create.syncFailedMessage",
        "onboarding.create.syncFailedTitle",
        "onboarding.create.taskDeriveAddress",
        "onboarding.create.taskVerifyKey",
        "onboarding.create.taskWriteIndex",
        "onboarding.create.technicalDetails",
        "onboarding.create.verifyHint",
        "onboarding.login.alertSignInFailedTitle",
        "onboarding.login.recoverCancel",
        "onboarding.login.recoverConfirm",
        "onboarding.login.recoverFailedBody",
        "onboarding.login.recoverFailedTitle",
        "onboarding.login.recoverOfferBody",
        "onboarding.login.recoverOfferTitle",
        "onboarding.login.signInFailedBody",
        "onboarding.settings.endpointUrlLabel",
        "onboarding.settings.passkeyHint",
        "onboarding.settings.resetToDefault",
        "onboarding.settings.sectionPasskeyIndex",
        "onboarding.settings.warningText",
    ];

    /// Where a translation is CORRECTLY identical to the English.
    ///
    /// EMPTY today. It held one entry, a loanword: `providerGeneric` was the
    /// bare word "Passkey", which German-language passkey UI — Apple's,
    /// Google's, and the browsers' — also says. Issue #207 made that key answer
    /// "where does this key live" instead ("Phone or tablet"), which every
    /// locale translates. Kept as a list rather than deleted, so a future entry
    /// has to be argued for in a diff.
    const SAME_AS_ENGLISH: [(&str, &str); 0] = [];

    /// Keys whose value is a term of art that most locales keep verbatim.
    ///
    /// "PIN" is an international acronym — German, Italian, Indonesian,
    /// Turkish, Japanese, Korean, Spanish and Portuguese all print those three
    /// letters, while Chinese, French, Russian and Vietnamese put a word for
    /// "code" around them. Listing nine locale/key pairs would be a list of
    /// coincidences; the fact is about the KEY, so it is stated once about the
    /// key. Every other string in that dialog is still checked per locale.
    const SAME_AS_ENGLISH_KEYS: [&str; 1] = ["onboarding.create.pinLabel"];

    /// Does this value contain anything a translator could translate?
    ///
    /// `{{var}}` names are wire identifiers, not words: `{{current}} / {{max}}`
    /// is the same string in all fifteen locales, by design.
    fn has_words(value: &str) -> bool {
        let mut rest = value;
        let mut stripped = String::new();
        while let Some(open) = rest.find("{{") {
            stripped.push_str(&rest[..open]);
            match rest[open..].find("}}") {
                Some(close) => rest = &rest[open + close + 2..],
                None => {
                    rest = "";
                    break;
                }
            }
        }
        stripped.push_str(rest);
        stripped.chars().any(char::is_alphabetic)
    }

    fn engine_for(lng: &str) -> I18n {
        let mut engine = I18n::embedded().expect("en catalog compiled in");
        let state = engine.change_language(lng);
        if let Some(resolved) = state.resolved_language.as_deref()
            && resolved != "en"
        {
            engine.load_catalog(Catalog::embedded(resolved).expect("catalog compiled in"));
        }
        engine
    }

    /// Spec 038: the desktop intro's nine keys resolve without echo in the
    /// languages the visual pass uses.
    #[test]
    fn intro_keys_resolve_without_echo() {
        const INTRO_KEYS: [&str; 9] = [
            "onboarding.intro.skip",
            "onboarding.intro.next",
            "onboarding.intro.pageOf",
            "onboarding.intro.noSeedTitle",
            "onboarding.intro.noSeedBody",
            "onboarding.intro.custodyTitle",
            "onboarding.intro.custodyBody",
            "onboarding.intro.chainsTitle",
            "onboarding.intro.chainsBody",
        ];
        for lng in ["en", "zh", "de", "zh-TW", "ru"] {
            let engine = engine_for(lng);
            for key in INTRO_KEYS {
                let value = engine.t(key, &Options::default()).expect("t() is total");
                assert_ne!(value, key, "{lng}: {key} echoed");
                assert!(has_words(&value), "{lng}: {key} has no words");
            }
        }
    }

    /// SC-004 as a test: no key echoes in the languages the visual pass uses,
    /// and zh/de actually differ from en (proves the catalog loaded).
    #[test]
    fn welcome_keys_resolve_without_echo() {
        let en = engine_for("en");
        for lng in ["en", "zh", "de", "zh-TW", "ru"] {
            let engine = engine_for(lng);
            for key in WELCOME_KEYS {
                let value = engine.t(key, &Options::default()).expect("t() is total");
                assert_ne!(value, key, "{lng}: `{key}` echoed the key");
                assert!(!value.is_empty(), "{lng}: `{key}` resolved empty");
                if lng != "en" && !key.ends_with("SafeContractTitle") {
                    let en_value = en.t(key, &Options::default()).expect("t() is total");
                    assert_ne!(value, en_value, "{lng}: `{key}` fell back to English");
                }
            }
        }
    }

    /// Spec 014's sweep: every flow key resolves in the visual-pass languages
    /// (no echo, no empty), and non-en locales differ from en — verified at
    /// authoring time that none of these 49 keys legitimately matches English
    /// in zh/de/zh-TW/ru, so no per-key exclusions are needed.
    #[test]
    fn flow_keys_resolve_without_echo() {
        let en = engine_for("en");
        for lng in ["en", "zh", "de", "zh-TW", "ru"] {
            let engine = engine_for(lng);
            for key in FLOW_KEYS {
                let value = engine.t(key, &Options::default()).expect("t() is total");
                assert_ne!(value, key, "{lng}: `{key}` echoed the key");
                assert!(!value.is_empty(), "{lng}: `{key}` resolved empty");
                // "Same as English" is the signal for an untranslated key —
                // EXCEPT where there is nothing to translate. Strip the
                // `{{var}}` placeholders (which are part of the WIRE, identical
                // in every locale) and what is left of `{{current}} / {{max}}`
                // or `.` has no words in it at all. Asserting a difference
                // there would demand a translator change a string that says
                // nothing.
                if lng != "en"
                    && has_words(&value)
                    && !SAME_AS_ENGLISH.contains(&(lng, key))
                    && !SAME_AS_ENGLISH_KEYS.contains(&key)
                {
                    let en_value = en.t(key, &Options::default()).expect("t() is total");
                    assert_ne!(value, en_value, "{lng}: `{key}` fell back to English");
                }
            }
        }
    }

    /// Issue #409: the done screen's line takes the plural route, so a one-key
    /// wallet is not told about "your 1 keys", and every founding-set size
    /// resolves in the visual-pass languages. Plural keys are not in
    /// `FLOW_KEYS`: under default options they have no form to resolve to.
    #[test]
    fn the_done_line_agrees_with_its_count() {
        const KEY: &str = "onboarding.create.successMessage";
        let en = Loc::for_tag("en");
        assert_eq!(
            en.t_count(KEY, 1).as_ref(),
            "Your key can sign in on its own. The contract deploys with your first transaction."
        );
        assert!(
            en.t_count(KEY, 3)
                .starts_with("Any of your 3 keys can sign in")
        );
        for lng in ["en", "zh", "de", "zh-TW", "ru"] {
            let loc = Loc::for_tag(lng);
            for keys in 1..=vela_core::safe::MAX_MULTI_KEYS {
                let line = loc.t_count(KEY, keys);
                assert_ne!(line.as_ref(), KEY, "{lng}/{keys}: the key echoed");
                assert!(!line.contains("{{"), "{lng}/{keys}: unfilled: {line}");
            }
        }
        // What the screen used to call: a variable fills `{{count}}` but never
        // picks a form, so the plural key has nothing to give it.
        assert_eq!(en.t_vars(KEY, &[("count", 1.0)]).as_ref(), KEY);
    }

    /// The mock's zh flow copy is the source of record — pin representative
    /// keys verbatim (contracts/i18n-keys.md zh column).
    #[test]
    fn zh_flow_copy_matches_the_mock_verbatim() {
        let zh = engine_for("zh");
        let opts = Options::default();
        assert_eq!(
            zh.t("onboarding.common.networkTitle", &opts).unwrap(),
            "网络连接不稳定"
        );
        assert_eq!(
            zh.t("onboarding.login.statusAwaitingPasskey", &opts)
                .unwrap(),
            "正在等待通行密钥"
        );
        assert_eq!(
            zh.t("onboarding.common.headerShared", &opts).unwrap(),
            "创建钱包 / 登录"
        );
    }

    /// The mock's zh copy is the source of record — pin two representative keys.
    #[test]
    fn zh_matches_the_mock_verbatim() {
        let zh = engine_for("zh");
        let opts = Options::default();
        assert_eq!(
            zh.t("onboarding.welcome.desktopTagline", &opts).unwrap(),
            "您的密钥，您的资产"
        );
        assert_eq!(
            zh.t("onboarding.welcome.featureNoMnemonicTitle", &opts)
                .unwrap(),
            "不用助记词"
        );
    }

    /// Spec 072: a language chosen in Settings outranks the machine, and the
    /// developer's `VELA_LANG` pin outranks both — the same order `VELA_THEME`
    /// takes over the stored theme.
    #[test]
    fn a_chosen_language_outranks_the_machine_and_the_pin_outranks_both() {
        let system = || "de".to_owned();
        assert_eq!(pick_tag(None, None, system), "de");
        assert_eq!(pick_tag(None, Some("zh-TW".to_owned()), system), "zh-TW");
        assert_eq!(
            pick_tag(Some("ja".to_owned()), Some("zh-TW".to_owned()), system),
            "ja"
        );
    }

    #[test]
    fn posix_tags_normalize() {
        assert_eq!(normalize_posix_tag("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(normalize_posix_tag("de"), "de");
        assert_eq!(normalize_posix_tag("pt_BR"), "pt-BR");
    }
}
