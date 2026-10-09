package app.getvela.wallet.core.marks

import uniffi.vela_core_uniffi.MarkView

/**
 * Which logo a token or a network wears — asked of the core, which holds the
 * one rule every shell draws by (`vela_core::app::remote_mark`; `MarksTest`
 * replays its vectors, `rust/crates/vela-core/tests/vectors/marks.json`).
 * This object decides nothing: it keeps the person's chain-data endpoint as
 * Settings stores it and hands it over with each question.
 *
 * What the core decides: logos come from that endpoint, the built-in one when
 * the field is blank; a native coin wears its own home chain's logo (ETH on
 * Base is Ethereum's); the badge is left off where it would repeat the coin;
 * a token's logo is its asset entry, checksummed then lowercase, after any
 * URL an index already named; chain 0 asks for nothing.
 *
 * **The kind rule.** Anything that names a NETWORK (a network row or fact, a
 * notice that locks a chain, a receive row, the QR centre, a chip) wears
 * [chainMark] or [chainLogoUrl]; anything that names a COIN wears [tokenMark].
 * The network row of ETH sent on Base wears Base's logo, never Ethereum's.
 */
object Marks {
    /**
     * The person's ethereum-data endpoint as stored (Settings → 服务节点),
     * set by the container. Blank is not "no logos": the core reads it as the
     * built-in endpoint, so a mark asked for before Settings has loaded still
     * gets its logo.
     */
    @Volatile
    var base: String = ""

    /** `{endpoint}/chainlogos/eip155-{id}.png` — a network's own logo; `null` on chain 0. */
    fun chainLogoUrl(chainId: Int): String? {
        val base = base
        return remember(Key.Logo(base, chainId)) { Answer(url = uniffi.vela_core_uniffi.chainLogoUrl(base, chain(chainId))) }.url
    }

    /**
     * A coin's mark. [tokenAddress] is the contract, `null` for the chain's
     * own coin (never ""); [named] are logo URLs an index already gave it,
     * tried first.
     */
    fun tokenMark(chainId: Int, symbol: String, tokenAddress: String?, named: List<String> = emptyList()): MarkView {
        val base = base
        return remember(Key.Token(base, chainId, symbol, tokenAddress, named)) {
            Answer(mark = uniffi.vela_core_uniffi.tokenMark(base, chain(chainId), symbol, tokenAddress, named))
        }.mark!!
    }

    /** A network drawn as itself: its own logo, its coin's letters under it, never a badge. */
    fun chainMark(chainId: Int, nativeSymbol: String): MarkView {
        val base = base
        return remember(Key.Chain(base, chainId, nativeSymbol)) {
            Answer(mark = uniffi.vela_core_uniffi.chainMark(base, chain(chainId), nativeSymbol))
        }.mark!!
    }

    /** A chain id that is not one (negative) is chain 0, on which the core builds no URL. */
    private fun chain(chainId: Int): UInt = if (chainId < 0) 0u else chainId.toUInt()

    // The builders ask again on every recomposition of a list; the answer is
    // a pure function of the question, so a recent one is kept rather than
    // crossing the FFI per row per frame. The endpoint is part of the key, so
    // a changed 服务节点 is a new question.
    private sealed interface Key {
        data class Logo(val base: String, val chainId: Int) : Key
        data class Token(val base: String, val chainId: Int, val symbol: String, val tokenAddress: String?, val named: List<String>) : Key
        data class Chain(val base: String, val chainId: Int, val nativeSymbol: String) : Key
    }

    /** What the core answered: a logo URL (possibly none) or a mark. */
    private class Answer(val url: String? = null, val mark: MarkView? = null)

    private const val REMEMBERED = 512

    private val answers = object : LinkedHashMap<Key, Answer>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Key, Answer>): Boolean = size > REMEMBERED
    }

    private fun remember(key: Key, ask: () -> Answer): Answer =
        synchronized(answers) { answers[key] } ?: ask().also { answer -> synchronized(answers) { answers[key] = answer } }
}
