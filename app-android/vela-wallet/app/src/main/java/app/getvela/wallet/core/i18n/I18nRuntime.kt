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
    /**
     * Told the active language before [state] says it: the app's own runtime
     * publishes it to [AppLanguage] here, so a figure redrawn because the
     * language changed is already drawn in the new one. Nothing by default —
     * a test's or a board's runtime speaks only for itself.
     */
    private val onLanguage: (String) -> Unit = {},
    /**
     * An on-demand namespace's catalog for a language (`assets/i18n/<name>/<tag>.json`,
     * [I18nCatalogs]), or null where there is none. Read only once a screen
     * that draws the namespace asks for it ([include]).
     */
    private val readOnDemand: (namespace: String, tag: String) -> ByteArray? = { _, _ -> null },
    private val readCatalog: (tag: String) -> ByteArray,
) : VelaStrings {

    private lateinit var engine: I18n

    private val _state = MutableStateFlow(State(ready = false, language = FALLBACK, direction = "ltr"))
    val state: StateFlow<State> = _state

    /**
     * [namespaces]: the on-demand namespaces resident now — a screen that
     * draws one reads it here, so it redraws once its words are in.
     */
    data class State(
        val ready: Boolean,
        val language: String,
        val direction: String,
        val namespaces: Set<String> = emptySet(),
    )

    /** The on-demand namespaces asked for so far; they stay resident across language changes. */
    private val included = linkedSetOf<String>()

    /** Blocking engine construction — call off the main thread. */
    @Synchronized
    fun initialize(localeTag: String) {
        engine = I18n(catalog(FALLBACK))
        applyLocale(localeTag)
        onLanguage(engine.language())
        publish()
    }

    /** Switches the active language (no-op when already active). */
    @Synchronized
    fun setLocale(tag: String) {
        check(::engine.isInitialized) { "I18nRuntime.setLocale before initialize" }
        if (tag == _state.value.language && _state.value.ready) return
        applyLocale(tag)
        onLanguage(engine.language())
        publish()
    }

    /**
     * Loads an on-demand namespace ([I18nCatalogs.ON_DEMAND]) into the
     * fallback and the active catalog, for a screen that draws it — the web's
     * `catalogBytes` merge, done when it is first needed instead of at build
     * time. Blocking file + FFI work (call off the main thread); a no-op when
     * it is already resident. Before [initialize] it is only remembered, and
     * the first catalog loads with it.
     */
    @Synchronized
    fun include(namespace: String) {
        if (!included.add(namespace)) return
        if (!::engine.isInitialized) return
        val active = engine.language()
        engine.loadCatalog(FALLBACK, catalog(FALLBACK))
        if (active != FALLBACK) engine.loadCatalog(active, catalog(active))
        engine.changeLanguage(active)
        publish()
    }

    private fun publish() {
        _state.value = State(
            ready = true,
            language = engine.language(),
            direction = engine.dir(),
            namespaces = included.toSet(),
        )
    }

    /** The language's main catalog with every included namespace deep-merged in ([I18nCatalogs.merged]). */
    private fun catalog(tag: String): ByteArray =
        I18nCatalogs.merged(readCatalog(tag), included.mapNotNull { readOnDemand(it, tag) })

    private fun applyLocale(tag: String) {
        if (tag != FALLBACK) {
            engine.loadCatalog(tag, catalog(tag))
        }
        engine.changeLanguage(tag)
    }

    override fun t(key: String): String = engine.t(key, options(emptyList()))

    override fun t(key: String, vars: Map<String, String>): String =
        engine.t(key, options(vars.map { (name, value) -> TVar(name, value) }))

    override fun t(key: String, count: Int): String =
        engine.t(key, options(emptyList(), count = count.toDouble()))

    override fun relativeTime(tsSeconds: Long, nowMs: Long, utcOffsetMinutes: Int, dateFormat: String): String =
        engine.formatRelativeTime(tsSeconds, nowMs, utcOffsetMinutes, dateFormat)

    override val language: String get() = _state.value.language

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
