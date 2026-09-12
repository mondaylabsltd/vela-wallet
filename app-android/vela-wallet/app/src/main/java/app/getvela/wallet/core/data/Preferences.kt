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
import org.json.JSONObject

/**
 * The person's preferences that have no machine (spec 028's ruling, kept in
 * 047): language, the three format presets, the text scale, the avatar
 * style. Stored under the web's keys byte-for-byte (`vela.language` a bare
 * tag or `system`; `vela.localePrefs` one JSON object; `vela.avatarStyle` a
 * bare string), so a person's phone and browser would read the same record.
 */
data class PrefsView(
    val language: String = "system",
    val numberFormat: NumberFormatKey = NumberFormatKey.Auto,
    val dateFormat: DateFormatKey = DateFormatKey.Auto,
    val timeFormat: TimeFormatKey = TimeFormatKey.Auto,
    val textScale: TextScaleLevel = TextScaleLevel.Standard,
    val avatarStyle: String = "identicon",
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
            val language = store.read(KEY_LANGUAGE)?.takeIf { it.isNotBlank() } ?: "system"
            val prefs = runCatching { JSONObject(store.read(KEY_LOCALE_PREFS).orEmpty()) }.getOrNull()
            val avatar = store.read(KEY_AVATAR_STYLE)?.takeIf { it == "initials" || it == "identicon" } ?: "identicon"
            publish(
                PrefsView(
                    language = language,
                    numberFormat = NumberFormatKey.of(prefs?.optString("numberFormat")),
                    dateFormat = DateFormatKey.of(prefs?.optString("dateFormat")),
                    timeFormat = TimeFormatKey.of(prefs?.optString("timeFormat")),
                    textScale = TextScaleLevel.of(prefs?.optString("textScale")),
                    avatarStyle = avatar,
                    loaded = true,
                ),
            )
        }
    }

    fun setLanguage(tag: String) = update(_view.value.copy(language = tag)) { store.write(KEY_LANGUAGE, tag) }

    fun setNumberFormat(key: NumberFormatKey) = update(_view.value.copy(numberFormat = key)) { writeLocalePrefs(it) }

    fun setDateFormat(key: DateFormatKey) = update(_view.value.copy(dateFormat = key)) { writeLocalePrefs(it) }

    fun setTimeFormat(key: TimeFormatKey) = update(_view.value.copy(timeFormat = key)) { writeLocalePrefs(it) }

    fun setTextScale(level: TextScaleLevel) = update(_view.value.copy(textScale = level)) { writeLocalePrefs(it) }

    fun setAvatarStyle(style: String) = update(_view.value.copy(avatarStyle = style)) { store.write(KEY_AVATAR_STYLE, style) }

    private fun update(next: PrefsView, persist: suspend (PrefsView) -> Unit) {
        publish(next)
        scope.launch { persist(next) }
    }

    private suspend fun writeLocalePrefs(view: PrefsView) {
        store.write(
            KEY_LOCALE_PREFS,
            JSONObject()
                .put("numberFormat", view.numberFormat.wire)
                .put("dateFormat", view.dateFormat.wire)
                .put("timeFormat", view.timeFormat.wire)
                .put("textScale", view.textScale.wire)
                .toString(),
        )
    }

    private fun publish(view: PrefsView) {
        _view.value = view
        onFormats(Formats(number = view.numberFormat, date = view.dateFormat, time = view.timeFormat, locale = locale()))
    }

    companion object {
        const val KEY_LANGUAGE = "vela.language"
        const val KEY_LOCALE_PREFS = "vela.localePrefs"
        const val KEY_AVATAR_STYLE = "vela.avatarStyle"
    }
}
