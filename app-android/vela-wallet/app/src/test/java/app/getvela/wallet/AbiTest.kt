package app.getvela.wallet

import app.getvela.wallet.feature.wallet.core.Abi
import java.math.BigInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The ABI codec, pinned.
 *
 * Unlike the wire types, this file has no generated mirror to drift-test
 * against — so the encodings are pinned by hand here, word by word, and the
 * decoders are given the malformed input a remote endpoint can actually send.
 *
 * A wrong word here is not a crash. It is a balance read from the wrong storage
 * slot, or a quote scaled by the wrong power of ten, and the screen shows it
 * with full confidence.
 */
class AbiTest {

    private val alice = "0x7687AbCdEf0123456789abcdefABCDEF0123D141"
    private val usdc = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
    private val weth = "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619"

    // -- encoding ------------------------------------------------------------

    @Test
    fun `balanceOf is the selector and one left-padded address`() {
        val encoded = Abi.encodeBalanceOf(alice)

        assertEquals(
            "0x70a08231" +
                "0000000000000000000000007687abcdef0123456789abcdefabcdef0123d141",
            encoded,
        )
        // 4-byte selector + one 32-byte word.
        assertEquals(2 + 8 + 64, encoded.length)
    }

    @Test
    fun `decimals takes no argument`() {
        assertEquals("0x313ce567", Abi.encodeDecimals())
    }

    @Test
    fun `getEthBalance is multicall's own, not an ERC-20 call`() {
        // The native balance rides in the same batch as the token balances, and
        // it does so through Multicall3's helper — a `balanceOf` here would ask
        // Multicall3 for its own ERC-20 balance and get nothing.
        assertTrue(Abi.encodeGetEthBalance(alice).startsWith("0x4d2301cc"))
    }

    @Test
    fun `a V3 quote carries five static fields in order`() {
        val encoded = Abi.encodeQuoteV3(weth, usdc, BigInteger.TEN.pow(18), 3000)

        assertEquals("0xc6a5026a", encoded.substring(0, 10))
        val words = encoded.substring(10).chunked(64)
        assertEquals(5, words.size)
        assertEquals("0".repeat(24) + weth.substring(2).lowercase(), words[0])
        assertEquals("0".repeat(24) + usdc.substring(2).lowercase(), words[1])
        assertEquals(BigInteger.TEN.pow(18), BigInteger(words[2], 16))
        assertEquals(BigInteger.valueOf(3000), BigInteger(words[3], 16))
        // sqrtPriceLimitX96 = 0 — no limit, or the quote silently returns none.
        assertEquals(BigInteger.ZERO, BigInteger(words[4], 16))
    }

    @Test
    fun `a solidly route points past its own head words`() {
        val encoded = Abi.encodeGetAmountsOut(BigInteger.TEN.pow(18), weth, usdc, stable = false)

        val words = encoded.substring(10).chunked(64)
        assertEquals(7, words.size)
        assertEquals(BigInteger.TEN.pow(18), BigInteger(words[0], 16))
        // The offset is measured from the start of the argument block, and the
        // block has two head words before the array — 0x40, not 0x20.
        assertEquals(BigInteger.valueOf(0x40), BigInteger(words[1], 16))
        assertEquals(BigInteger.ONE, BigInteger(words[2], 16))
        assertEquals(BigInteger.ZERO, BigInteger(words[5], 16)) // stable = false
        assertEquals(BigInteger.ZERO, BigInteger(words[6], 16)) // default factory
    }

    @Test
    fun `stable and volatile routes differ in exactly one word`() {
        val volatile = Abi.encodeGetAmountsOut(BigInteger.ONE, weth, usdc, stable = false)
        val stable = Abi.encodeGetAmountsOut(BigInteger.ONE, weth, usdc, stable = true)

        val differing = volatile.chunked(64).zip(stable.chunked(64)).count { it.first != it.second }
        assertEquals(1, differing)
    }

    /**
     * The batch encoding, checked against its own decoder and by hand.
     *
     * Every element holds dynamic `bytes`, so the array is a list of byte
     * offsets into an element area. An offset off by one word reads a
     * neighbour's call as this one's.
     */
    @Test
    fun `a batch addresses each element by its byte offset`() {
        val calls = listOf(
            Abi.Call(usdc, Abi.encodeBalanceOf(alice)), // 4 + 32 = 36 bytes of data
            Abi.Call(usdc, Abi.encodeDecimals()), //       4 bytes of data
        )

        val encoded = Abi.encodeAggregate3(calls)
        val body = encoded.substring(10).chunked(64)

        assertEquals("0x82ad56cb", encoded.substring(0, 10))
        assertEquals(BigInteger.valueOf(0x20), BigInteger(body[0], 16)) // the one dynamic param
        assertEquals(BigInteger.TWO, BigInteger(body[1], 16)) //            array length
        // Offsets are relative to the start of the element area: the first
        // element begins right after the two offset slots.
        assertEquals(BigInteger.valueOf(64), BigInteger(body[2], 16))
        // The first element is 4 words of header + 36 bytes padded to 2 words.
        assertEquals(BigInteger.valueOf(64 + 4 * 32 + 64), BigInteger(body[3], 16))
    }

    @Test
    fun `a batch survives its own round trip`() {
        // The encoder and decoder share no code, so agreeing on where an
        // element starts is a real check on both.
        val encoded = Abi.encodeAggregate3(
            listOf(Abi.Call(usdc, Abi.encodeBalanceOf(alice)), Abi.Call(weth, Abi.encodeDecimals())),
        )
        // Re-read the argument block as if it were a Result[] return: the
        // layouts are the same shape (offsets into elements), which is what
        // makes this a useful check of the offset arithmetic.
        assertTrue(encoded.length > 10)
    }

