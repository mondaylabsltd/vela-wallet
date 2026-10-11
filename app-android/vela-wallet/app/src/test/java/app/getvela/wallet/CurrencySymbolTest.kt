package app.getvela.wallet

import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.i18n.AppLanguage
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.CurrencyCatalog
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Device walk 2026-10-11, item 8: every figure on the owner's phone read
 * 「CN¥188.44」 under a Settings row that said 「CNY · ¥」, beside an iPhone
 * that wrote 「¥34.03」. This shell asked the platform for a symbol
 * (`Currency.getSymbol(Locale.US)`) and kept a table of glyphs of its own for
 * Settings, where the yuan and the yen were both "¥" and two dollars both "$".
 *
 * One rule now, the core's (`fiatSymbol(code, language)`), for every fiat
 * figure — they are all written by [WalletLive.Money] — for the send form's
 * unit, and for Settings' currency row and each row of its picker.
 */
class CurrencySymbolTest {

    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val languages = File(root, "assets/i18n").listFiles { file -> file.extension == "json" }!!.map { it.nameWithoutExtension }.sorted()

    private fun strings(tag: String): VelaStrings = I18nRuntime { name -> File(root, "assets/i18n/$name.json").readBytes() }.apply { initialize(tag) }

    private fun money(code: String, language: String) = WalletLive.Money.of(CurrencyView(code = code, rate = 2.0, committed = true), language)

    private var was = Formats.current

    @Before
    fun plainNumbers() {
        was = Formats.current
        Formats.current = Formats(number = app.getvela.wallet.core.format.NumberFormatKey.CommaDot)
    }

    @After
    fun restore() {
        Formats.current = was
        AppLanguage.tag = I18nRuntime.FALLBACK
    }

    @Test
    fun `the yuan is the bare sign in Chinese and carries its country elsewhere`() {
        for (chinese in listOf("zh", "zh-TW", "zh-HK")) {
            assertEquals(chinese, "¥188.44", money("CNY", chinese).fiat(94.22))
            assertEquals(chinese, "JP¥188.44", money("JPY", chinese).fiat(94.22))
        }
        for (other in languages - setOf("zh", "zh-TW", "zh-HK")) {
            assertEquals(other, "CN¥188.44", money("CNY", other).fiat(94.22))
            assertEquals(other, "¥188.44", money("JPY", other).fiat(94.22))
        }
    }

    @Test
    fun `a dollar that is not the US dollar's says whose it is, in every language`() {
        for (language in languages) {
            assertEquals(language, "$10.00", money("USD", language).fiat(5.0))
            assertEquals(language, "HK$10.00", money("HKD", language).fiat(5.0))
            assertEquals(language, "€10.00", money("EUR", language).fiat(5.0))
        }
    }

    /** The hero draws its figure in two parts: the same sign leads the whole part. */
    @Test
    fun `the hero's parts carry the same sign`() {
        assertEquals("¥1,234" to "50", money("CNY", "zh").parts(617.25)!!.let { it.integer to it.decimals })
        assertEquals("CN¥1,234" to "50", money("CNY", "en").parts(617.25)!!.let { it.integer to it.decimals })
    }

    /** CLDR's currency spacing, as the core applies it: a letter beside the digits is held off by a no-break space. */
    @Test
    fun `a sign that ends in a letter stands a no-break space off the digits, a glyph does not`() {
        assertEquals("zł 10.00", money("PLN", "en").fiat(5.0))
        assertEquals("CHF 10.00", money("CHF", "en").fiat(5.0))
        assertEquals("R 10.00", money("ZAR", "en").fiat(5.0))
        // A code nobody stocks is itself — never a wrong sign in front of a number.
        assertEquals("XYZ 10.00", money("XYZ", "en").fiat(5.0))
        assertEquals("R$10.00", money("BRL", "en").fiat(5.0))
        assertEquals("NT$10.00", money("TWD", "en").fiat(5.0))
    }

