package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import app.getvela.wallet.feature.wallet.core.BalanceExecutor
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.ChainDex
import app.getvela.wallet.feature.wallet.core.ChainInfo
import app.getvela.wallet.feature.wallet.core.ChainNative
import app.getvela.wallet.feature.wallet.core.ChainStable
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * The wallet home's balances, end to end on the real `balance_dashboard`.
 *
 * Two of these cases exist because the desktop sibling shipped them as latent
 * bugs (spec 031) that were **invisible while every price was `None`** — and
 * this spec is the one that makes prices exist. They are inherited as tests
 * rather than as a warning in a document nobody re-reads.
 */
class BalanceMachineTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    /** One endpoint per chain, so a test can tell which chain was asked. */
    private class PerChainEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun row(chainId: Long, symbol: String, name: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
        rpc_url = "https://chain-$chainId.example",
    )

    private class Harness(
        val host: CoreHost<BalanceView>,
        val transport: FakeRpcTransport,
    )

    private fun harness(
        rows: List<NetNetworkRow>,
        chains: Map<Int, ChainInfo> = emptyMap(),
        mainnet: Map<String, Double> = emptyMap(),
        /** The device's own storage — the custom-token cases write into it. */
        store: FakeStore = FakeStore(),
        answer: (url: String, method: String) -> app.getvela.wallet.feature.wallet.core.RpcPostResult,
    ): Harness {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val transport = FakeRpcTransport(answer)
        val networks = MutableStateFlow(NetView(loaded = true, networks = rows))
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = PerChainEndpoints(),
            scope = scope,
            transport = transport,
        )
        val executor = BalanceExecutor(
            pool = pool,
            networks = networks,
            store = store,
            chainInfo = { chainId -> chains[chainId] },
            mainnetPrices = { mainnet },
        )
        val host = CoreHost(
            bridge = BalanceDashboardCore().asBridge(),
            scope = scope,
            initial = BalanceView(),
            serializer = BalanceView.serializer(),
            perform = JsonShell.perform(
                BalanceOperation.serializer(),
                BalanceShellResult.serializer(),
                executor::perform,
            ),
            escapedFailure = JsonShell.escapedFailure(
                BalanceOperation.serializer(),
                BalanceShellResult.serializer(),
                fallback = BalanceShellResult.PrivacyWritten,
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
        executor.stream = { event -> host.dispatch(event, BalanceEvent.serializer()) }
        runBlocking { pool.start() }
        return Harness(host, transport)
    }

    private fun CoreHost<BalanceView>.settle(predicate: (BalanceView) -> Boolean): BalanceView =
        runBlocking { withTimeout(TIMEOUT) { view.first(predicate) } }

    @Test
    fun aNativeBalanceCrossesAsAHumanDecimal() {
        // **The 10^18 trap.** The core parses this string straight into a float
        // and multiplies it by a price, so raw units here would produce a total
        // a quintillion times too large — and nothing would show it until a
        // price existed, which is exactly what this spec brings. 1.5 ETH is
        // 0x14d1120d7b160000 wei.
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }
        val token = view.tokens.single()
        assertEquals("1.5", token.balance)
        assertEquals("ETH", token.symbol)
        assertEquals(1, token.chain_id)
        // Native coin: no contract address.
        assertNull(token.token_address)
    }

    @Test
    fun aChainWithNoNativeCoinIsNeverAsked() {
        // **The 4×10^57 trap.** Tempo's RPC answers the same constant for every
        // address and its native symbol is `USD`, so a stablecoin peg would
        // price that constant at a dollar. The guard is the CORE's predicate,
        // not a magnitude threshold invented here — and the proof is that the
        // chain is never queried at all.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(4217, "USD", "Tempo")),
        ) { _, _ -> FakeRpcTransport.body("0x14d1120d7b160000") }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        h.host.settle { it.tokens.isNotEmpty() }
        assertTrue(
            "Ethereum was read",
            h.transport.asked.any { it.contains("chain-1.example") },
        )
        assertFalse(
            "a chain with no native coin must never be asked for a native balance",
            h.transport.asked.any { it.contains("chain-4217.example") },
        )
    }

    @Test
    fun aChainHoldingNothingIsNotAFailedChain() {
        // The bug this caught: the first version treated "produced no token" as
        // "did not answer", so an empty chain arrived as a broken one and the
        // screen would have shown a network-down banner to somebody whose only
        // crime is having no funds there.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis")),
        ) { url, _ ->
            if (url.contains("chain-100")) FakeRpcTransport.body("0x0")
            else FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() && !it.holdings_loading }
        assertTrue(
            "an empty chain answered, so it has not failed",
            view.failed_chain_ids.isEmpty(),
        )
    }

    @Test
    fun aZeroBalanceIsNotAHolding() {
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x0")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        // Settle on the fetch finishing rather than on tokens appearing, since
        // the point is that none do.
        val view = h.host.settle { !it.holdings_loading && it.address != null }
        assertTrue(view.tokens.isEmpty())
    }

    @Test
    fun aChainThatDidNotAnswerIsReportedAsFailedNotAsZero() {
        // The distinction a money screen must not blur: a chain that did not
        // answer is UNKNOWN, and rendering unknown as zero tells a person their
        // holding is gone.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis")),
        ) { url, _ ->
            if (url.contains("chain-100")) FakeRpcTransport.network()
            else FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        // Settle on the FETCH finishing, not on a token appearing: the streamed
        // `chain_assets_arrived` for the healthy chain lands strictly before
        // the settle that carries the failed list, so `tokens.isNotEmpty()`
        // fires while the answer is still half-formed.
        // Only `fetch_settled` carries the failed list, so waiting for it is
        // waiting for the fetch to actually finish. `!holdings_loading` is true
        // before the fetch even starts, which is how the first version of this
        // predicate settled on the empty initial view.
        val view = h.host.settle { it.failed_chain_ids.isNotEmpty() }
        assertEquals(listOf("ETH"), view.tokens.map { it.symbol })
        assertTrue("the silent chain is named", view.failed_chain_ids.contains(100))
    }

    /**
     * Settings' two readings of an unread chain, over the real core: a dead
     * RPC earns the banner and SR3's red retry row; a 429 earns neither — it
     * is a grey "retrying" line, because it heals by itself. The Android
     * banner used to take the pool's raw failed list, rate-limited included.
     */
    @Test
    fun settingsBannersADeadChainAndOnlyGreysARateLimitedOne() {
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis"), row(137, "POL", "Polygon")),
        ) { url, _ ->
            when {
                url.contains("chain-100") -> FakeRpcTransport.network()
                url.contains("chain-137") -> FakeRpcTransport.httpError(429)
                else -> FakeRpcTransport.body("0x14d1120d7b160000")
            }
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())
        val view = h.host.settle { it.failed_chain_ids.containsAll(listOf(100, 137)) && it.rate_limited_chain_ids.contains(137) }

        val strings = app.getvela.wallet.core.i18n.I18nRuntime { tag ->
            java.io.File(System.getProperty("vela.repo.root")!!, "assets/i18n/$tag.json").readBytes()
        }.apply { initialize("en") }
        val names = mapOf(1 to "Ethereum", 100 to "Gnosis", 137 to "Polygon")
        val base = app.getvela.wallet.feature.settings.SettingsFixtures.buildState(
            app.getvela.wallet.feature.settings.SettingsScreenState.SR1,
            strings,
        )
        val banner = app.getvela.wallet.feature.settings.SettingsLive.withBanner(base, view.banner_chain_ids, names, strings).rpcBanner
        assertEquals("only the dead chain earns the banner", listOf("Gnosis"), banner?.chips?.map { it.name })

        val detail = app.getvela.wallet.feature.settings.SettingsLive.balanceDetail(
            base.balanceDetail,
            view,
            app.getvela.wallet.feature.settings.core.CurrencyView("USD", null, true),
            names,
            strings,
        )
        val polygon = detail.pending.single { it.name == "Polygon" }
        val gnosis = detail.pending.single { it.name == "Gnosis" }
        assertNull("a rate limit offers no retry", polygon.action)
        assertTrue("a dead chain offers the retry", gnosis.action != null)
        assertEquals(listOf("Ethereum"), detail.done.map { it.name })
    }

    @Test
    fun anUnpricedHoldingIsNotCountedAsZeroDollars() {
        // Phase 4b has no price source, so every holding arrives unpriced. The
        // core must say the total is unknown rather than call it $0 — the whole
        // reason `display_total_usd` is nullable.
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }
        assertNull("an unpriced holding must not become a price", view.tokens.single().price_usd)
        assertTrue(
            "the core must hold at least one unpriced holding",
            view.unpriced_tokens.isNotEmpty() || view.notice != null || view.display_total_usd == null,
        )
    }

    // -- the batch, and what it is worth -------------------------------------

    /**
     * A quoter that answers one price per (tokenIn → tokenOut) pair.
     *
     * The batch is opaque hex by the time it reaches the transport, so the test
     * builds the ANSWER by position: whatever the executor put in slot `n`, the
     * fake replies to slot `n`. That is exactly the coupling under test — a
     * result is matched to its call by index and nothing else.
     */
    private fun batched(vararg words: String?): app.getvela.wallet.feature.wallet.core.RpcPostResult =
        FakeRpcTransport.body(aggregate3Return(words.toList()))

    private fun word(value: java.math.BigInteger): String =
        value.toString(16).padStart(64, '0')

    private fun aggregate3Return(entries: List<String?>): String {
        val elements = entries.map { data ->
            val payload = data ?: ""
            word(if (data == null) java.math.BigInteger.ZERO else java.math.BigInteger.ONE) +
                word(java.math.BigInteger.valueOf(0x40)) +
                word(java.math.BigInteger.valueOf((payload.length / 2).toLong())) +
                payload
        }
        val offsets = StringBuilder()
        var offset = entries.size * 32L
        elements.forEach { element ->
            offsets.append(word(java.math.BigInteger.valueOf(offset)))
            offset += element.length / 2
        }
        return "0x" + word(java.math.BigInteger.valueOf(32)) +
            word(java.math.BigInteger.valueOf(entries.size.toLong())) +
            offsets + elements.joinToString("")
    }

    private fun polygonLike(stables: List<ChainStable>) = ChainInfo(
        chainId = 137,
        native = ChainNative("Polygon", "POL", 18),
        stables = stables,
        wrappedNative = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
        dex = ChainDex("uniswap-v3", "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", null),
    )

    private val usdc = ChainStable("USDC", "native", "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174")
    private val dai = ChainStable("DAI", "bridge", "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063")

    /**
     * The whole batch, decoded and priced.
     *
     * Slots, in the order the executor builds them: native balance, USDC
     * balance, USDC decimals, wrapped balance, wrapped decimals, then four
     * V3 quotes (one per fee tier) of one POL against USDC.
     */
    @Test
    fun oneCallPerChainPricesTheCoinAndItsStables() {
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc))),
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("2000000000000000000")), // 2 POL
                word(java.math.BigInteger("5000000")), //              5 USDC
                word(java.math.BigInteger.valueOf(6)), //              USDC decimals
                null, //                                              no wrapped balance
                word(java.math.BigInteger.valueOf(18)),
                word(java.math.BigInteger.valueOf(250_000)), //        $0.25 per POL
                null,
                null,
                null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.size >= 2 }

        val pol = view.tokens.first { it.symbol == "POL" }
        assertEquals("2", pol.balance)
        assertEquals(0.25, pol.price_usd!!, 1e-9)

        val stable = view.tokens.first { it.symbol == "USDC" }
        assertEquals("5", stable.balance)
        // A curated stablecoin is a MEMBERSHIP verdict, not a missing factor
        // defaulting to 1.
        assertEquals(1.0, stable.price_usd!!, 1e-9)
        assertEquals(6, stable.decimals)

        // 2 × $0.25 + 5 × $1 = $5.50, and the core is the one that adds up.
        assertEquals(5.5, view.display_total_usd!!, 1e-9)
    }

    /**
     * **The 10^12 trap.**
     *
     * A chain carrying both a 6-decimal USDC and an 18-decimal DAI, where only
     * the DAI pool answers. Scaling that quote by the neighbouring USDC's
     * decimals prices the coin a trillion times too high. Each quote group must
     * carry its OWN `decimals()` read — which is the reason the calls are
     * grouped per stable rather than kept in one flat list.
     */
    @Test
    fun aQuoteIsScaledByItsOwnStablesDecimals() {
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc, dai))),
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("1000000000000000000")), // 1 POL
                null, word(java.math.BigInteger.valueOf(6)), //        USDC: none held, 6 decimals
                null, word(java.math.BigInteger.valueOf(18)), //       DAI:  none held, 18 decimals
                null, word(java.math.BigInteger.valueOf(18)), //       wrapped
                // USDC quotes: the pool is dead, every tier fails.
                null, null, null, null,
                // DAI quotes: 0.25 DAI per POL, in DAI's 18 base units.
                word(java.math.BigInteger("250000000000000000")), null, null, null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }

        val pol = view.tokens.first { it.symbol == "POL" }
        // Scaled by USDC's 6 this would be 250,000,000,000 dollars a coin.
        assertEquals(0.25, pol.price_usd!!, 1e-9)
    }

    /**
     * A DEX quote that disagrees with Chainlink by too much loses to it.
     *
     * The band is the core's (`choose_native_price`), and this test exists to
     * prove the shell actually hands it both numbers — a shell that forgot the
     * Chainlink argument would still look right whenever the pools are healthy.
     */
    @Test
    fun aThinPoolLosesToChainlink() {
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc))),
            mainnet = mapOf("MATIC" to 0.25), // POL's feed key on mainnet
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("1000000000000000000")),
                null, word(java.math.BigInteger.valueOf(6)),
                null, word(java.math.BigInteger.valueOf(18)),
                // A near-empty pool quoting POL at $5.
                word(java.math.BigInteger.valueOf(5_000_000)), null, null, null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }

        assertEquals(0.25, view.tokens.single().price_usd!!, 1e-9)
    }

    /**
     * A chain without Multicall3 still shows what it holds.
     *
     * The batch is a convenience; a balance is not. A chain somebody added
     * themselves may have no Multicall3 deployment, and losing a real holding
     * to a missing helper contract would be a worse failure than one extra
     * request.
     */
    @Test
    fun aChainWithoutTheBatchFallsBackToAPlainBalance() {
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc))),
        ) { _, method ->
            // `eth_call` answers with something that is not a batch at all.
            if (method == "eth_call") FakeRpcTransport.body("0x")
            else FakeRpcTransport.body("0x1bc16d674ec80000") // 2 POL
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }

        val pol = view.tokens.single()
        assertEquals("2", pol.balance)
        // No batch means no quotes and no feed: shown, and honestly unpriced.
        assertNull(pol.price_usd)
    }

    // -- custom tokens (spec 041 phase 4d) -----------------------------------

    private val newc = "0xdeadbeef0000000000000000000000000000cafe"

    /** One custom token in the store, as this device records them. */
    private fun withCustomToken(store: FakeStore) = store.also {
        runBlocking {
            it.write(
                app.getvela.wallet.core.data.KeyValueStore.Keys.CUSTOM_TOKENS,
                org.json.JSONArray().put(
                    org.json.JSONObject()
                        .put("id", "137_$newc")
                        .put("chainId", 137)
                        .put("contractAddress", newc)
                        .put("symbol", "NEWC")
                        .put("name", "New Coin")
                        .put("decimals", 18),
                ).toString(),
            )
        }
    }

    /**
     * A custom token is read and priced through the CORE's rule.
     *
     * Slots: native, USDC balance, USDC decimals, wrapped balance, wrapped
     * decimals, NEWC balance, then the native quote tiers, then NEWC's own.
     */
    @Test
    fun aCustomTokenIsReadAndPricedByTheCoresRule() {
        val store = withCustomToken(FakeStore())
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc))),
            store = store,
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("1000000000000000000")), // 1 POL
                null, word(java.math.BigInteger.valueOf(6)), //        USDC: none held
                null, word(java.math.BigInteger.valueOf(18)), //       wrapped
                word(java.math.BigInteger("3000000000000000000")), // 3 NEWC
                // POL priced at $0.25 against USDC.
                word(java.math.BigInteger.valueOf(250_000)), null, null, null,
                // NEWC → USDC: $2.00, in USDC's six decimals.
                word(java.math.BigInteger.valueOf(2_000_000)), null, null, null,
                // NEWC → wrapped POL: not needed, path A answered.
                null, null, null, null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.any { token -> token.symbol == "NEWC" } }

        val token = view.tokens.first { it.symbol == "NEWC" }
        assertEquals("3", token.balance)
        assertEquals(newc, token.token_address)
        assertEquals(2.0, token.price_usd!!, 1e-9)
        // 1 POL at $0.25 + 3 NEWC at $2.00 = $6.25, and the core adds up.
        assertEquals(6.25, view.display_total_usd!!, 1e-9)
    }

    /**
     * **The 10^12 trap, on the custom path.**
     *
     * Two stables with different decimals, and only the 18-decimal one quotes.
     * Scaling that by the 6-decimal neighbour's value prices the token a
     * trillion times too high — into a portfolio total and a sort order.
     */
    @Test
    fun aCustomTokensQuoteIsScaledByItsOwnStablesDecimals() {
        val store = withCustomToken(FakeStore())
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc, dai))),
            store = store,
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("1000000000000000000")), // 1 POL
                null, word(java.math.BigInteger.valueOf(6)), //        USDC
                null, word(java.math.BigInteger.valueOf(18)), //       DAI
                null, word(java.math.BigInteger.valueOf(18)), //       wrapped
                word(java.math.BigInteger("1000000000000000000")), // 1 NEWC
                // Native quotes: USDC answers $0.25.
                word(java.math.BigInteger.valueOf(250_000)), null, null, null,
                null, null, null, null,
                // NEWC → USDC: dead. NEWC → DAI: 0.5 DAI, at 18 decimals.
                null, null, null, null,
                word(java.math.BigInteger("500000000000000000")), null, null, null,
                // NEWC → wrapped: unused.
                null, null, null, null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.any { token -> token.symbol == "NEWC" } }

        // Scaled by USDC's 6 this would be 500,000,000,000 dollars a token.
        assertEquals(0.5, view.tokens.first { it.symbol == "NEWC" }.price_usd!!, 1e-9)
    }

    /**
     * A token nothing could price is held, shown, and not counted.
     *
     * Path B multiplies by the native coin's price, so it cannot run when the
     * coin has none: multiplying by an unknown is a fabrication, not a
     * fallback.
     */
    @Test
    fun aCustomTokenNothingCanPriceStaysUnpriced() {
        val store = withCustomToken(FakeStore())
        val h = harness(
            rows = listOf(row(137, "POL", "Polygon")),
            chains = mapOf(137 to polygonLike(listOf(usdc))),
            store = store,
        ) { _, _ ->
            batched(
                word(java.math.BigInteger("1000000000000000000")),
                null, word(java.math.BigInteger.valueOf(6)),
                null, word(java.math.BigInteger.valueOf(18)),
                word(java.math.BigInteger("1000000000000000000")), // 1 NEWC held
                // No quotes answer at all — not for POL, not for NEWC.
                null, null, null, null,
                null, null, null, null,
                null, null, null, null,
            )
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.any { token -> token.symbol == "NEWC" } }

        assertNull(view.tokens.first { it.symbol == "NEWC" }.price_usd)
    }

    /**
     * The hero names the chain that is really down — and only that one.
     *
     * A dead RPC and a rate-limited one both leave a chain unread, but only
     * the first is the person's to fix: the core's `banner_chain_ids` is failed
     * MINUS rate-limited, and the hero's line is worded from it. Then the fix:
     * `FixChainResolved` drops the chain and reads again, so the repaired
     * chain stops being named without waiting for the next throttled refresh
     * (the Android rescue never sent it).
     */
    @Test
    fun aDeadChainIsNamedARateLimitedOneIsNotAndTheFixClearsIt() {
        val gnosisDown = java.util.concurrent.atomic.AtomicBoolean(true)
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis"), row(137, "POL", "Polygon")),
        ) { url, _ ->
            when {
                url.contains("chain-100") && gnosisDown.get() -> FakeRpcTransport.network()
                url.contains("chain-137") -> FakeRpcTransport.httpError(429)
                else -> FakeRpcTransport.body("0x14d1120d7b160000")
            }
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val failing = h.host.settle { it.failed_chain_ids.containsAll(listOf(100, 137)) && it.rate_limited_chain_ids.contains(137) }
        assertEquals("the rate-limited chain never reaches the banner", listOf(100), failing.banner_chain_ids)
        val strings = app.getvela.wallet.core.i18n.I18nRuntime { tag ->
            java.io.File(System.getProperty("vela.repo.root")!!, "assets/i18n/$tag.json").readBytes()
        }.apply { initialize("en") }
        val line = app.getvela.wallet.feature.wallet.WalletLive.balanceStatus(failing, strings, mapOf(100 to "Gnosis", 137 to "Polygon"))
        assertEquals("Gnosis RPC unavailable", line?.text)

        gnosisDown.set(false)
        h.host.dispatch(BalanceEvent.FixChainResolved(100), BalanceEvent.serializer())
        // The drop is the core's immediate answer; whether the re-read then
        // lands is the pool's cooldown, which this test does not pin.
        val fixed = h.host.settle { !it.failed_chain_ids.contains(100) }
        assertFalse(fixed.banner_chain_ids.contains(100))
        assertEquals(null, app.getvela.wallet.feature.wallet.WalletLive.balanceStatus(fixed, strings, emptyMap())?.takeIf { it.text.contains("Gnosis") })
    }

    private companion object {
        const val ADDRESS = "0x1111111111111111111111111111111111111111"
        const val TIMEOUT = 20_000L
    }
}
