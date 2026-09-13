package app.getvela.wallet.feature.scan

import java.math.BigInteger
import java.net.URLDecoder

/**
 * The EIP-681 tokenizer — the phone's port of the web's `services/eip681.ts`
 * (the desktop carries the same in Rust). It only tokenizes: whether a scan
 * locks the Send, whether a chainless request may lock at all, and how base
 * units become a figure are the core's (`send.rs::scan_resolved` /
 * `resolve_locked_request`). Two refusals bought with real bugs stay:
 *  - a call that is not a payment (`/approve`, …) is `null`, never a native
 *    send to the token contract;
 *  - in a `/transfer`, `value` is ether attached to the call, NOT the token
 *    amount — only `uint256` is.
 */
object Eip681 {
    class Request(
        val chainId: Long?,
        val recipient: String,
        val tokenAddress: String?,
        val amountBaseUnits: String?,
        val isNative: Boolean,
    )

    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")
    private val AMOUNT = Regex("^([+-]?)(\\d+)(?:\\.(\\d+))?(?:[eE]([+-]?\\d+))?$")

    fun isHexAddress(s: String): Boolean = ADDRESS.matches(s.trim())

    fun parse(input: String?): Request? {
        var s = input.orEmpty().trim()
        if (!s.startsWith("ethereum:", ignoreCase = true)) return null
        s = s.substring("ethereum:".length)
        if (s.startsWith("pay-")) s = s.substring("pay-".length)
        val q = s.indexOf('?')
        val path = if (q >= 0) s.substring(0, q) else s
        val query = if (q >= 0) s.substring(q + 1) else ""
        val params = parseQuery(query)
        val slash = path.indexOf('/')
        val targetWithChain = if (slash >= 0) path.substring(0, slash) else path
        val functionName = if (slash >= 0) path.substring(slash + 1) else ""
        val at = targetWithChain.indexOf('@')
        val target = (if (at >= 0) targetWithChain.substring(0, at) else targetWithChain).trim()
        val chainText = if (at >= 0) targetWithChain.substring(at + 1) else ""
        if (target.isEmpty()) return null
        val chainId = chainText.takeIf { it.isNotEmpty() && it.all(Char::isDigit) }?.toLongOrNull()
        if (functionName == "transfer") {
            val recipient = params["address"].orEmpty().trim()
            if (!isHexAddress(target) || !isHexAddress(recipient)) return null
            return Request(
                chainId = chainId,
                recipient = recipient,
                tokenAddress = target,
                amountBaseUnits = params["uint256"]?.let(::parseAmount),
                isNative = false,
            )
        }
        if (functionName.isNotEmpty()) return null
        if (!isHexAddress(target)) return null
        return Request(
            chainId = chainId,
            recipient = target,
            tokenAddress = null,
            amountBaseUnits = params["value"]?.let(::parseAmount),
            isNative = true,
        )
    }

    /** `1e18`, `2.5e6`, `1000`; a negative answers `null` (nothing), as the web. */
    internal fun parseAmount(raw: String): String? {
        val m = AMOUNT.matchEntire(raw.trim()) ?: return null
        val (sign, intDigits, fracDigits, expText) = m.destructured
        if (sign == "-") return null
        val exp = if (expText.isEmpty()) 0 else expText.toIntOrNull() ?: return null
        val digits = intDigits + fracDigits
        val decimalExp = exp - fracDigits.length
        if (decimalExp > 512) return null
        val value = BigInteger(digits)
        return if (decimalExp >= 0) {
            (value * BigInteger.TEN.pow(decimalExp)).toString()
        } else {
            value.divide(BigInteger.TEN.pow(-decimalExp)).toString()
        }
    }

    internal fun parseQuery(query: String): Map<String, String> {
        if (query.isEmpty()) return emptyMap()
        val out = LinkedHashMap<String, String>()
        query.split('&').forEach { pair ->
            if (pair.isEmpty()) return@forEach
            val eq = pair.indexOf('=')
            val key = if (eq >= 0) pair.substring(0, eq) else pair
            val value = if (eq >= 0) pair.substring(eq + 1) else ""
            out[decode(key)] = decode(value)
        }
        return out
    }

    private fun decode(raw: String): String =
        runCatching { URLDecoder.decode(raw.replace("+", "%2B"), "UTF-8") }.getOrDefault(raw)
}