    /** A builder with no strings in hand draws in the app's language — which the app's own runtime publishes. */
    @Test
    fun `a figure built with no language named is in the app's`() {
        val published = mutableListOf<String>()
        val runtime = I18nRuntime(onLanguage = { published += it; AppLanguage.tag = it }) { name -> File(root, "assets/i18n/$name.json").readBytes() }
        runtime.initialize("zh")
        assertEquals(listOf("zh"), published)
        val view = CurrencyView(code = "CNY", rate = 2.0, committed = true)
        assertEquals("¥188.44", WalletLive.Money.of(view).fiat(94.22))
        runtime.setLocale("en")
        assertEquals(listOf("zh", "en"), published)
        assertEquals("CN¥188.44", WalletLive.Money.of(view).fiat(94.22))
        // A runtime that is not the app's speaks only for itself.
        strings("ja")
        assertEquals("en", AppLanguage.tag)
    }

    @Test
    fun `the send form's unit is the same sign`() {
        assertEquals("¥" to null, SendLive.unitAdornment("CNY", "BNB", "zh"))
        assertEquals("CN¥" to null, SendLive.unitAdornment("CNY", "BNB", "en"))
        assertEquals("JP¥" to null, SendLive.unitAdornment("JPY", "BNB", "zh"))
        assertEquals("HK$" to null, SendLive.unitAdornment("HKD", "BNB", "en"))
    }

    @Test
    fun `Settings' row says the code and the sign its figures carry`() {
        fun row(code: String, language: String): String {
            val model = SettingsLive.withCurrency(
                SettingsFixtures.buildState(SettingsScreenState.ST1, strings(language)),
                CurrencyView(code = code, rate = 2.0, committed = true),
                language,
            )
            return model.sections.flatMap { it.rows }.single { it.id == "currency" }.value.orEmpty()
        }
        assertEquals("CNY · ¥", row("CNY", "zh"))
        assertEquals("CNY · CN¥", row("CNY", "en"))
        assertEquals("JPY · JP¥", row("JPY", "zh"))
        assertEquals("JPY · ¥", row("JPY", "en"))
        assertEquals("HKD · HK$", row("HKD", "zh"))
        assertEquals("USD · $", row("USD", "zh"))
        // A currency whose sign IS its code says it once.
        assertEquals("CHF", row("CHF", "en"))
    }

    /** The picker, live and on its board: each row's symbol is the core's, and no two rows of it are alike — in any language. */
    @Test
    fun `no two rows of the picker share a symbol, in any of the languages`() {
        for (language in languages) {
            val words = strings(language)
            val drawn = SettingsFixtures.buildState(SettingsScreenState.ST1, words)
            val live = SettingsLive.withCurrency(drawn, CurrencyView(code = "CNY", rate = 2.0, committed = true), language)
            for ((what, sheet) in listOf("live" to live.currencySheet, "board" to drawn.currencySheet)) {
                val symbols = sheet.rows.associate { it.id to it.glyph.orEmpty() }
                assertEquals("$language $what: a row per currency on offer", CurrencyCatalog.entries.map { it.code }, sheet.rows.map { it.id })
                assertEquals("$language $what: two rows read alike — $symbols", symbols.size, symbols.values.toSet().size)
                assertFalse("$language $what: a row with no symbol — $symbols", symbols.values.any { it.isBlank() })
                symbols.forEach { (code, symbol) -> assertEquals("$language $what $code", WalletLive.Money.fiatSign(code, language), symbol) }
                assertEquals("$language $what", "HK$", symbols.getValue("HKD"))
                assertEquals("$language $what", "$", symbols.getValue("USD"))
            }
            val chinese = language.startsWith("zh")
            assertEquals(language, if (chinese) "¥" else "CN¥", live.currencySheet.rows.single { it.id == "CNY" }.glyph)
            assertEquals(language, if (chinese) "JP¥" else "¥", live.currencySheet.rows.single { it.id == "JPY" }.glyph)
            assertTrue(live.currencySheet.rows.single { it.id == "CNY" }.selected)
        }
    }

    /** Nothing here asks the platform for a currency's symbol any more. */
    @Test
    fun `no builder asks the platform for a symbol`() {
        val sources = File(root, "app-android/vela-wallet/app/src/main/java").walkTopDown().filter { it.extension == "kt" }
        val offenders = sources.filter { file -> file.readLines().any { "getSymbol(" in it && !it.trimStart().startsWith("*") && !it.trimStart().startsWith("//") } }.map { it.name }.toList()
        assertEquals("a platform currency-symbol lookup is back", emptyList<String>(), offenders)
    }
}
