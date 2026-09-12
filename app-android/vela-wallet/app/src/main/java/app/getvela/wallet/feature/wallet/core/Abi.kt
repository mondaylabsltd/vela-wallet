package app.getvela.wallet.feature.wallet.core

import java.math.BigInteger

/**
 * Minimal ABI encoding and decoding for the read path: Multicall3, ERC-20,
 * Uniswap-V3 QuoterV2, Solidly routers and Chainlink feeds. Pure hex, no
 * dependency.
 *
 * Port of `app-web/vela-wallet/src/lib/services/abi.ts`.
 *
 * **Why this is a shell file and not a core one.** The codebase already drew
 * this line and it is worth stating rather than re-deciding: the *judgement*
 * about a price — which pool wins inside a quote token, which stable wins
 * across them, whether a DEX quote survives the sanity band — lives in
 * `vela_core::app::balance_dashboard` and is called from here through uniffi.
 * What is in this file is transport framing: 32-byte words and offsets. The
 * web made the same split deliberately.
 *
 * **What that costs, honestly.** This is a second hand-written copy of the
 * framing, and there is no generated artifact to drift-test it against — the
 * way `CoreWireDriftTest` guards the wire types. The mitigation is that every
 * function here is pinned against a known encoding in `AbiTest`, and the whole
 * path is exercised against real chains on a device. `first_grouped_quote_price`
 * — the custom-token rule, which IS judgement and today exists only in the
 * web's TypeScript — is deliberately NOT ported here; it belongs in the core
 * before Android prices a custom token.
 */
object Abi {

    /** The canonical Multicall3 deployment, identical on every EVM chain. */
    const val MULTICALL3 = "0xcA11bde05977b3631167028862bE2a173976CA11"

    // First 4 bytes of keccak256 over the canonical signature.
    private const val SEL_AGGREGATE3 = "82ad56cb" // aggregate3((address,bool,bytes)[])
    private const val SEL_GET_ETH_BALANCE = "4d2301cc" // getEthBalance(address)
    private const val SEL_BALANCE_OF = "70a08231" // balanceOf(address)
    private const val SEL_DECIMALS = "313ce567" // decimals()
    private const val SEL_QUOTE_V3 = "c6a5026a" // quoteExactInputSingle((address,address,uint256,uint24,uint160))
    private const val SEL_GET_AMOUNTS_OUT = "5509a1ac" // getAmountsOut(uint256,(address,address,bool,address)[])
    private const val SEL_SYMBOL = "95d89b41" // symbol()
    private const val SEL_NAME = "06fdde03" // name()
    private const val SEL_ALLOWANCE = "dd62ed3e" // allowance(address,address)
    private const val SEL_LATEST_ROUND = "feaf968c" // latestRoundData()

    /** One entry in a Multicall3 batch. `allowFailure` is always true here. */
    data class Call(val target: String, val callData: String)

    /** One answer from a batch: whether the call succeeded, and its return data. */
    data class CallResult(val success: Boolean, val data: String)

    // -- encoding ------------------------------------------------------------

    /**
     * `aggregate3(Call3[])`, where `Call3 = (address, bool, bytes)`.
     *
     * The tuple holds `bytes`, which is dynamic, so each element is encoded
     * first and then addressed by a byte offset measured from the start of the
     * element area — the word right after the array length.
     */
    fun encodeAggregate3(calls: List<Call>): String {
        val elements = calls.map { call ->
            val data = strip(call.callData)
            word(BigInteger(strip(call.target).lowercase(), 16)) + // address target
                word(BigInteger.ONE) + //                            bool allowFailure
                word(BigInteger.valueOf(0x60)) + //                   offset to bytes (3 words)
                word(BigInteger.valueOf((data.length / 2).toLong())) + // bytes length
                padRight(data)
        }

        val head = StringBuilder()
        head.append(SEL_AGGREGATE3)
        head.append(word(BigInteger.valueOf(0x20))) // offset to the one dynamic param
        head.append(word(BigInteger.valueOf(calls.size.toLong())))

        var offset = calls.size * 32L // past the offset slots themselves
        elements.forEach { element ->
            head.append(word(BigInteger.valueOf(offset)))
            offset += element.length / 2
        }
        elements.forEach(head::append)
        return "0x$head"
    }

    fun encodeBalanceOf(address: String): String = "0x" + SEL_BALANCE_OF + addressWord(address)

    fun encodeDecimals(): String = "0x$SEL_DECIMALS"

    fun encodeSymbol(): String = "0x$SEL_SYMBOL"

    fun encodeName(): String = "0x$SEL_NAME"

    fun encodeAllowance(owner: String, spender: String): String = "0x" + SEL_ALLOWANCE + addressWord(owner) + addressWord(spender)

