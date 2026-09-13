package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.wallet.core.ChainDex
import app.getvela.wallet.feature.wallet.core.ChainInfo
import app.getvela.wallet.feature.wallet.core.ChainNative
import app.getvela.wallet.feature.wallet.core.ChainStable
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.IncomingScan
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.TrustExecutor
import app.getvela.wallet.feature.wallet.core.TrustOperation
import app.getvela.wallet.feature.wallet.core.TrustShellResult
import app.getvela.wallet.feature.wallet.core.TrustView
import java.math.BigInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.TokenTrustCore

/**
 * Money arriving, end to end on the real `token_trust`: a Transfer log on a
 * chain becomes a row in this device's history.
 *
 * This is the path a scam token attacks, so the cases that matter most are the
 * ones where nothing should be written — an unknown contract, an unresolvable
 * symbol, the same receipt seen twice.
 */
class IncomingScanTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private val wallet = "0x1111111111111111111111111111111111111111"
    private val usdc = "0x2791bca1f2de4661ed88a30c99a7a9449aa84174"
    private val stranger = "0xbadbadbadbadbadbadbadbadbadbadbadbadbad0"

    /**
     * A stablecoin in this chain's registry that the core has NO pinned
     * metadata for — so its symbol and decimals really do have to be read from
     * the chain, which is what makes the "unresolvable metadata" case testable.
     */
    private val unknownStable = "0xdeadbeef0000000000000000000000000000cafe"
    private val sender = "0x9f3c000000000000000000000000000000021ae0"

    private class PerChainEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    /** `topics[2]` for a recipient: the address, left-padded to 32 bytes. */
    private fun topic(address: String) =
        "0x" + address.removePrefix("0x").lowercase().padStart(64, '0')

    private fun transferLog(
        token: String,
        from: String = sender,
        to: String = wallet,
        amount: BigInteger = BigInteger.valueOf(50_000_000), // 50 USDC
        block: Long = 1000,
        logIndex: Int = 2,
    ) = JSONObject()
        .put("address", token)
        .put(
            "topics",
            JSONArray()
                .put("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
                .put(topic(from))
                .put(topic(to)),
        )
        .put("data", "0x" + amount.toString(16).padStart(64, '0'))
        .put("transactionHash", "0xabc123")
        .put("blockNumber", "0x" + block.toString(16))
        .put("logIndex", "0x" + logIndex.toString(16))

    private class Harness(
        val scan: IncomingScan,
        val feed: FeedExecutor,
        val store: FakeStore,
    )

    /**
     * A device with one chain, one stablecoin in its registry, and whatever
     * logs the test hands back.
     */
    private fun harness(
        logs: List<JSONObject>,
        symbol: String? = "USDC",
        decimals: Int? = 6,
    ): Harness {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val store = FakeStore()

        val transport = FakeRpcTransport { _, method, params ->
            when (method) {
                "eth_blockNumber" -> FakeRpcTransport.body("0x3e8") // block 1000
                // A real endpoint honours the `address` filter it was handed.
                // The core builds that filter from its own allowlist, and it
                // does NOT re-check the contract when the logs come back — so a
                // fake that returned everything would be modelling a hostile
                // endpoint, not a working one, and would pass tests that should
                // fail.
                "eth_getLogs" -> {
                    val filter = params.firstOrNull() as? JSONObject
                    val allowed = filter?.optJSONArray("address")?.let { array ->
                        (0 until array.length()).map { array.optString(it).lowercase() }
                    }
                    val visible = logs.filter { log ->
                        allowed == null || log.optString("address").lowercase() in allowed
                    }
                    FakeRpcTransport.body(JSONArray().also { visible.forEach(it::put) })
                }
                "eth_getBlockByNumber" ->
                    FakeRpcTransport.body(JSONObject().put("timestamp", "0x68bd0000"))
                "eth_call" -> metadataAnswer(symbol, decimals)
                else -> FakeRpcTransport.body("0x")
            }
        }

        val pool = RpcPool(
            store = store,
            endpoints = PerChainEndpoints(),
            scope = scope,
            transport = transport,
        )
        runBlocking { pool.start() }

        val trustExecutor = TrustExecutor(pool, store)
        val trustHost = CoreHost(
            bridge = TokenTrustCore().asBridge(),
            scope = scope,
            initial = TrustView(),
            serializer = TrustView.serializer(),
            perform = JsonShell.perform(
                TrustOperation.serializer(),
                TrustShellResult.serializer(),
                trustExecutor::perform,
            ),
            escapedFailure = JsonShell.escapedFailure(
                TrustOperation.serializer(),
                TrustShellResult.serializer(),
                fallback = TrustShellResult.CacheInvalidated,
                answer = trustExecutor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )

        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        val scan = IncomingScan(
            trust = trustHost,
            feed = feed,
            chainInfo = { chainId ->
                ChainInfo(
                    chainId = chainId,
                    native = ChainNative("Polygon", "POL", 18),
                    stables = listOf(
                        ChainStable("USDC", "native", usdc),
                        ChainStable("NEWC", "bridge", unknownStable),
                    ),
                    wrappedNative = "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270",
                    dex = ChainDex("uniswap-v3", "0x61ffe014ba17989e743c5f6cb21bf9697530b21e", null),
                )
            },
            heldChains = { listOf(137) },
            heldTokens = { emptyList() },
            nativeSymbol = { "POL" },
        )
        return Harness(scan, feed, store)
    }

    /** A Multicall3 answer carrying `symbol()` then `decimals()`. */
    private fun metadataAnswer(symbol: String?, decimals: Int?): RpcPostResult {
        fun word(value: BigInteger) = value.toString(16).padStart(64, '0')
        fun stringReturn(text: String): String {
            val bytes = text.toByteArray(Charsets.UTF_8).joinToString("") { "%02x".format(it) }
            return word(BigInteger.valueOf(32)) +
                word(BigInteger.valueOf((text.toByteArray(Charsets.UTF_8).size).toLong())) +
                bytes.padEnd(64, '0')
        }

        val entries = listOf(
            symbol?.let { stringReturn(it) },
            decimals?.let { word(BigInteger.valueOf(it.toLong())) },
        )
        val elements = entries.map { data ->
            val payload = data ?: ""
            word(if (data == null) BigInteger.ZERO else BigInteger.ONE) +
                word(BigInteger.valueOf(0x40)) +
                word(BigInteger.valueOf((payload.length / 2).toLong())) +
                payload
        }
        val offsets = StringBuilder()
        var offset = entries.size * 32L
        elements.forEach {
            offsets.append(word(BigInteger.valueOf(offset)))
            offset += it.length / 2
        }
        val hex = "0x" + word(BigInteger.valueOf(32)) +
            word(BigInteger.valueOf(entries.size.toLong())) +
            offsets + elements.joinToString("")
        return FakeRpcTransport.body(hex)
    }

    private suspend fun storedRecords(harness: Harness) =
        (harness.feed.perform(FeedOperation.ReadTxStore(wallet, 1)) as FeedShellResult.StoreLoaded)
            .records

    // -- what should land ----------------------------------------------------

    @Test
    fun `a transfer of a registry stablecoin becomes a stored receipt`() = runBlocking {
        val harness = harness(listOf(transferLog(usdc)))

        assertEquals(1, harness.scan.runOnce(wallet))

        val record = storedRecords(harness).single()
        assertEquals("50", record.value)
        // "USDC.e", not "USDC": this contract is in the core's own pinned
        // metadata table, and the core's fact beats anything an endpoint says
        // its `symbol()` returns. That is the point of pinning it — an endpoint
        // cannot rename a person's stablecoin.
        assertEquals("USDC.e", record.symbol)
        assertEquals(6, record.decimals)
        assertEquals(137, record.chain_id)
        assertEquals(wallet, record.to)
        // The id is the core's `{chain}-{tx}-{logIndex}`, which is what makes a
        // second sighting of the same receipt a no-op.
        assertTrue(record.id.startsWith("137-0xabc123-"))
    }

    @Test
    fun `the same receipt seen twice is stored once`() = runBlocking {
        // The Activity surface polls every ten seconds and the scan windows
        // overlap on purpose. Without the de-dupe, a person would watch the
        // same payment pile up in their history — and be buzzed for each one.
        val harness = harness(listOf(transferLog(usdc)))

        assertEquals(1, harness.scan.runOnce(wallet))
        assertEquals(0, harness.scan.runOnce(wallet))

        assertEquals(1, storedRecords(harness).size)
    }

    // -- what should not ------------------------------------------------------

    @Test
    fun `a transfer of an unknown contract is not stored`() = runBlocking {
        // The whole attack: anybody can emit a Transfer log from any contract
        // to any address. The allowlist is the core's, built from the registry
        // and what this person already holds — an airdropped token is not on it.
        val harness = harness(listOf(transferLog(stranger)))

        assertEquals(0, harness.scan.runOnce(wallet))
        assertEquals(emptyList<Any>(), storedRecords(harness))
    }

    @Test
    fun `a transfer to somebody else is not stored`() = runBlocking {
        val elsewhere = "0x2222222222222222222222222222222222222222"
        val harness = harness(listOf(transferLog(usdc, to = elsewhere)))

        assertEquals(0, harness.scan.runOnce(wallet))
        assertEquals(emptyList<Any>(), storedRecords(harness))
    }

    @Test
    fun `a token whose symbol never resolved is not stored`() = runBlocking {
        // Writing it with a guessed symbol and 18 decimals would put a
        // permanent, wrong line in somebody's history for a receipt that might
        // have been thousands. It stays out and the next poll retries.
        //
        // The contract here is one the core has no pinned metadata for, so the
        // chain read is genuinely the only source — which is the situation a
        // brand-new token is always in.
        val harness = harness(
            listOf(transferLog(unknownStable)),
            symbol = null,
            decimals = null,
        )

        assertEquals(0, harness.scan.runOnce(wallet))
        assertEquals(emptyList<Any>(), storedRecords(harness))
    }

    @Test
    fun `a token the core has no metadata for is stored once the chain answers`() = runBlocking {
        // The other half of the case above: the read succeeds, so the receipt
        // lands with the symbol the contract itself reported.
        val harness = harness(listOf(transferLog(unknownStable)), symbol = "NEWC", decimals = 2)

        assertEquals(1, harness.scan.runOnce(wallet))

        val record = storedRecords(harness).single()
        assertEquals("NEWC", record.symbol)
        assertEquals(2, record.decimals)
        // 50,000,000 at 2 decimals is 500,000 — not 50. Reading the decimals
        // from the wrong token is a receipt wrong by four orders of magnitude.
        assertEquals("500000", record.value)
    }

    @Test
    fun `a chain that answers nothing writes nothing and does not throw`() = runBlocking {
        val harness = harness(emptyList())

        assertEquals(0, harness.scan.runOnce(wallet))
        assertEquals(emptyList<Any>(), storedRecords(harness))
    }

    @Test
    fun `an empty address never scans`() = runBlocking {
        val harness = harness(listOf(transferLog(usdc)))

        assertEquals(0, harness.scan.runOnce(""))
        assertEquals(null, harness.store.read(KeyValueStore.Keys.TRANSACTIONS))
    }
}
