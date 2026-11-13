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

    /**
     * Replaced whole (never edited) when an on-demand namespace comes in —
     * the core's `en` is a field fixed at construction, so a namespace can
     * only reach the fallback through a new engine ([include]). Readers take
     * whichever engine is current; each is complete when it is published.
     */
    @Volatile
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
        snapshot()
    }

    /** Switches the active language (no-op when already active). */
    @Synchronized
    fun setLocale(tag: String) {
        check(::engine.isInitialized) { "I18nRuntime.setLocale before initialize" }
        if (tag == _state.value.language && _state.value.ready) return
        applyLocale(tag)
        onLanguage(engine.language())
        publish()
        snapshot()
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
        // r10-2: the core's `en` is a field set at construction, and
        // `load_catalog` fills only the ACTIVE slot. Loading the merged
        // English there left the pinned fallback without the namespace: the
        // next switch to English (or any key a language lacks) fell through
        // to it and drew raw keys until a restart. A new engine, built with
        // the merged English and then the active language, is published only
        // once it is whole.
        val next = I18n(catalog(FALLBACK))
        if (active != FALLBACK) next.loadCatalog(active, catalog(active))
        next.changeLanguage(active)
        engine = next
        plain = emptyMap()
        filled = Filled()
        publish()
        snapshot()
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
        // The old language's words go at once; until the new snapshot is in,
        // every word is the core's, asked per label as before.
        plain = emptyMap()
        filled = Filled()
    }

    /**
     * The language's plain words, taken whole (`I18n.plain_strings("")`): one
     * crossing per language (or namespace) change instead of one per label.
     * Taken AFTER [publish]: the splash waits for `ready`, not for this, and
     * a word asked in between is answered by the core directly. Under the
     * same lock as the change it follows, so it is always the language now
     * active.
     */
    private fun snapshot() {
        plain = runCatching { engine.plainStrings("") }.getOrDefault(emptyMap())
    }

    /**
     * Every plain string of the active language as `t(key)` says it, taken
     * from the core whole after every catalog or language change
     * ([applyLocale], [include]; core-perf-1 `plain_strings`). `t(key)` is the
     * commonest call a screen makes — every label of every row, again on
     * every recomposition — and used to be an FFI crossing each (then a
     * per-label memo). Replaced, never edited: a reader holds the map of one
     * language, and a word from the old language can never be served from
     * the new one. A key it does not hold (a branch, a key nothing resolves)
     * goes to the core, which answers it as before.
     */
    @Volatile
    private var plain: Map<String, String> = emptyMap()

    override fun t(key: String): String =
        plain[key] ?: engine.t(key, options(emptyList()))

    /**
     * Many keys, in the order asked: plain ones from [plain], the rest — and
     * every key with [vars] — in ONE crossing (`I18n.t_many`).
     */
    override fun tMany(keys: List<String>, vars: Map<String, String>): List<String> {
        if (keys.isEmpty()) return emptyList()
        if (vars.isEmpty()) {
            val memo = plain
            val missing = keys.filter { it !in memo }
            if (missing.isEmpty()) return keys.map { memo.getValue(it) }
            val asked = missing.distinct()
            val answered = asked.zip(engine.tMany(asked, options(emptyList()))).toMap()
            return keys.map { memo[it] ?: answered.getValue(it) }
        }
        return engine.tMany(keys, options(vars.map { (name, value) -> TVar(name, value) }))
    }

    override fun t(key: String, vars: Map<String, String>): String =
        filled.answer("$key\u0000$vars") { engine.t(key, options(vars.map { (name, value) -> TVar(name, value) })) }

    override fun t(key: String, count: Int): String =
        filled.answer("$key\u0000#$count") { engine.t(key, options(emptyList(), count = count.toDouble())) }

    /**
     * r13 (the ANR typing in Send): a word with values, as the core filled it
     * in the active language — kept for the next recomposition that asks the
     * same (a form says dozens on every keystroke, each an FFI crossing).
     * Replaced, never edited, with [plain]: a language or catalog change
     * starts it empty. Bounded: the oldest answers go first.
     */
    @Volatile
    private var filled = Filled()

    private class Filled {
        private val answers = object : LinkedHashMap<String, String>(256, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, String>?): Boolean = size > MAX_FILLED
        }

        fun answer(key: String, ask: () -> String): String {
            synchronized(answers) { answers[key] }?.let { return it }
            val said = ask()
            synchronized(answers) { answers[key] = said }
            return said
        }
    }

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

        /** How many filled words [filled] keeps. */
        private const val MAX_FILLED = 1024
    }
}