    fun encodeGetEthBalance(address: String): String =
        "0x" + SEL_GET_ETH_BALANCE + addressWord(address)

    fun encodeLatestRound(): String = "0x$SEL_LATEST_ROUND"

    /**
     * Uniswap-V3 `quoteExactInputSingle`. The struct is static, so the encoding
     * is the fields in order; `sqrtPriceLimitX96` is 0 (no limit).
     */
    fun encodeQuoteV3(
        tokenIn: String,
        tokenOut: String,
        amountIn: BigInteger,
        fee: Int,
    ): String = "0x" + SEL_QUOTE_V3 +
        addressWord(tokenIn) +
        addressWord(tokenOut) +
        word(amountIn) +
        word(BigInteger.valueOf(fee.toLong())) +
        word(BigInteger.ZERO)

    /**
     * Solidly (Aerodrome/Velodrome V2) `getAmountsOut(uint256, Route[])` for a
     * single hop. `Route = (address from, address to, bool stable, address
     * factory)`; a zero factory means the router's default.
     */
    fun encodeGetAmountsOut(
        amountIn: BigInteger,
        tokenIn: String,
        tokenOut: String,
        stable: Boolean,
    ): String = "0x" + SEL_GET_AMOUNTS_OUT +
        word(amountIn) +
        word(BigInteger.valueOf(0x40)) + // offset to Route[] — two head words
        word(BigInteger.ONE) + //           routes.length
        addressWord(tokenIn) +
        addressWord(tokenOut) +
        word(if (stable) BigInteger.ONE else BigInteger.ZERO) +
        word(BigInteger.ZERO)

    // -- decoding ------------------------------------------------------------

    /**
     * `aggregate3 → Result[]`, where `Result = (bool success, bytes data)`.
     *
     * Every bound is checked because this parses a remote answer: a truncated
     * or hostile response must produce fewer results, never an exception and
     * never a wrong number. A missing element is reported as a failed call,
     * which is what the core already knows how to handle.
     */
    fun decodeAggregate3(hex: String): List<CallResult> {
        val d = strip(hex)
        if (d.length < 128) return emptyList()

        val arrayOffset = wordAt(d, 0) * 2
        if (arrayOffset < 0 || arrayOffset + 64 > d.length) return emptyList()

        val count = wordAt(d, arrayOffset)
        if (count <= 0 || count > 10_000) return emptyList() // sanity cap
        val offsetsStart = arrayOffset + 64

        val results = ArrayList<CallResult>(count)
        for (index in 0 until count) {
            val offsetPos = offsetsStart + index * 64
            if (offsetPos + 64 > d.length) break

            val elementPos = offsetsStart + wordAt(d, offsetPos) * 2
            if (elementPos < 0 || elementPos + 128 > d.length) {
                results.add(CallResult(success = false, data = "0x"))
                continue
            }

            val success = wordAt(d, elementPos) != 0
            val bytesPos = elementPos + wordAt(d, elementPos + 64) * 2
            if (bytesPos < 0 || bytesPos + 64 > d.length) {
                results.add(CallResult(success, "0x"))
                continue
            }

            val length = wordAt(d, bytesPos)
            val end = bytesPos + 64 + length * 2
            val data = if (length > 0 && end <= d.length && end > bytesPos + 64) {
                "0x" + d.substring(bytesPos + 64, end)
            } else {
                "0x"
            }
            results.add(CallResult(success, data))
        }
        return results
    }

    /** The first 256-bit word, unsigned. Short input reads as zero, not as an error. */
    fun decodeUint256(hex: String): BigInteger {
        val d = strip(hex)
        if (d.length < 64) return BigInteger.ZERO
        return runCatching { BigInteger(d.substring(0, 64), 16) }.getOrDefault(BigInteger.ZERO)
    }

    /** `decimals()`. */
    fun decodeUint8(hex: String): Int = decodeUint256(hex).toInt()

    /**
     * An ABI `string` return — `symbol()` and `name()`.
     *
     * Two layouts in the wild: the modern `[offset][length][data]`, and a
     * single fixed 32-byte word from legacy tokens that declared `bytes32`
     * (MKR is the famous one). A length that cannot be a length means this is
     * the legacy shape, not a corrupt string.
     *
     * Decoded as UTF-8 so multibyte symbols survive — "USD₮0" is a real
     * token, and rendering it as mojibake makes a legitimate holding look like
     * a scam.
     *
     * Returns `null` rather than `""` when nothing readable is there: an
     * unresolvable symbol is a fact the core acts on, and an empty string
     * would read as a token that answered with a blank name.
     */
    fun decodeString(hex: String): String? {
        val d = strip(hex)
        if (d.length < 64) return null
        if (d.length < 128) return utf8(d.substring(0, 64))

        val length = wordAt(d, 64)
        if (length <= 0 || length > 4096 || 128 + length * 2 > d.length) {
            return utf8(d.substring(0, 64))
        }
        return utf8(d.substring(128, 128 + length * 2))
    }

