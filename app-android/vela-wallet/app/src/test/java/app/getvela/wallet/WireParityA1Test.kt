package app.getvela.wallet

import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.TrackOperation
import app.getvela.wallet.feature.send.core.TrackRecordStatus
import app.getvela.wallet.feature.send.core.TrackShellResult
import app.getvela.wallet.feature.send.core.TrackerExecutor
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.signing.core.ClearExecutor
import app.getvela.wallet.feature.signing.core.ClearOperation
import app.getvela.wallet.feature.signing.core.ClearProbe
import app.getvela.wallet.feature.signing.core.ClearShellResult
import app.getvela.wallet.feature.signing.core.ClearSigningEvent
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.NetworkEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcOperation
import app.getvela.wallet.feature.wallet.core.RpcPayload
import app.getvela.wallet.feature.wallet.core.RpcPoolCallRegistry
import app.getvela.wallet.feature.wallet.core.RpcPoolExecutor
import app.getvela.wallet.feature.wallet.core.RpcShellResult
import app.getvela.wallet.feature.wallet.core.RpcCallVerdict
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Native parity A1: the operations Android could not answer, answered — each
 * as the web and desktop do, in the core's vocabulary.
 */
class WireParityA1Test {

    // -- clear signing: rpc_get_code (cs-13) --------------------------------

    @Test
    fun `the target's code is answered as a code probe, raw`() = runBlocking<Unit> {
        val executor = ClearExecutor(
            dataBase = { "" },
            ethCall = { _, _, _ -> null to false },
            getCode = { chainId, address -> if (chainId == 100 && address == "0xabc") "0x6080" else null },
        )
        assertEquals(
            ClearShellResult.RpcAnswer(ClearProbe.Code, 100, "0xabc", "0x6080", rpc_error = false),
            executor.perform(ClearOperation.RpcGetCode(100, "0xabc")),
        )
        // No answer is "could not ask", never "no code".
        assertEquals(
            ClearShellResult.RpcAnswer(ClearProbe.Code, 1, "0xdef", null, rpc_error = false),
            executor.perform(ClearOperation.RpcGetCode(1, "0xdef")),
        )
        assertEquals(
            ClearShellResult.RpcAnswer(ClearProbe.Code, 1, "0xdef", null, false),
            executor.neutralAnswer(ClearOperation.RpcGetCode(1, "0xdef")),
        )
    }

    @Test
    fun `a transaction's reading asks for the code and names the signing account`() {
        val tx = SigningController.clearKickoff(
            "eth_sendTransaction",
            """[{"to":"0x1111111111111111111111111111111111111111","data":"0x","value":"0x0"}]""",
            100,
            "https://app.example",
            from = "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ) as ClearSigningEvent.ResolveTransaction
        assertTrue(tx.probe_code)
        assertEquals("0x88cCA0EeDbF2C4426110bbFc998F048689266894", tx.from)
        val batch = SigningController.clearKickoff("wallet_sendCalls", """[{"calls":[]}]""", 100, null, from = "") as ClearSigningEvent.ResolveBatch
        assertTrue(batch.probe_code)
        assertNull("a blank account is no account", batch.from)
    }

    // -- tx_tracker: read_nonce (round 3, send-5) ----------------------------

    private fun tracker(port: FakeRelayPort) = TrackerExecutor(
        relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        feed = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() }),
        now = { 1_000.0 },
        ports = object : TrackerExecutor.TrackerPorts {
            override fun recordsPatched(ids: List<String>, status: TrackRecordStatus, txHash: String?) = Unit
            override fun notifyConfirmed(userOpHash: String, chainId: Int, txHash: String) = Unit
            override fun receiptLogsConfirmed(from: String, chainId: Int, logs: List<TrustReceiptLog>) = Unit
        },
    )

    private val readNonce = TrackOperation.ReadNonce(
        chain_id = 100,
        entry_point = "0x0000000071727De22E5E9d8BAf0edAc6f37da032",
        sender = "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        user_op_hash = "0xop",
    )

    @Test
    fun `the nonce is read at the head it was read at`() = runBlocking<Unit> {
        val port = FakeRelayPort()
        port.answer("eth_blockNumber", FakeRelayPort.body("0x10"))
        port.answer("eth_call", FakeRelayPort.body("0x05"))
        assertEquals(
            TrackShellResult.NonceRead("0xop", 1_000.0, nonce = "0x05", head_block = 16),
            tracker(port).perform(readNonce),
        )
        assertTrue("the call is asked at the head, not at latest", port.calls.any { it.contains("eth_call") })
    }

    @Test
    fun `no head or no answer is nothing known`() = runBlocking<Unit> {
        val noHead = FakeRelayPort()
        assertEquals(TrackShellResult.NonceRead("0xop", 1_000.0), tracker(noHead).perform(readNonce))
        val refused = FakeRelayPort()
        refused.answer("eth_blockNumber", FakeRelayPort.body("0x10"))
        refused.answer("eth_call", FakeRelayPort.error("execution reverted"))
        assertEquals(TrackShellResult.NonceRead("0xop", 1_000.0), tracker(refused).perform(readNonce))
        assertEquals(TrackShellResult.NonceRead("0xop", 1_000.0), tracker(refused).neutralAnswer(readNonce))
    }

    // -- rpc_pool: pool_config.own_only ("Only use my node") ------------------

    private val registry = object : RpcPoolCallRegistry {
        override fun payload(callId: String): RpcPayload? = null
        override fun keepBody(callId: String, url: String, body: JSONObject) = Unit
        override fun settle(callId: String, verdict: RpcCallVerdict) = Unit
    }

    private fun row(chain: Long, rpc: String, ownRpc: Boolean, onlyMine: Boolean) = NetNetworkRow(
        id = chain.toString(), chain_id = chain, display_name = "Chain $chain", native_symbol = "ETH",
        rpc_url = rpc, own_rpc = ownRpc, only_mine = onlyMine,
    )

    @Test
    fun `the pool is told the person's only-my-node, and their own RPC is theirs`() = runBlocking<Unit> {
        val endpoints = NetworkEndpointSource(
            networks = { NetView(loaded = true, networks = listOf(row(100, "http://127.0.0.1:8945", ownRpc = true, onlyMine = true), row(1, "https://eth.example", ownRpc = false, onlyMine = false))) },
            publicRpcs = { emptyList() },
        )
        val executor = RpcPoolExecutor(FakeStore(), registry, endpoints, now = { 5.0 })
        val mine = executor.perform(RpcOperation.LoadPoolConfig(100)) as RpcShellResult.PoolConfig
        assertTrue(mine.own_only)
        assertEquals(RpcSource.User, mine.rpc_endpoints.single().source)
        val shipped = executor.perform(RpcOperation.LoadPoolConfig(1)) as RpcShellResult.PoolConfig
        assertFalse(shipped.own_only)
        assertEquals(RpcSource.Default, shipped.rpc_endpoints.single().source)
    }
}
