package app.getvela.wallet

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.wrappedNativeIsTheNative

/**
 * Which chains have a "wrapped" native that is the native itself is the core's
 * list, not the shell's (spec 038, the founder's Celo report: CELO 6.96 and
 * WCELO 6.96 for one holding). The balance walk asks before it adds the
 * wrapped slot; this pins that the answer crosses the bridge intact, including
 * its case-insensitivity — chain data spells the GoldToken in checksum case and
 * the core's table in lower.
 */
class WrappedNativeTest {

    @Test
    fun celoGoldTokenIsTheNative() {
        assertTrue(wrappedNativeIsTheNative(42220u, "0x471EcE3750Da237f93B8E339c536989b8978a438"))
        assertTrue(wrappedNativeIsTheNative(42220u, "0x471ece3750da237f93b8e339c536989b8978a438"))
        assertTrue(wrappedNativeIsTheNative(44787u, "0xF194afDf50B03e69Bd7D057c1Aa9e10c9954E4C9"))
    }

    @Test
    fun aRealWrapperIsNotTheNative() {
        // WETH on mainnet and WBNB on BSC wrap a coin; their balances are
        // holdings of their own and belong in the list.
        assertFalse(wrappedNativeIsTheNative(1u, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
        assertFalse(wrappedNativeIsTheNative(56u, "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"))
        assertFalse(wrappedNativeIsTheNative(42220u, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"))
    }
}
