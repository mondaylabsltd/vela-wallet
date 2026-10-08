package app.getvela.wallet.feature.onboarding

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.tooling.preview.Preview
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Preview-only translation fake (never shipped): the tooling process cannot load
 * the native engine. Sample copy mirrors locales/en — the corpus stays the
 * single source of truth for the real app.
 */
private object PreviewStrings : VelaStrings {
    private val sample = mapOf(
        I18nKeys.Welcome.TAGLINE to "Your keys, your assets",
        I18nKeys.Welcome.CREATE_WALLET to "Create Wallet",
        I18nKeys.Welcome.ALREADY_HAVE_WALLET to "I already have a wallet",
        I18nKeys.Welcome.FEATURE_NO_MNEMONIC_TITLE to "No seed phrase",
        I18nKeys.Welcome.FEATURE_NO_MNEMONIC_BODY to
            "Create and sign in with a passkey on your device — no 12 words to copy down and keep safe.",
    )

    override fun t(key: String): String = sample[key] ?: key.substringAfterLast('.')

    override fun t(key: String, vars: Map<String, String>): String = t(key)

    /** Previews carry one sample per key, so a plural key is its own sample. */
    override fun t(key: String, count: Int): String = t(key, mapOf("count" to count.toString()))

    /** Previews have no engine and no clock: one sample. */
    override fun relativeTime(tsSeconds: Long, nowMs: Long, utcOffsetMinutes: Int, dateFormat: String): String = "2m"
}

@Composable
private fun WelcomePreviewContent(darkTheme: Boolean) {
    VelaTheme(darkTheme = darkTheme) {
        CompositionLocalProvider(LocalVelaStrings provides PreviewStrings) {
            WelcomeScreen(
                darkTheme = darkTheme,
                onIntent = {},
                onLongPressLogo = {},
            )
        }
    }
}

@Preview(name = "Welcome — dark (W1)")
@Composable
private fun WelcomePreviewDark() {
    WelcomePreviewContent(darkTheme = true)
}

@Preview(name = "Welcome — light (W1L)")
@Composable
private fun WelcomePreviewLight() {
    WelcomePreviewContent(darkTheme = false)
}
