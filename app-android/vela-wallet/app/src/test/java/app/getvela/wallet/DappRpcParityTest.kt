package app.getvela.wallet

import app.getvela.wallet.feature.browser.core.DappRpc
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Spec 044 T013: the routing table is the extension's `protocol.js`, and
 * this proves it by reading that file from the repo (the same file the APK
 * carries as the page's provider) rather than trusting a hand-kept copy.
 */
class DappRpcParityTest {
    private val js: String = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        File(root, "app-web/vela-wallet/extension/lib/protocol.js").readText()
    }

    /** Every single-quoted string inside `export const NAME = …` up to its closing bracket. */
    private fun block(name: String): Set<String> {
        val start = js.indexOf("export const $name")
        check(start >= 0) { "$name is gone from protocol.js" }
        val body = js.substring(start)
        val end = listOf(body.indexOf("]);"), body.indexOf("];")).filter { it >= 0 }.minOrNull() ?: error("$name has no end")
        return Regex("'([^']+)'").findAll(body.substring(0, end)).map { it.groupValues[1] }.toSet()
    }

    @Test
    fun `the read allowlist matches the javascript`() {
        val mine = (DappRpc.READ_ONLY_RPC_METHODS + DappRpc.BUNDLER_METHODS + DappRpc.EXTRA_READ_METHODS).toSet()
        val theirs = block("READ_ONLY_RPC_METHODS") + block("BUNDLER_METHODS") + block("READ_PROXY_METHODS")
        assertEquals(theirs.sorted(), mine.sorted())
        assertEquals(block("BUNDLER_METHODS").sorted(), DappRpc.BUNDLER_METHODS.sorted())
        assertEquals(block("READ_ONLY_RPC_METHODS").sorted(), DappRpc.READ_ONLY_RPC_METHODS.sorted())
    }

    @Test
    fun `routes fail closed`() {
        assertEquals(DappRpc.Route.Sign, DappRpc.route("eth_sendTransaction"))
        assertEquals(DappRpc.Route.Sign, DappRpc.route("personal_sign"))
        assertEquals(DappRpc.Route.Sign, DappRpc.route("eth_signTypedData_v4"))
        assertEquals(DappRpc.Route.Unsupported, DappRpc.route("eth_sign"))
        assertEquals(DappRpc.Route.Unsupported, DappRpc.route("eth_signTransaction"))
        assertEquals(DappRpc.Route.State, DappRpc.route("eth_chainId"))
        assertEquals(DappRpc.Route.Switch, DappRpc.route("wallet_switchEthereumChain"))
        assertEquals(DappRpc.Route.Ack, DappRpc.route("wallet_watchAsset"))
        assertEquals(DappRpc.Route.Read(bundler = true), DappRpc.route("eth_estimateUserOperationGas"))
        assertEquals(DappRpc.Route.Read(bundler = false), DappRpc.route("eth_blockNumber"))
        assertEquals(DappRpc.Route.Unsupported, DappRpc.route("eth_accounts_but_not_really"))
    }

    @Test
    fun `the switch parameter reads hex and decimal`() {
        assertEquals(100, DappRpc.switchChainParam("""[{"chainId":"0x64"}]"""))
        assertEquals(100, DappRpc.switchChainParam("""[{"chainId":"100"}]"""))
        assertNull(DappRpc.switchChainParam("""[]"""))
        assertNull(DappRpc.switchChainParam("""not json"""))
        assertEquals("0x64", DappRpc.hexChainId(100))
    }
}
