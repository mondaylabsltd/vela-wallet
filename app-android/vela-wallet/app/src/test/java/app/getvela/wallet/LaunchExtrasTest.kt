package app.getvela.wallet

import app.getvela.wallet.navigation.VelaDestinations
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Spec 088 FR-003: MainActivity is exported, so any installed app can start it
 * with extras. A release build must ignore every device-pass extra — above all
 * the route extra, which can open a fixture gallery of invented balances.
 */
class LaunchExtrasTest {
    @Test
    fun `a release build starts at Welcome whatever the route extra says`() {
        for (route in VelaDestinations.ALL + listOf("nonsense", "")) {
            assertEquals(route, VelaDestinations.WELCOME, LaunchExtras.startDestination(route, debug = false))
        }
        assertEquals(VelaDestinations.WELCOME, LaunchExtras.startDestination(null, debug = false))
    }

    @Test
    fun `a debug build may start on any known route, and only a known one`() {
        for (route in VelaDestinations.ALL) {
            assertEquals(route, LaunchExtras.startDestination(route, debug = true))
        }
        assertEquals(VelaDestinations.WELCOME, LaunchExtras.startDestination("nonsense", debug = true))
        assertEquals(VelaDestinations.WELCOME, LaunchExtras.startDestination(null, debug = true))
    }

    @Test
    fun `a release build reads none of the device-pass extras`() {
        for (name in LaunchExtras.DEBUG_ONLY) {
            assertNull(name, LaunchExtras.honoured(name, "x", debug = false))
            assertNull(name, LaunchExtras.honoured(name, true, debug = false))
            assertEquals(name, "x", LaunchExtras.honoured(name, "x", debug = true))
        }
        // The galleries, the route, the page door and the parallel space are all on the list.
        for (name in listOf("vela.gallery", "vela.startDestination", "vela.openUrl", "vela.parallelSpace", "vela.settingsState", "vela.flowState", "vela.signingState")) {
            assert(name in LaunchExtras.DEBUG_ONLY) { name }
        }
    }

    @Test
    fun `the app's real doors are not debug extras`() {
        // The notification's own door opens a receipt in release too.
        assertEquals("0xabc", LaunchExtras.honoured("vela.receipt", "0xabc", debug = false))
    }
}
