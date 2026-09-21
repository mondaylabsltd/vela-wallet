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
    }
}
