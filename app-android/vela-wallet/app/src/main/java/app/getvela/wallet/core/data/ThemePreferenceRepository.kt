package app.getvela.wallet.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import uniffi.vela_core_uniffi.prefsRead

/** Spec entity: Light | Dark | Auto; Auto follows the system (default). */
enum class ThemePreference(val storageValue: String) {
    Light("light"),
    Dark("dark"),
    /** Stored as `system`, the shared word (spec 072); this shell wrote `auto`. */
    Auto("system");

    companion object {
        fun fromStorage(value: String?): ThemePreference =
            entries.firstOrNull { it.storageValue == value } ?: Auto
    }
}

private val Context.settingsDataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

/**
 * Persists the theme choice across restarts (spec FR-006, research D9) — as
 * `vela.theme` in the store every other preference lives in, the record every
 * Vela shares (spec 072). It used to live in a DataStore of its own, where no
 * other shell could read it and "erase this device" did not reach it; a value
 * found there is moved over once and the old key removed.
 */
class ThemePreferenceRepository(
    private val context: Context,
    private val store: KeyValueStore,
    scope: CoroutineScope,
) {
    private val legacyKey = stringPreferencesKey("theme_preference")
    private val state = MutableStateFlow<ThemePreference?>(null)

    val themePreference: Flow<ThemePreference> = state.filterNotNull()

    init {
        scope.launch { state.value = load() }
    }

    private suspend fun load(): ThemePreference {
        val raw = store.read(KEY) ?: legacy()?.also { old ->
            store.write(KEY, prefsRead(mapOf(KEY to old)).theme)
            runCatching { context.settingsDataStore.edit { it.remove(legacyKey) } }
        }
        return ThemePreference.fromStorage(prefsRead(buildMap { raw?.let { put(KEY, it) } }).theme)
    }

    private suspend fun legacy(): String? =
        context.settingsDataStore.data
            // An unreadable preferences file must degrade to the Auto default, not
            // become a crash loop (DataStore's data flow throws IOException).
            .catch { error -> if (error is IOException) emit(emptyPreferences()) else throw error }
            .map { prefs -> prefs[legacyKey] }
            .first()

    suspend fun setThemePreference(preference: ThemePreference) {
        state.value = preference
        store.write(KEY, preference.storageValue)
    }

    companion object {
        const val KEY = "vela.theme"

        /**
         * Delete the DataStore this preference USED to live in — the erase
         * path's, and nothing else's (spec 081 FR-017).
         *
         * The choice itself is `vela.theme` in the shared store now (spec 072),
         * so an erase already takes it: the core's catalog says it is erasable
         * and `DeviceStorage.erase` sweeps the namespace. What is left is this
         * file. A device that has been opened since 072 migrated its value over
         * and removed the key, but the FILE remains, and a device that never
         * loaded the preference still has the value in it — so an erase that
         * skipped it could hand a reset wallet the old owner's theme.
         *
         * On the companion rather than an instance, because the erase path has
         * no repository and must not build one: constructing it launches a read
         * of the very store that is being emptied.
         *
         * Whole-file `clear()`, not `remove(key)`: anything else that was ever
         * written here is equally the old owner's.
         *
         * @return false when the platform refused; the caller reports the survivor.
         */
        suspend fun clearLegacy(context: Context): Boolean =
            runCatching { context.settingsDataStore.edit { it.clear() } }.isSuccess
    }
}
