package app.getvela.wallet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.NativeQuoteGroup
import uniffi.vela_core_uniffi.bestNativeDexPrice
import uniffi.vela_core_uniffi.chooseNativePrice

/**
 * What a coin is worth is the core's answer, and now Kotlin can ask it.
 *
 * These two rules were reachable from the web through wasm and from nowhere
 * else, so Android and iOS were each one convenient afternoon away from writing
 * their own price ladder — and two Vela wallets would have disagreed about the
 * value of the same holding. Spec 041 promotes them through uniffi instead
 * (the desktop handover asked for exactly this rather than a third copy).
 *
 * The shell still owns the multicall and the decoding. What crosses the bridge
 * is the judgement: which quote is deepest, and which source to believe.
 */
class NativePriceTest {

    @Test
    fun theDeepestPoolWins() {
        // For a fixed input, a more liquid pool returns more output. Two quotes
        // in the same stable: the larger one is the better price.
        val price = bestNativeDexPrice(
            listOf(
                NativeQuoteGroup(
                    amountsOut = listOf("1000000000", "2500000000"),
                    quoteDecimals = 6u,
                ),
            ),
        )
        assertNotNull(price)
        assertEquals(2500.0, price!!, 0.01)
    }

    @Test
    fun noQuotesIsNoPriceRatherThanZero() {
        // The distinction the whole read path depends on: a coin nobody could
        // price is UNKNOWN, and a screen that renders unknown as `0` has told a
        // person their holding is worthless.
        assertNull(bestNativeDexPrice(emptyList()))
        assertNull(
            bestNativeDexPrice(
                listOf(NativeQuoteGroup(amountsOut = emptyList(), quoteDecimals = 6u)),
            ),
        )
    }

    @Test
    fun theDexPriceWinsWhenItAgrees() {
        val chosen = chooseNativePrice(dex = 2000.0, chainlinkLocal = 2010.0, chainlinkEth = null)
        assertEquals(2000.0, chosen.price!!, 0.01)
        assertEquals("dex", chosen.source)
    }

    @Test
    fun aDexPriceThatDisagreesTooMuchLosesToChainlink() {
        // The sanity band. A DEX quote from a thin or manipulated pool is
        // exactly how a wallet ends up showing somebody a fortune or nothing;
        // the band is why it does not. Where the band sits is the core's
        // business — this asserts only that it HAS one and that Kotlin gets its
        // verdict rather than the raw quote.
        val chosen = chooseNativePrice(dex = 2.0, chainlinkLocal = 2000.0, chainlinkEth = null)
        assertEquals(2000.0, chosen.price!!, 0.01)
        assertEquals("chainlink_sanity", chosen.source)
    }

    @Test
    fun chainlinkIsTheFallbackWhenThereIsNoDexQuote() {
        val local = chooseNativePrice(dex = null, chainlinkLocal = 1800.0, chainlinkEth = 1799.0)
        assertEquals("chainlink_local", local.source)

        val eth = chooseNativePrice(dex = null, chainlinkLocal = null, chainlinkEth = 1799.0)
        assertEquals("chainlink_eth", eth.source)
    }

    @Test
    fun nothingPriceableIsSaidPlainly() {
        val chosen = chooseNativePrice(dex = null, chainlinkLocal = null, chainlinkEth = null)
        assertNull(chosen.price)
        assertEquals("none", chosen.source)
    }
}