    /** Hex bytes as UTF-8, stopping at the first NUL (bytes32 padding). */
    private fun utf8(dataHex: String): String? {
        val bytes = ArrayList<Byte>(dataHex.length / 2)
        var index = 0
        while (index + 1 < dataHex.length) {
            val byte = dataHex.substring(index, index + 2).toIntOrNull(16) ?: break
            if (byte == 0) break
            bytes.add(byte.toByte())
            index += 2
        }
        if (bytes.isEmpty()) return null
        return String(bytes.toByteArray(), Charsets.UTF_8).trim().ifBlank { null }
    }

    /**
     * `getAmountsOut → uint256[]`. For one hop the array is
     * `[amountIn, amountOut]`, and the answer is the last element:
     * `offset | length | amounts[0] | amounts[1]`.
     */
    fun decodeAmountsOut(hex: String): BigInteger {
        val d = strip(hex)
        if (d.length < 256) return BigInteger.ZERO
        return runCatching { BigInteger(d.substring(192, 256), 16) }
            .getOrDefault(BigInteger.ZERO)
    }

    /**
     * Chainlink `latestRoundData → (roundId, answer, startedAt, updatedAt,
     * answeredInRound)` — the USD price is `answer`, the second word, with the
     * feeds' fixed 8 decimals.
     *
     * `answer` is `int256`: a negative value is a feed reporting something this
     * cannot interpret as a price, and it must not wrap into an enormous
     * positive one.
     *
     * **A deliberate divergence from the web port.** `decChainlinkUsd` there
     * reads the word as unsigned ("always positive for USD feeds") while its
     * sibling `decChainlinkAnswer` applies the sign. A negative answer would
     * therefore decode as ~1.16e69 and sail through the caller's `> 0` gate.
     * The feeds are positive in practice, so this is defence rather than a
     * fixed bug — but a wallet should not have a path where one bad word prices
     * a coin at 10^69 dollars.
     *
     * Returns `null` rather than a number for anything unusable, which is the
     * `isFinite && > 0` gate `choose_native_price` documents as the shell's.
     */
    fun decodeChainlinkUsd(hex: String): Double? = decodeChainlinkAnswer(hex, USD_FEED_DECIMALS)

    /**
     * The same answer, scaled by a decimals the caller read from the feed.
     *
     * The USD price feeds all use 8, but the FIAT feeds do not — PHP reports 18
     * — so anything reading a currency feed must pass what that feed said about
     * itself. Assuming 8 there is a rate off by a factor of 10^10.
     */
    fun decodeChainlinkAnswer(hex: String, decimals: Int): Double? {
        val d = strip(hex)
        if (d.length < 128) return null
        if (decimals < 0 || decimals > 38) return null
        val raw = runCatching { BigInteger(d.substring(64, 128), 16) }.getOrNull() ?: return null
        val signed = if (raw.bitLength() >= 256) raw.subtract(TWO_POW_256) else raw
        if (signed.signum() <= 0) return null
        val price = signed.toDouble() / StrictMath.pow(10.0, decimals.toDouble())
        return if (price.isFinite() && price > 0.0) price else null
    }

    // -- helpers -------------------------------------------------------------

    private val TWO_POW_256: BigInteger = BigInteger.ONE.shiftLeft(256)

    /** Every Chainlink USD PRICE feed reports 8. The fiat feeds do not. */
    private const val USD_FEED_DECIMALS = 8

    private fun strip(hex: String): String = if (hex.startsWith("0x")) hex.substring(2) else hex

    /** A 32-byte left-padded word. */
    private fun word(value: BigInteger): String = value.toString(16).padStart(64, '0')

    private fun addressWord(address: String): String =
        strip(address).lowercase().padStart(64, '0')

    /** Right-pad dynamic data to a 32-byte boundary. */
    private fun padRight(hex: String): String {
        val words = (hex.length + 63) / 64
        return hex.padEnd(words * 64, '0')
    }

    /**
     * A 256-bit word read as an Int, for lengths and offsets.
     *
     * These are positions inside a byte string this device just received, so a
     * value that cannot be a position — negative, or beyond any plausible
     * response — answers -1 and every caller bounds-checks it. Truncating a
     * huge word into a valid-looking index is how a decoder reads a response as
     * something it is not.
     */
    private fun wordAt(hex: String, position: Int): Int {
        if (position < 0 || position + 64 > hex.length) return -1
        val value = runCatching { BigInteger(hex.substring(position, position + 64), 16) }
            .getOrNull() ?: return -1
        if (value.signum() < 0 || value.bitLength() > 31) return -1
        return value.toInt()
    }
}
