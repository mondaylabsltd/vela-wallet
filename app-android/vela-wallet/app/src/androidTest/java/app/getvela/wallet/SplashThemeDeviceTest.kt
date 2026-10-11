package app.getvela.wallet

import android.app.UiModeManager
import android.content.ComponentCallbacks
import android.content.res.Configuration
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.SdkSuppress
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.AppNightMode
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.data.ThemePreferenceRepository
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 12: the system splash follows the app's theme.
 *
 * The system paints the splash before the app runs, by the night mode it
 * holds FOR THIS APP — so what has to be true is that the system holds the
 * person's choice. Through the real [ThemePreferenceRepository] on a real
 * system (Android 12+): choosing 深色 makes the app's own configuration night
 * whatever the system's is, 浅色 makes it day, 跟随系统 gives it back; and a
 * choice already in the store — made on an older build — is told at start.
 *
 * (The splash itself is measured from a screen recording, frame by frame:
 * `scratchpad/pr4/shell/android/splash.sh`.)
 *
 * Emulator only: it sets this app's night mode, and puts it back.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SplashThemeDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
@SdkSuppress(minSdkVersion = 31)
class SplashThemeDeviceTest {

    private val app = InstrumentationRegistry.getInstrumentation().targetContext.applicationContext
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** The app's own night flag, as each configuration change states it. */
    private val night = MutableStateFlow(nightOf(app.resources.configuration))
    private val watcher = object : ComponentCallbacks {
        override fun onConfigurationChanged(next: Configuration) { night.value = nightOf(next) }
        @Deprecated("the platform's interface")
        override fun onLowMemory() = Unit
    }

    private fun nightOf(configuration: Configuration) = configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK

    private class MemoryStore(initial: Map<String, String> = emptyMap()) : KeyValueStore {
        val values = ConcurrentHashMap(initial)
        override suspend fun read(key: String): String? = values[key]
        override suspend fun write(key: String, value: String): Boolean { values[key] = value; return true }
        override suspend fun remove(vararg keys: String): Boolean { keys.forEach { values.remove(it) }; return true }
        override suspend fun allKeys(): List<String> = values.keys.toList()
    }

    @Before
    fun watch() {
        app.registerComponentCallbacks(watcher)
        // From a known place: the system's own.
        AppNightMode.tell(app, ThemePreference.Auto)
    }

    @After
    fun putBack() {
        AppNightMode.tell(app, ThemePreference.Auto)
        app.unregisterComponentCallbacks(watcher)
        scope.cancel()
    }

    /** The app's configuration is [wanted] — now, or once the system has said so. Woken by the change itself. */
    private fun reaches(wanted: Int, what: String) = runBlocking {
        withTimeoutOrNull(HANG_MS) { night.first { nightOf(app.resources.configuration) == wanted } }
            ?: throw AssertionError("$what: the app's night mode is ${nightOf(app.resources.configuration)}, not $wanted")
    }

    @Test
    fun theSystemHoldsThePersonsChoiceForThisApp() {
        val systemNight = app.getSystemService(UiModeManager::class.java).nightMode
        val system = if (systemNight == UiModeManager.MODE_NIGHT_YES) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO
        val store = MemoryStore()
        val repository = ThemePreferenceRepository(app, store, scope)
        runBlocking { repository.themePreference.first() }
        reaches(system, "跟随系统, the default")

        runBlocking { repository.setThemePreference(ThemePreference.Dark) }
        reaches(Configuration.UI_MODE_NIGHT_YES, "深色")
        assertEquals("dark", store.values[ThemePreferenceRepository.KEY])

        runBlocking { repository.setThemePreference(ThemePreference.Light) }
        reaches(Configuration.UI_MODE_NIGHT_NO, "浅色")

        runBlocking { repository.setThemePreference(ThemePreference.Auto) }
        reaches(system, "跟随系统 again")

        // A choice already stored — an older build's, another launch's — is told when the store is read.
        val opposite = if (system == Configuration.UI_MODE_NIGHT_YES) ThemePreference.Light else ThemePreference.Dark
        val stored = ThemePreferenceRepository(app, MemoryStore(mapOf(ThemePreferenceRepository.KEY to opposite.storageValue)), scope)
        assertEquals(opposite, runBlocking { stored.themePreference.first() })
        reaches(if (opposite == ThemePreference.Dark) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO, "a stored ${opposite.storageValue}, at start")
    }

    private companion object {
        /** A hang limit, not a deadline. */
        const val HANG_MS = 60_000L
    }
}
