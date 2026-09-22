package app.getvela.wallet.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map

/** Spec entity: Light | Dark | Auto; Auto follows the system (default). */
enum class ThemePreference(val storageValue: String) {
    Light("light"),
    Dark("dark"),
    Auto("auto");

    companion object {
        fun fromStorage(value: String?): ThemePreference =
            entries.firstOrNull { it.storageValue == value } ?: Auto
    }
}

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

/** Persists the theme choice across restarts (spec FR-006, research D9). */
class ThemePreferenceRepository(private val context: Context) {

    private val key = stringPreferencesKey("theme_preference")

    val themePreference: Flow<ThemePreference> =
        context.settingsDataStore.data
            // An unreadable preferences file must degrade to the Auto default, not
            // become a crash loop (DataStore's data flow throws IOException).
            .catch { error ->
                if (error is IOException) emit(emptyPreferences()) else throw error
            }
            .map { prefs -> ThemePreference.fromStorage(prefs[key]) }

    suspend fun setThemePreference(preference: ThemePreference) {
        context.settingsDataStore.edit { prefs -> prefs[key] = preference.storageValue }
    }

    /**
     * Forget the choice entirely — the erase path's, and nothing else's
     * (spec 081 FR-017).
     *
     * This store sits OUTSIDE the wallet's `vela.*` file on purpose, so that
     * signing out cannot reach a preference that is about the device rather
     * than the account. The same separation is exactly why the erase used to
     * miss it: `DeviceStorage.erase` enumerated one file, and this is the
     * other one. Whole-store `clear()`, not `remove(key)`, because a
     * preference added here next year must go on the day it is first written.
     *
     * @return false when the platform refused; the caller reports the survivor.
     */
    suspend fun clear(): Boolean =
        runCatching { context.settingsDataStore.edit { it.clear() } }.isSuccess
}
