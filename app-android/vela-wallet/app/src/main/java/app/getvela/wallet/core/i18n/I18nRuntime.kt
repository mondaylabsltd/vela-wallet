package app.getvela.wallet.core.i18n

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.vela_core_uniffi.I18n
import uniffi.vela_core_uniffi.TOptions
import uniffi.vela_core_uniffi.TVar

/**
 * Runtime wrapper over the vela-core i18n engine (uniffi binding).
 *
 * Lifecycle (data-model state machine): [initialize] constructs the engine with the
 * `en` fallback catalog; [setLocale] loads the resolved catalog and activates it.
 * The engine keeps at most two catalogs resident (active + en) and is internally
 * thread-safe; vars are pre-stringified — one FFI crossing per translation.
 */
class I18nRuntime(
    private val readCatalog: (tag: String) -> ByteArray,
) : VelaStrings {

    private lateinit var engine: I18n

    private val _state = MutableStateFlow(State(ready = false, language = FALLBACK, direction = "ltr"))
    val state: StateFlow<State> = _state

    data class State(val ready: Boolean, val language: String, val direction: String)

    /** Blocking engine construction — call off the main thread. */
    fun initialize(localeTag: String) {
        engine = I18n(readCatalog(FALLBACK))
        applyLocale(localeTag)
        _state.value = State(ready = true, language = engine.language(), direction = engine.dir())
    }

    /** Switches the active language (no-op when already active). */
    fun setLocale(tag: String) {
        check(::engine.isInitialized) { "I18nRuntime.setLocale before initialize" }
        if (tag == _state.value.language && _state.value.ready) return
        applyLocale(tag)
        _state.value = State(ready = true, language = engine.language(), direction = engine.dir())
    }

    private fun applyLocale(tag: String) {
        if (tag != FALLBACK) {
            engine.loadCatalog(tag, readCatalog(tag))
        }
        engine.changeLanguage(tag)
    }

    override fun t(key: String): String = engine.t(key, options(emptyList()))

    override fun t(key: String, vars: Map<String, String>): String =
        engine.t(key, options(vars.map { (name, value) -> TVar(name, value) }))

    override fun t(key: String, count: Int): String =
        engine.t(key, options(emptyList(), count = count.toDouble()))

    private fun options(vars: List<TVar>, count: Double? = null): TOptions = TOptions(
        count = count,
        context = null,
        defaultValue = null,
        lng = null,
        ordinal = false,
        vars = vars,
    )

    companion object {
        const val FALLBACK: String = "en"
    }
}
