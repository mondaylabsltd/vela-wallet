package app.getvela.wallet

import app.getvela.wallet.core.i18n.LocaleResolver
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Test

/** Research D4 ladder, table-tested — the core's `system_language` (spec 095) through the shell. */
class LocaleResolverTest {

    private fun resolve(vararg tags: String): String =
        LocaleResolver.resolve(tags.map { Locale.forLanguageTag(it) })

    @Test
    fun exactAndBareLanguageMatches() {
        assertEquals("en", resolve("en-US"))
        assertEquals("en", resolve("en"))
        assertEquals("ja", resolve("ja-JP"))
        assertEquals("ko", resolve("ko-KR"))
        assertEquals("vi", resolve("vi-VN"))
        assertEquals("tr", resolve("tr-TR"))
        assertEquals("fr", resolve("fr-FR"))
        assertEquals("de", resolve("de-DE"))
        assertEquals("ru", resolve("ru-RU"))
        assertEquals("it", resolve("it-IT"))
    }

    @Test
    fun indonesianSurvivesLegacyJavaLocaleCode() {
        // Locale.language returns the legacy "in"; the resolver must still hit "id".
        assertEquals("id", resolve("id-ID"))
        assertEquals("id", resolve("id"))
    }

    @Test
    fun chineseScriptAndRegionLadder() {
        assertEquals("zh-TW", resolve("zh-Hant-TW"))
        assertEquals("zh-TW", resolve("zh-Hant"))
        assertEquals("zh-HK", resolve("zh-Hant-HK"))
        assertEquals("zh-HK", resolve("zh-Hant-MO"))
        assertEquals("zh-TW", resolve("zh-TW"))
        assertEquals("zh-HK", resolve("zh-HK"))
        assertEquals("zh-HK", resolve("zh-MO"))
        assertEquals("zh", resolve("zh-Hans-CN"))
        assertEquals("zh", resolve("zh-CN"))
        assertEquals("zh", resolve("zh-SG"))
        assertEquals("zh", resolve("zh"))
    }

    @Test
    fun regionalRepresentatives() {
        assertEquals("es-MX", resolve("es-MX"))
        assertEquals("es-MX", resolve("es-ES"))
        assertEquals("es-MX", resolve("es"))
        assertEquals("pt-BR", resolve("pt-BR"))
        assertEquals("pt-BR", resolve("pt-PT"))
        assertEquals("pt-BR", resolve("pt"))
    }

    @Test
    fun unsupportedFallsThroughTheListThenToEnglish() {
        assertEquals("en", resolve("pl-PL"))
        assertEquals("zh-TW", resolve("pl-PL", "zh-TW", "en"))
        assertEquals("de", resolve("nl-NL", "de-DE"))
        assertEquals("en", LocaleResolver.resolve(emptyList()))
    }
}
