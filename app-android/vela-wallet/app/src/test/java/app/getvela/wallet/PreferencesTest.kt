package app.getvela.wallet

import app.getvela.wallet.core.data.DebugMode
import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.feature.settings.VersionTapCounter
import java.util.Locale
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.prefsDebugModeValue

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

    private suspend fun loaded(store: FakeStore): Preferences {
        val prefs = Preferences(store, kotlinx.coroutines.CoroutineScope(Dispatchers.Unconfined), { Locale.US }, {})
        prefs.load()
        withTimeout(5_000) { prefs.view.first { it.loaded } }
        return prefs
    }

    /** Spec 091: `vela.debugMode` read through the core — absent, or anything it does not write, is hidden. */
    @Test
    fun `debug mode reads hidden, off and on`() = runBlocking<Unit> {
        assertEquals(DebugMode.Hidden, loaded(FakeStore()).view.value.debugMode)
        assertEquals(DebugMode.Off, loaded(FakeStore(mapOf(Preferences.KEY_DEBUG_MODE to prefsDebugModeValue(false)))).view.value.debugMode)
        assertEquals(DebugMode.On, loaded(FakeStore(mapOf(Preferences.KEY_DEBUG_MODE to prefsDebugModeValue(true)))).view.value.debugMode)
        for (stray in listOf("ON", "true", "")) {
            val mode = loaded(FakeStore(mapOf(Preferences.KEY_DEBUG_MODE to stray))).view.value.debugMode
            assertEquals(stray, DebugMode.Hidden, mode)
            assertFalse(mode.revealed || mode.on)
        }
        assertEquals("the core's key", "vela.debugMode", Preferences.KEY_DEBUG_MODE)
    }

    /** Spec 091: revealing stores the switch off; turning it stores the core's spelling; an erase hides it again. */
    @Test
    fun `debug mode is revealed off, switched, and hidden again by an erase`() = runBlocking<Unit> {
        val store = FakeStore()
        val prefs = loaded(store)
        prefs.revealDebugMode()
        assertEquals(DebugMode.Off, prefs.view.value.debugMode)
        assertEquals(prefsDebugModeValue(false), store.values[Preferences.KEY_DEBUG_MODE])
        prefs.setDebugMode(true)
        assertEquals(DebugMode.On, prefs.view.value.debugMode)
        assertEquals(prefsDebugModeValue(true), store.values[Preferences.KEY_DEBUG_MODE])
        prefs.setDebugMode(false)
        assertEquals(DebugMode.Off, prefs.view.value.debugMode)
        assertEquals(prefsDebugModeValue(false), store.values[Preferences.KEY_DEBUG_MODE])
        // What was stored is what the next launch reads.
        prefs.setDebugMode(true)
        assertEquals(DebugMode.On, loaded(store).view.value.debugMode)
        // The erase removes every `vela.` key; the view reads the store again.
        store.values.clear()
        prefs.reloadDebugMode()
        assertEquals(DebugMode.Hidden, prefs.view.value.debugMode)
    }

    /**
     * Spec 091: About's version, wired as the screen wires it — each tap asks
     * the core ([VersionTapCounter]), a revealing tap stores the switch
     * (`SettingsRoute` → `onDebugModeRevealed` → [Preferences.revealDebugMode]).
     * The rule is the core's; this is the wiring.
     */
    @Test
    fun `seven quick taps on the version reveal debug mode once, and a revealed switch ignores taps`() = runBlocking<Unit> {
        val store = FakeStore()
        val prefs = loaded(store)
        var clock = 1_000_000.0
        val counter = VersionTapCounter { clock }
        var reveals = 0
        fun tap(gapMs: Double = 300.0) {
            clock += gapMs
            if (counter.tap(prefs.view.value.debugMode)) {
                reveals += 1
                prefs.revealDebugMode()
            }
        }
        // Six quick taps, a pause, and the count starts again.
        repeat(6) { tap() }
        tap(gapMs = 1_500.0)
        repeat(5) { tap() }
        assertEquals(0, reveals)
        assertEquals(DebugMode.Hidden, prefs.view.value.debugMode)
        assertNull(store.values[Preferences.KEY_DEBUG_MODE])
        // The seventh of the run reveals, once.
        tap()
        assertEquals(1, reveals)
        assertEquals(DebugMode.Off, prefs.view.value.debugMode)
        assertEquals(prefsDebugModeValue(false), store.values[Preferences.KEY_DEBUG_MODE])
        // Revealed — off or on — nothing counts.
        repeat(20) { tap() }
        prefs.setDebugMode(true)
        repeat(20) { tap() }
        assertEquals(1, reveals)
        assertEquals(DebugMode.On, prefs.view.value.debugMode)
    }
}
