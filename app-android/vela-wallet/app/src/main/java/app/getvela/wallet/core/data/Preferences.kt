package app.getvela.wallet.core.data

import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.format.TimeFormatKey
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import uniffi.vela_core_uniffi.prefsLocaleJson
import uniffi.vela_core_uniffi.prefsMigrations
import uniffi.vela_core_uniffi.prefsRead

/**
 * The person's preferences that have no machine (spec 028's ruling, kept in
 * 047): language, the three format presets, the text scale. The record
 * format is the core's (`vela_core::prefs`, spec 072): `vela.language` a tag
 * or `auto`, `vela.localePrefs` the three formats, `vela.textScale` a bare
 * string — the same record on every Vela. This shell once wrote `system` and
 * kept the text size inside `vela.localePrefs`; the core reads that and
 * [load] rewrites it once. The avatar style is retired (spec 074: every avatar
 * is the identicon); [load] hands the core a stored `vela.avatarStyle` so its
 * migrations remove it.
 */
data class PrefsView(
    /** `auto` follows the device's languages; otherwise a shipped tag. */
    val language: String = Preferences.AUTO_LANGUAGE,
    val numberFormat: NumberFormatKey = NumberFormatKey.Auto,
    val dateFormat: DateFormatKey = DateFormatKey.Auto,
    val timeFormat: TimeFormatKey = TimeFormatKey.Auto,
    val textScale: TextScaleLevel = TextScaleLevel.Standard,
    val loaded: Boolean = false,
)

class Preferences(
    private val store: KeyValueStore,
    private val scope: CoroutineScope,
    private val locale: () -> Locale = { Locale.getDefault() },
    /** Called on every change with the formats to draw with; the default publishes `Formats.current`. */
    private val onFormats: (Formats) -> Unit = { Formats.current = it },
) {
    private val _view = MutableStateFlow(PrefsView())
    val view: StateFlow<PrefsView> = _view

    fun load() {
        scope.launch {
            val entries = buildMap {
                for (key in READ_KEYS) store.read(key)?.let { put(key, it) }
            }
            // An older build's spellings, rewritten once (safe every launch:
            // nothing to do once the store agrees).
            for (write in prefsMigrations(entries)) {
                val value = write.value
                if (value == null) store.remove(write.key) else store.write(write.key, value)
            }
            val read = prefsRead(entries)
            publish(
                PrefsView(
                    language = read.language,
                    numberFormat = NumberFormatKey.of(read.numberFormat),
                    dateFormat = DateFormatKey.of(read.dateFormat),
                    timeFormat = TimeFormatKey.of(read.timeFormat),
                    textScale = TextScaleLevel.of(read.textScale),
                    loaded = true,
                ),
            )
        }
    }

    fun setLanguage(tag: String) = update(_view.value.copy(language = tag)) { store.write(KEY_LANGUAGE, tag) }

    fun setNumberFormat(key: NumberFormatKey) = update(_view.value.copy(numberFormat = key)) { writeLocalePrefs(it) }

    fun setDateFormat(key: DateFormatKey) = update(_view.value.copy(dateFormat = key)) { writeLocalePrefs(it) }

    fun setTimeFormat(key: TimeFormatKey) = update(_view.value.copy(timeFormat = key)) { writeLocalePrefs(it) }

    fun setTextScale(level: TextScaleLevel) = update(_view.value.copy(textScale = level)) { store.write(KEY_TEXT_SCALE, level.wire) }

    private fun update(next: PrefsView, persist: suspend (PrefsView) -> Unit) {
        publish(next)
        scope.launch { persist(next) }
    }

    private suspend fun writeLocalePrefs(view: PrefsView) {
        store.write(KEY_LOCALE_PREFS, prefsLocaleJson(view.numberFormat.wire, view.dateFormat.wire, view.timeFormat.wire))
    }

    private fun publish(view: PrefsView) {
        // The formats first: a reader woken by the view must already find the
        // presets it is about to draw with (spec 049).
        onFormats(Formats(number = view.numberFormat, date = view.dateFormat, time = view.timeFormat, locale = locale()))
        _view.value = view
    }

    companion object {
        const val KEY_LANGUAGE = "vela.language"
        const val KEY_LOCALE_PREFS = "vela.localePrefs"
        const val KEY_TEXT_SCALE = "vela.textScale"

        /** Retired (spec 074): read only so the core's migrations can remove it. */
        private const val RETIRED_AVATAR_STYLE = "vela.avatarStyle"

        /** "Follow the device's languages" — the shared word (it was `system` here). */
        const val AUTO_LANGUAGE = "auto"

        /** What the core's codec reads, including the desktop's old `vela.formats` and the retired avatar style. */
        private val READ_KEYS = listOf(
            KEY_LANGUAGE,
            KEY_LOCALE_PREFS,
            KEY_TEXT_SCALE,
            "vela.formats",
            RETIRED_AVATAR_STYLE,
            // Spec 075, renamed 2026-09-23: the core MOVES this to
            // `vela.clearSignerTunnel`, so the migration has to see it.
            KeyValueStore.Keys.RETIRED_CLEAR_SIGNER_RELAY,
        )
    }
}
