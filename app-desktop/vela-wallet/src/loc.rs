//! Localization access — the only module that touches `vela_core::i18n`
//! (spec 007 FR-009).
//!
//! Desktop compiles all 15 catalogs in (`i18n-all`, research.md D6), so language
//! selection is synchronous and infallible: the engine always holds the pinned
//! `en` fallback, plus the resolved locale's catalog when that locale isn't `en`.

use gpui::SharedString;
use vela_core::i18n::{Catalog, I18n, Options, Var};

pub struct Loc {
    engine: I18n,
}

impl Loc {
    /// Build the engine for the language in force: the `VELA_LANG` pin, the
    /// language chosen in Settings (spec 072), else `LC_ALL` → `LC_MESSAGES`
    /// → `LANG` → `en` (spec 007 FR-007) — resolved through the same ladder
    /// i18next uses (`resolve_language`).
    pub fn from_env() -> Self {
        let requested = requested_tag();

        let mut engine = match I18n::embedded() {
            Ok(engine) => engine,
            // `i18n-en` is a compile-time feature of this binary; construction
            // can only fail if the crate was built without it, which the
            // dependency declaration makes impossible. Render keys as-is rather
            // than crash the welcome screen if that invariant is ever broken.
            Err(_) => return Self::key_echo(),
        };

        let state = engine.change_language(&requested);
        if let Some(resolved) = state.resolved_language.as_deref()
            && resolved != "en"
            && let Ok(catalog) = Catalog::embedded(resolved)
        {
            engine.load_catalog(catalog);
        }
        Self { engine }
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

    /// The BCP-47 tag actually resolved (used only for logging).
    pub fn language(&self) -> &str {
        self.engine.language()
    }
}

/// The tag the strings resolve from: the `VELA_LANG` pin, else the language
/// the person chose in Settings (spec 072: `vela.language`), else the
/// machine's own ([`system_tag`]).
pub(crate) fn requested_tag() -> String {
    pick_tag(
        env_tag(&["VELA_LANG"]),
        crate::executor::preferences::pinned_language(),
        system_tag,
    )
}

/// The machine's locale: `VELA_LANG` → `LC_ALL` → `LC_MESSAGES` → `LANG` →
/// `en`, as a BCP-47-shaped tag (`zh_CN.UTF-8` → `zh-CN`). What "follow the
/// system" follows, and what the format presets' "Automatic" means (spec 038
/// #E3) — the web's `auto` formats read the platform too, never the app's
/// chosen language.
pub(crate) fn system_tag() -> String {
    env_tag(&["VELA_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]).unwrap_or_else(|| "en".to_owned())
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
    const FLOW_KEYS: [&str; 120] = [
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
        "onboarding.create.identiconHint",
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
        "onboarding.create.methodHybridTitle",
        "onboarding.create.methodHybridUnavailable",
        "onboarding.create.methodPlatformTitle",
        "onboarding.create.methodSecurityKeyBody",
        "onboarding.create.methodSecurityKeyTitle",
        "onboarding.create.nameTitle",
        "onboarding.create.nameTooLong",
        "onboarding.create.needSecondKeyHint",
        "onboarding.create.nextBtn",
        "onboarding.create.progressMeterLabel",
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
        "onboarding.create.successMessage",
        "onboarding.create.successTitle",
        "onboarding.create.syncFailedHint",
        "onboarding.create.syncFailedMessage",
        "onboarding.create.syncFailedTitle",
        "onboarding.create.taskDeriveAddress",
        "onboarding.create.taskVerifyKey",
        "onboarding.create.taskWriteIndex",
        "onboarding.create.technicalDetails",
        "onboarding.create.verifyHint",
        "onboarding.create.walletAddressLabel",
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