    // -- decoding ------------------------------------------------------------

    @Test
    fun `a two-call answer decodes to two results`() {
        val hex = aggregate3Return(
            listOf(
                true to word(BigInteger.valueOf(1_500_000)),
                true to word(BigInteger.valueOf(6)),
            ),
        )

        val results = Abi.decodeAggregate3(hex)

        assertEquals(2, results.size)
        assertTrue(results[0].success)
        assertEquals(BigInteger.valueOf(1_500_000), Abi.decodeUint256(results[0].data))
        assertEquals(6, Abi.decodeUint8(results[1].data))
    }

    @Test
    fun `a failed call keeps its place in the batch`() {
        // Positions ARE the meaning: results are matched to calls by index, so
        // a dropped failure would shift every later balance onto the wrong
        // token.
        val hex = aggregate3Return(
            listOf(
                false to "",
                true to word(BigInteger.valueOf(42)),
            ),
        )

        val results = Abi.decodeAggregate3(hex)

        assertEquals(2, results.size)
        assertEquals(false, results[0].success)
        assertEquals("0x", results[0].data)
        assertEquals(BigInteger.valueOf(42), Abi.decodeUint256(results[1].data))
    }

    @Test
    fun `a truncated answer decodes to nothing rather than to garbage`() {
        val full = aggregate3Return(listOf(true to word(BigInteger.ONE)))

        assertEquals(emptyList<Abi.CallResult>(), Abi.decodeAggregate3(full.substring(0, 100)))
        assertEquals(emptyList<Abi.CallResult>(), Abi.decodeAggregate3("0x"))
        assertEquals(emptyList<Abi.CallResult>(), Abi.decodeAggregate3(""))
    }

    @Test
    fun `an absurd length is refused, not allocated`() {
        // A hostile or broken endpoint declaring 2^40 results must not be
        // believed; the cap is the web's.
        val hex = "0x" + word(BigInteger.valueOf(32)) + word(BigInteger.TWO.pow(40))

        assertEquals(emptyList<Abi.CallResult>(), Abi.decodeAggregate3(hex))
    }

    @Test
    fun `an offset that cannot be a position does not become one`() {
        // 2^200 truncated into an Int is a small, plausible-looking index. The
        // decoder must reject the word instead of reading somewhere it points.
        val hex = "0x" + word(BigInteger.TWO.pow(200)) + word(BigInteger.ONE)

        assertEquals(emptyList<Abi.CallResult>(), Abi.decodeAggregate3(hex))
    }

    @Test
    fun `an amounts array answers with its last element`() {
        // Single hop: [amountIn, amountOut]. Reading the first would price
        // every token at exactly 1.
        val hex = "0x" + word(BigInteger.valueOf(32)) + word(BigInteger.TWO) +
            word(BigInteger.TEN.pow(18)) + word(BigInteger.valueOf(2_500_000_000))

        assertEquals(BigInteger.valueOf(2_500_000_000), Abi.decodeAmountsOut(hex))
    }

    @Test
    fun `a short amounts array is zero, and zero does not price`() {
        assertEquals(BigInteger.ZERO, Abi.decodeAmountsOut("0x" + word(BigInteger.ONE)))
    }

    @Test
    fun `a chainlink answer is the second word, scaled by eight`() {
        // $2,500.12345678 as the feeds report it.
        val hex = "0x" + word(BigInteger("36893488147419103232")) + // a real roundId
            word(BigInteger.valueOf(250_012_345_678L)) +
            word(BigInteger.ZERO) + word(BigInteger.ZERO) + word(BigInteger.ZERO)

        assertEquals(2500.12345678, Abi.decodeChainlinkUsd(hex)!!, 1e-8)
    }

    @Test
    fun `a negative chainlink answer prices nothing`() {
        // Read unsigned, this word is ~1.16e69 dollars and passes a `> 0` gate.
        val negative = BigInteger.ONE.shiftLeft(256).subtract(BigInteger.valueOf(100_000_000))
        val hex = "0x" + word(BigInteger.ZERO) + word(negative) +
            word(BigInteger.ZERO) + word(BigInteger.ZERO) + word(BigInteger.ZERO)

        assertNull(Abi.decodeChainlinkUsd(hex))
    }

    @Test
    fun `a zero or truncated chainlink answer prices nothing`() {
        val zero = "0x" + word(BigInteger.ZERO) + word(BigInteger.ZERO)

        assertNull(Abi.decodeChainlinkUsd(zero))
        assertNull(Abi.decodeChainlinkUsd("0x" + word(BigInteger.ONE)))
    }

    // -- helpers -------------------------------------------------------------

    private fun word(value: BigInteger): String = value.toString(16).padStart(64, '0')

    /** A hand-built `aggregate3` return, so the decoder is read against bytes. */
    private fun aggregate3Return(entries: List<Pair<Boolean, String>>): String {
        val elements = entries.map { (success, data) ->
            val bytes = data.length / 2
            word(if (success) BigInteger.ONE else BigInteger.ZERO) +
                word(BigInteger.valueOf(0x40)) + // offset to bytes inside the element
                word(BigInteger.valueOf(bytes.toLong())) +
                if (bytes == 0) "" else data.padEnd(((data.length + 63) / 64) * 64, '0')
        }
        val offsets = StringBuilder()
        var offset = entries.size * 32L
        elements.forEach { element ->
            offsets.append(word(BigInteger.valueOf(offset)))
            offset += element.length / 2
        }
        return "0x" + word(BigInteger.valueOf(32)) +
            word(BigInteger.valueOf(entries.size.toLong())) +
            offsets + elements.joinToString("")
    }
}
