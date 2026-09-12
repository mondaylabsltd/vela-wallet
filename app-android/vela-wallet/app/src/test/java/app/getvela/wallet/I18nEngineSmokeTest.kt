package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocaleResolver
import java.io.File
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * SC-002b: differential evidence on the real engine (host dylib via JNA — the
 * jna.library.path is wired in app/build.gradle.kts; spec 005's native-rollout
 * precondition). Catalogs come from the generated assets/i18n, the same files
 * the app packages as assets.
 */
class I18nEngineSmokeTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private fun newRuntime(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }

    private val welcomeKeys = listOf(
        I18nKeys.Welcome.TAGLINE,
        I18nKeys.Welcome.HERO_TITLE,
        I18nKeys.Welcome.HERO_SUBTITLE,
        I18nKeys.Welcome.CREATE_WALLET,
        I18nKeys.Welcome.ALREADY_HAVE_WALLET,
        I18nKeys.Welcome.FEATURE_NO_MNEMONIC_TITLE,
        I18nKeys.Welcome.FEATURE_NO_MNEMONIC_BODY,
        I18nKeys.Welcome.FEATURE_ONE_ADDRESS_TITLE,
        I18nKeys.Welcome.FEATURE_ONE_ADDRESS_BODY,
        I18nKeys.Welcome.FEATURE_OPEN_SOURCE_TITLE,
        I18nKeys.Welcome.FEATURE_OPEN_SOURCE_BODY,
        I18nKeys.Welcome.FEATURE_KEY_CUSTODY_TITLE,
        I18nKeys.Welcome.FEATURE_KEY_CUSTODY_BODY,
        I18nKeys.Welcome.FEATURE_SAFE_CONTRACT_TITLE,
        I18nKeys.Welcome.FEATURE_SAFE_CONTRACT_BODY,
        I18nKeys.Welcome.FEATURE_STABLECOIN_GAS_TITLE,
        I18nKeys.Welcome.FEATURE_STABLECOIN_GAS_BODY,
        I18nKeys.Settings.TITLE,
        I18nKeys.Settings.SECTION_APPEARANCE,
        I18nKeys.Settings.THEME_LIGHT,
        I18nKeys.Settings.THEME_DARK,
        I18nKeys.Settings.THEME_AUTO,
        I18nKeys.Create.HEADER,
        I18nKeys.Common.CANCEL,
        // Spec 014 onboarding flow keys (contracts/i18n-keys.md): every string
        // the create/login state panels render must resolve, not echo.
        I18nKeys.Create.HEADER_SYNC_FAILED,
        I18nKeys.Create.ACCOUNT_NAME_PLACEHOLDER,
        I18nKeys.Create.NAME_TOO_LONG,
        I18nKeys.Create.TECHNICAL_DETAILS,
        I18nKeys.Create.ACK0,
        I18nKeys.Create.ACK1,
        I18nKeys.Create.ACK2,
        I18nKeys.Create.ACK2_PRIVACY_POLICY,
        I18nKeys.Create.ACK2_AND,
        I18nKeys.Create.ACK2_TERMS,
        I18nKeys.Create.ACK2_PERIOD,
        // The v2 create journey (spec 019). Listed here rather than in a second
        // test because the claim is the same one: no key this app renders may
        // echo its own name at a person.
        I18nKeys.Create.NAME_TITLE,
        I18nKeys.Create.NEXT_BTN,
        I18nKeys.Create.STATUS_SETUP_CANCELLED,
        I18nKeys.Create.STATUS_VERIFY_CANCELLED,
        I18nKeys.Create.KEYS_TITLE,
        I18nKeys.Create.KEYS_TITLE_BLOCKED,
        I18nKeys.Create.KEYS_SUBTITLE,
        I18nKeys.Create.KEYS_SUBTITLE_BLOCKED,
        I18nKeys.Create.KEYS_SUBTITLE_FULL,
        I18nKeys.Create.KEYS_LABEL,
        I18nKeys.Create.KEYS_HINT,
        I18nKeys.Create.KEY_COUNT,
        I18nKeys.Create.KEY_SYNCED_BADGE,
        I18nKeys.Create.KEY_DEVICE_ONLY_BADGE,
        I18nKeys.Create.KEY_LIMIT_REACHED,
        I18nKeys.Create.NEED_SECOND_KEY_HINT,
        I18nKeys.Create.ADD_KEY_BTN,
        I18nKeys.Create.ADD_SECOND_KEY_BTN,
        I18nKeys.Create.CONFIRM_KEY_BTN,
        I18nKeys.Create.REMOVE_KEY_BTN,
        I18nKeys.Create.FINISH_KEYS_BTN,
        I18nKeys.Create.ADD_METHOD_LABEL,
        I18nKeys.Create.METHOD_PLATFORM_TITLE,
        I18nKeys.Create.METHOD_PLATFORM_BODY,
        I18nKeys.Create.METHOD_HYBRID_TITLE,
        I18nKeys.Create.METHOD_HYBRID_BODY,
        I18nKeys.Create.METHOD_HYBRID_UNAVAILABLE,
        I18nKeys.Create.METHOD_SECURITY_KEY_TITLE,
        I18nKeys.Create.METHOD_SECURITY_KEY_BODY,
        I18nKeys.Create.PROVIDER_PLATFORM,
        I18nKeys.Create.PROVIDER_GENERIC,
        I18nKeys.Create.PROVIDER_SECURITY_KEY,
        I18nKeys.Create.PROGRESS_TITLE,
        I18nKeys.Create.PROGRESS_SUBTITLE,
        I18nKeys.Create.PROGRESS_METER_LABEL,
        I18nKeys.Create.TASK_VERIFY_KEY,
        I18nKeys.Create.TASK_DERIVE_ADDRESS,
        I18nKeys.Create.TASK_WRITE_INDEX,
        I18nKeys.Create.SYNC_FAILED_MESSAGE,
        I18nKeys.Create.SYNC_FAILED_HINT,
        I18nKeys.Create.HEADER_CREATED,
        I18nKeys.Create.IDENTICON_HINT,
        I18nKeys.Create.WALLET_ADDRESS_LABEL,
        I18nKeys.Create.ALERT_ERROR_TITLE,
        I18nKeys.Create.ALERT_NOT_SUPPORTED_TITLE,
        I18nKeys.Create.ALERT_NOT_SUPPORTED_BODY,
        I18nKeys.Login.ALERT_NOT_SUPPORTED_TITLE,
        I18nKeys.Login.ALERT_NOT_SUPPORTED_BODY,
        I18nKeys.Login.ALERT_INCOMPATIBLE_TITLE,
        I18nKeys.Login.ALERT_INCOMPATIBLE_BODY,
        I18nKeys.Login.ALERT_INCOMPATIBLE_BODY_CREATE,
        I18nKeys.Login.ALERT_SIGN_IN_FAILED_BODY,
        I18nKeys.Login.SWITCH_DEVICE_BTN,
        I18nKeys.Settings.SECTION_PASSKEY_INDEX,
        I18nKeys.Settings.ENDPOINT_URL_LABEL,
        I18nKeys.Settings.PASSKEY_HINT,
        I18nKeys.Settings.RESET_TO_DEFAULT,
        I18nKeys.Settings.WARNING_TEXT,
        I18nKeys.Create.CREATE_WALLET_BTN,
        I18nKeys.Create.STATUS_SETTING_UP_IDENTITY,
        I18nKeys.Create.STATUS_VERIFYING_IDENTITY,
        I18nKeys.Create.STATUS_EXTRACTING_KEY,
        I18nKeys.Create.STATUS_COMPUTING_ADDRESS,
        I18nKeys.Create.STATUS_SYNCING_KEY,
        I18nKeys.Create.SUCCESS_TITLE,
        I18nKeys.Create.SUCCESS_MESSAGE,
        I18nKeys.Create.VERIFY_HINT,
        I18nKeys.Create.ENTER_WALLET_BTN,
        I18nKeys.Create.FINISH_VERIFY_BTN,
        I18nKeys.Create.START_OVER_BTN,
        I18nKeys.Create.SYNC_FAILED_TITLE,
        I18nKeys.Create.RETRY_UPLOAD_BTN,
        I18nKeys.Create.RETRY_VERIFY_BTN,
        I18nKeys.Login.HEADER,
        I18nKeys.Login.STATUS_AWAITING_PASSKEY,
        I18nKeys.Login.STATUS_AWAITING_PASSKEY_HINT,
        I18nKeys.Login.STATUS_CANCELLED_TITLE,
        I18nKeys.Login.STATUS_CANCELLED_BODY,
        I18nKeys.Login.SUCCESS_TITLE,
        I18nKeys.Login.SUCCESS_MESSAGE,
        I18nKeys.Login.SIGN_IN_FAILED_TITLE,
        I18nKeys.Login.SIGN_IN_FAILED_BODY,
        I18nKeys.Login.RETRY_LOGIN_BTN,
        I18nKeys.Login.CREATE_NEW_WALLET_BTN,
        I18nKeys.Login.RECOVER_OFFER_TITLE,
        I18nKeys.Login.RECOVER_OFFER_BODY,
        I18nKeys.Login.RECOVER_CONFIRM,
        I18nKeys.Login.RECOVER_CANCEL,
        I18nKeys.Login.RECOVER_FAILED_TITLE,
        I18nKeys.Login.RECOVER_FAILED_BODY,
        I18nKeys.Flow.HEADER_SHARED,
        I18nKeys.Flow.STEP_COUNTER,
        I18nKeys.Flow.CONFIRM_IN_PROMPT,
        I18nKeys.Flow.WAITED_SECONDS,
        I18nKeys.Flow.NETWORK_TITLE,
        I18nKeys.Flow.NETWORK_BODY,
        I18nKeys.Flow.SERVER_TITLE,
        I18nKeys.Flow.SERVER_BODY,
        I18nKeys.Flow.TIMEOUT_TITLE,
        I18nKeys.Flow.TIMEOUT_BODY,
        I18nKeys.Flow.UNKNOWN_TITLE,
        I18nKeys.Flow.UNKNOWN_BODY,
        I18nKeys.Flow.CANCELLED_SETUP_TITLE,
        I18nKeys.Flow.CANCELLED_SETUP_BODY,
        I18nKeys.Flow.CANCELLED_VERIFY_TITLE,
        I18nKeys.Flow.CANCELLED_VERIFY_BODY,
        I18nKeys.Flow.UNSUPPORTED_TITLE,
        I18nKeys.Flow.UNSUPPORTED_BODY,
        I18nKeys.Flow.INCOMPATIBLE_TITLE,
        I18nKeys.Flow.INCOMPATIBLE_BODY,
        I18nKeys.Flow.NOT_DISCOVERABLE_TITLE,
        I18nKeys.Flow.NOT_DISCOVERABLE_BODY,
        I18nKeys.Flow.NOT_FOUND_TITLE,
        I18nKeys.Flow.NOT_FOUND_BODY,
        I18nKeys.Flow.VERIFY_STUCK_TITLE,
        I18nKeys.Flow.VERIFY_STUCK_BODY,
        I18nKeys.Flow.SYNC_FAILED_BODY,
        I18nKeys.Flow.BACK,
        I18nKeys.Flow.RETRY,
        I18nKeys.Flow.RECREATE_WALLET,
        I18nKeys.Flow.EDIT_INDEX_ENDPOINT,
        I18nKeys.Flow.REPORT_ERROR,
        I18nKeys.Flow.OPEN_BIOMETRIC_SETTINGS,
        I18nKeys.Flow.OPEN_CREDENTIAL_MANAGER_SETTINGS,
        I18nKeys.Flow.COPY_ADDRESS,
        I18nKeys.Flow.COPIED,
        I18nKeys.Flow.CLOSE,
    )

    @Test
    fun everyScreenKeyTranslatesInEnglish() {
        val runtime = newRuntime()
        runtime.initialize("en")
        for (key in welcomeKeys) {
            val value = runtime.t(key)
            assertNotEquals("key echoed (missing in en catalog): $key", key, value)
            assertTrue("blank translation for $key", value.isNotBlank())
        }
    }

    @Test
    fun chineseCatalogTranslatesTheWelcomeScreen() {
        val runtime = newRuntime()
        runtime.initialize("zh")
        assertEquals("zh", runtime.state.value.language)
        assertEquals("创建钱包", runtime.t(I18nKeys.Welcome.CREATE_WALLET))
        assertEquals("我已有钱包", runtime.t(I18nKeys.Welcome.ALREADY_HAVE_WALLET))
        assertEquals("您的密钥，您的资产", runtime.t(I18nKeys.Welcome.TAGLINE))
        assertEquals("不用助记词", runtime.t(I18nKeys.Welcome.FEATURE_NO_MNEMONIC_TITLE))
    }

    @Test
    fun everySupportedLocaleLoadsAndTranslates() {
        val runtime = newRuntime()
        runtime.initialize("en")
        for (tag in LocaleResolver.SUPPORTED) {
            runtime.setLocale(tag)
            assertEquals(tag, runtime.state.value.language)
            val value = runtime.t(I18nKeys.Welcome.CREATE_WALLET)
            assertNotEquals("key echoed for locale $tag", I18nKeys.Welcome.CREATE_WALLET, value)
            assertTrue("blank translation for locale $tag", value.isNotBlank())
        }
    }

    @Test
    fun traditionalChineseResolvesToTaiwanCatalog() {
        val tag = LocaleResolver.resolve(listOf(Locale.forLanguageTag("zh-Hant-TW")))
        assertEquals("zh-TW", tag)
        val runtime = newRuntime()
        runtime.initialize(tag)
        assertEquals("zh-TW", runtime.state.value.language)
        assertNotEquals(
            I18nKeys.Welcome.CREATE_WALLET,
            runtime.t(I18nKeys.Welcome.CREATE_WALLET),
        )
    }

    @Test
    fun unsupportedLocaleFallsBackToEnglishCatalog() {
        val tag = LocaleResolver.resolve(listOf(Locale.forLanguageTag("pl-PL")))
        assertEquals("en", tag)
        val runtime = newRuntime()
        runtime.initialize(tag)
        assertEquals("Create Wallet", runtime.t(I18nKeys.Welcome.CREATE_WALLET))
    }

    @Test
    fun switchingBackAndForthKeepsEnglishFallbackResident() {
        val runtime = newRuntime()
        runtime.initialize("zh")
        runtime.setLocale("ja")
        runtime.setLocale("en")
        assertEquals("Create Wallet", runtime.t(I18nKeys.Welcome.CREATE_WALLET))
    }
}
