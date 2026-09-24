package app.getvela.wallet

import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class PreferencesTest {
    @Test
    fun `the web's keys, read back and written`() = runBlocking<Unit> {
        val store = FakeStore(mapOf("vela.language" to "fr", "vela.localePrefs" to """{"numberFormat":"dot_comma","dateFormat":"iso","timeFormat":"h12","textScale":"large"}"""))
        var published: Formats? = null
        val prefs = Preferences(store, kotlinx.coroutines.CoroutineScope(Dispatchers.Unconfined), { Locale.US }, { published = it })
        prefs.load()
        val loaded = withTimeout(5_000) { prefs.view.first { it.loaded } }
        assertEquals("fr", loaded.language)
        assertEquals(NumberFormatKey.DotComma, loaded.numberFormat)
        assertEquals(DateFormatKey.Iso, loaded.dateFormat)
        assertEquals(TextScaleLevel.Large, loaded.textScale)
        assertEquals("1.234,50", published!!.fixed2(1234.5))

        prefs.setNumberFormat(NumberFormatKey.Indian)
        prefs.setLanguage("system")
        assertEquals("system", store.values["vela.language"])
        assertEquals(true, store.values["vela.localePrefs"]!!.contains("\"numberFormat\":\"indian\""))
        assertEquals("12,34,567.00", published!!.fixed2(1234567.0))
    }

    @Test
    fun `an empty store is the defaults`() = runBlocking<Unit> {
        val prefs = Preferences(FakeStore(), kotlinx.coroutines.CoroutineScope(Dispatchers.Unconfined), { Locale.US }, {})
        prefs.load()
        val v = withTimeout(5_000) { prefs.view.first { it.loaded } }
        assertEquals("auto", v.language)
        assertEquals(NumberFormatKey.Auto, v.numberFormat)
    }

    /** Spec 074: every avatar is the identicon; the retired choice is removed once, and changes nothing. */
    @Test
    fun `a stored avatar style is removed at load`() = runBlocking<Unit> {
        val store = FakeStore(mapOf("vela.avatarStyle" to "initials", "vela.language" to "fr"))
        val prefs = Preferences(store, kotlinx.coroutines.CoroutineScope(Dispatchers.Unconfined), { Locale.US }, {})
        prefs.load()
        val v = withTimeout(5_000) { prefs.view.first { it.loaded } }
        assertEquals("fr", v.language)
        assertFalse(store.values.containsKey("vela.avatarStyle"))
        assertEquals("fr", store.values["vela.language"])
    }

    /**
     * Spec 072: what this shell used to write — `system`, and the text size
     * inside `vela.localePrefs` — reads as the shared record and is rewritten
     * once, so every other Vela reads the same preferences.
     */
    @Test
    fun `an older android record reads as the shared one and is rewritten`() = runBlocking<Unit> {
        val store = FakeStore(
            mapOf(
                "vela.language" to "system",
                "vela.localePrefs" to """{"numberFormat":"space_comma","dateFormat":"iso","timeFormat":"h24","textScale":"large"}""",
            ),
        )
        val prefs = Preferences(store, kotlinx.coroutines.CoroutineScope(Dispatchers.Unconfined), { Locale.US }, {})
        prefs.load()
        val v = withTimeout(5_000) { prefs.view.first { it.loaded } }
        assertEquals("auto", v.language)
        assertEquals(app.getvela.wallet.core.format.TextScaleLevel.Large, v.textScale)
        assertEquals(NumberFormatKey.SpaceComma, v.numberFormat)
        assertEquals("auto", store.values["vela.language"])
        assertEquals("large", store.values["vela.textScale"])
        assertEquals(false, store.values["vela.localePrefs"]!!.contains("textScale"))
    }
}
