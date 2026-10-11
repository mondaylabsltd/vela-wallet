package app.getvela.wallet

import android.app.UiModeManager
import app.getvela.wallet.core.data.AppNightMode
import app.getvela.wallet.core.data.ThemePreference
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Device walk 2026-10-11, item 12: with the phone in dark mode and Vela set
 * to 浅色, a dark system splash cut to the light launch animation. The system
 * paints the splash before any app code runs, so the person's choice is told
 * to the system ([AppNightMode]) and kept there for the next launch.
 *
 * What is told, for each choice. (That the system then paints the splash by
 * it is measured frame by frame on an emulator, and `SplashThemeDeviceTest`
 * checks the system holds it.)
 */
class AppNightModeTest {
    @Test
    fun `light and dark are themselves, and system hands the question back`() {
        assertEquals(UiModeManager.MODE_NIGHT_NO, AppNightMode.modeOf(ThemePreference.Light))
        assertEquals(UiModeManager.MODE_NIGHT_YES, AppNightMode.modeOf(ThemePreference.Dark))
        assertEquals(UiModeManager.MODE_NIGHT_AUTO, AppNightMode.modeOf(ThemePreference.Auto))
        // Every choice is told: none is left for the system to guess.
        assertEquals(ThemePreference.entries.size, ThemePreference.entries.map(AppNightMode::modeOf).toSet().size)
    }
}
