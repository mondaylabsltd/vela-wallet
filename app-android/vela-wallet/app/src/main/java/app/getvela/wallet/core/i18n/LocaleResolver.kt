package app.getvela.wallet.core.i18n

import java.util.Locale
import uniffi.vela_core_uniffi.i18nSystemLanguage

/**
 * Maps the system locale list onto the 15 supported catalog tags (research D4).
 *
 * The rule is the core's `system_language` (spec 095): the first locale in the
 * person's order that a shipped catalog serves — Chinese by script, then
 * region; any `es` → `es-MX`, any `pt` → `pt-BR`; language-region, then
 * language — else `en`. The same function the desktop and iOS call; this file
 * only turns Android's `Locale` list into tags.
 *
 * NOTE: always derive codes from `toLanguageTag()` — `Locale.language` returns
 * legacy ISO codes (`in` for Indonesian) that would silently miss `id`.
 */
object LocaleResolver {

    val SUPPORTED: List<String> = listOf(
        "en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id",
        "tr", "es-MX", "pt-BR", "fr", "de", "ru", "it",
    )

    const val FALLBACK: String = "en"

    fun resolve(locales: List<Locale>): String =
        i18nSystemLanguage(locales.map { it.toLanguageTag() })
}
