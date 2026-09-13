package app.getvela.wallet.core.marks

/**
 * The web's logo rules (`services/tokens-model.ts` + `flows/marks.ts`), ported
 * (spec 047, the founder's ruling of 2026-09-05 and 2026-09-12): logos come
 * from the chain-data endpoint with the drawn glyph as the fallback; a native
 * coin's logo is the COIN's chain (ETH on Base is still Ethereum's logo); the
 * chain badge is hidden when it would repeat the token (ETH on Ethereum, XDAI
 * on Gnosis); a token's logo is `assets/eip155-<chain>/<checksummed>/logo.png`
 * with the lowercase path as the fallback.
 */
object Marks {
    /** The chain-data endpoint's base (settings' EthereumData), set by the container; empty = no logos, glyphs only. */
    @Volatile
    var base: String = ""

    fun chainLogoUrl(chainId: Int): String? = base.takeIf { it.isNotBlank() }?.let { "${it.trimEnd('/')}/chainlogos/eip155-$chainId.png" }

    fun nativeCoinLogoChainId(symbol: String, fallbackChainId: Int): Int = when (symbol.uppercase()) {
        "ETH" -> 1
        "BNB" -> 56
        "POL", "MATIC" -> 137
        "AVAX" -> 43114
        "XDAI" -> 100
        else -> fallbackChainId
    }

    /** The badge's chain, or `null` when the badge would show the same logo as the token. */
    fun badgeChainId(chainId: Int, symbol: String, tokenAddress: String?): Int? =
        if (tokenAddress == null && nativeCoinLogoChainId(symbol, chainId) == chainId) null else chainId

    fun tokenLogoUrls(chainId: Int, symbol: String, tokenAddress: String?): List<String> {
        if (tokenAddress == null) return listOfNotNull(chainLogoUrl(nativeCoinLogoChainId(symbol, chainId)))
        val root = base.takeIf { it.isNotBlank() }?.trimEnd('/') ?: return emptyList()
        if (!ADDRESS.matches(tokenAddress)) return emptyList()
        val lower = tokenAddress.lowercase()
        val checksummed = runCatching { uniffi.vela_core_uniffi.checksumAddress(lower) }.getOrNull() ?: lower
        val urls = mutableListOf("$root/assets/eip155-$chainId/$checksummed/logo.png")
        if (lower != checksummed) urls.add("$root/assets/eip155-$chainId/$lower/logo.png")
        return urls
    }

    class Mark(val logoUrls: List<String>, val badgeLogoUrl: String?, val badgeHidden: Boolean)

    /** `tokenMarkFor`: named logo URLs first (the core's `logo_urls`), then the known candidates. */
    fun tokenMark(chainId: Int, symbol: String, tokenAddress: String?, logoUrls: List<String> = emptyList()): Mark {
        val badge = badgeChainId(chainId, symbol, tokenAddress)
        val named = logoUrls.filter { it.isNotEmpty() }
        val known = tokenLogoUrls(chainId, symbol, tokenAddress).filter { it !in named }
        return Mark(logoUrls = named + known, badgeLogoUrl = badge?.let(::chainLogoUrl), badgeHidden = badge == null)
    }

    /** `chainMark`: a network by itself — its own logo, no badge. */
    fun chainMark(chainId: Int): Mark = Mark(listOfNotNull(chainLogoUrl(chainId)), null, badgeHidden = true)

    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")
}
