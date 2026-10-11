package app.getvela.wallet

import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.NetworkEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcSource
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The pool's seeds for a chain: its own endpoint, then the CORE's curated
 * public ones (`publicRpcUrls`, the one list every shell reads). Android had
 * no public tier at all — one candidate per chain — so a default that would
 * not answer left the chain dark with nothing to fall back on.
 *
 * The list is the real core's (the host library), not a copy.
 */
class NetworkEndpointSourceTest {
    private fun row(chain: Long, rpc: String, custom: Boolean = false) =
        NetNetworkRow(id = chain.toString(), chain_id = chain, display_name = "Chain $chain", native_symbol = "ETH", is_custom = custom, rpc_url = rpc)

    private fun seeds(vararg rows: NetNetworkRow, chain: Int) = runBlocking {
        NetworkEndpointSource(networks = { NetView(loaded = true, networks = rows.toList()) }).forChain(chain).rpc
    }

    @Test
    fun `a built-in chain's default is followed by the core's public tier, in the core's order`() {
        val polygon = seeds(row(137, "https://polygon-rpc.com"), chain = 137)
        assertEquals(
            listOf(
                RpcEndpointSeed("https://polygon-rpc.com", RpcSource.Default),
                RpcEndpointSeed("https://polygon-bor-rpc.publicnode.com", RpcSource.Public),
                RpcEndpointSeed("https://polygon.gateway.tenderly.co", RpcSource.Public),
            ),
            polygon,
        )
        // The endpoint issue 483 found dead on every call is gone with the shell's having no list of its own.
        assertFalse(polygon.any { it.url.contains("1rpc.io") })
    }

    @Test
    fun `an endpoint is never listed twice`() {
        // A default that IS one of the public ones (trailing slash or not) stays the default, once.
        val gnosis = seeds(row(100, "https://gnosis-rpc.publicnode.com/"), chain = 100)
        assertEquals(listOf(RpcSource.Default, RpcSource.Public), gnosis.map { it.source })
        assertEquals(1, gnosis.count { it.url.trimEnd('/') == "https://gnosis-rpc.publicnode.com" })
    }

    @Test
    fun `a custom network has only what the person typed`() {
        val custom = seeds(row(48_900, "https://my.rpc", custom = true), chain = 48_900)
        assertEquals(listOf(RpcEndpointSeed("https://my.rpc", RpcSource.User)), custom)
    }

    @Test
    fun `every chain the core lists gains at least one fallback that is https`() {
        for (chain in listOf(1, 56, 137, 42161, 10, 8453, 43114, 100, 196, 42220, 57073)) {
            val public = seeds(row(chain.toLong(), "https://default.example"), chain = chain).filter { it.source == RpcSource.Public }
            assertTrue("chain $chain has no public endpoint", public.isNotEmpty())
            assertTrue("chain $chain: $public", public.all { it.url.startsWith("https://") })
        }
    }
}
