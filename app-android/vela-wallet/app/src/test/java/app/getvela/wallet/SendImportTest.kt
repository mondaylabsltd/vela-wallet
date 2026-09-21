package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.BatchUnit
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Issue #271 over the REAL send and batch cores (the bridges, not fakes):
 * an import ADDS to a recipient typed by hand; replacing is the person's
 * explicit choice. Relay, RPC and storage are faked — nothing is signed.
 */
class SendImportTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private class Endpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private val holdings = BalanceView(
        tokens = listOf(
            BalanceToken(chain_id = 43114, symbol = "AVAX", name = "Avalanche", balance = "10", decimals = 18, price_usd = 20.0),
        ),
    )

    private fun controller(): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val store = FakeStore()
        return SendController(
            scope = scope,
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = RpcPool(
                store = FakeStore(),
                endpoints = Endpoints(),
                scope = scope,
                transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
            ),
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accountStore = AccountStore(store),
            balances = { holdings },
            networks = { NetView(loaded = true, networks = listOf(NetNetworkRow(id = "avalanche", chain_id = 43114, display_name = "Avalanche", native_symbol = "AVAX"))) },
            signer = { error("no signing in these tests") },
            haptic = {},
            refreshBalances = {},
        )
    }

    private fun <T> StateFlow<T>.await(what: String, test: (T) -> Boolean): T = runBlocking {
        try {
            withTimeout(10_000) { first(test) }
        } catch (timeout: Throwable) {
            throw AssertionError("$what — last: $value", timeout)
        }
    }

    /** On the AVAX form with one recipient typed by hand, then the importer open with two rows parsed. */
    private fun typedThenImporting(): SendController {
        val send = controller()
        send.open(
            account = SendAccountRef(id = ME, address = ME, name = "Me"),
            display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2),
            params = SendOpenParams(preselected_symbol = "AVAX", preselected_network = "chain-43114"),
        )
        send.send.await("on the AVAX form") { it.stage == SendStage.EnterDetails && it.selected_token?.symbol == "AVAX" }
        send.setRecipient(TYPED)
        send.setAmount("0.025907")
        send.send.await("the typed row counts against the import's room") { it.recipient == TYPED && it.split_import_room == 59 }

        send.openBatch()
        send.batch.await("the importer opened") { it.opened }
        send.batchUnit(BatchUnit.Token)
        send.batchText("$IMPORTED_A,1\n$IMPORTED_B,2")
        send.batch.await("both rows parsed and importable") { it.can_apply && it.recipients.size == 2 }
        return send
    }

    @Test
    fun `an import adds to a recipient typed by hand`() {
        val send = typedThenImporting()

        send.batchApply()

        val view = send.send.await("three rows in split mode") { it.split_mode && it.recipients.size == 3 }
        assertEquals(listOf(TYPED, IMPORTED_A, IMPORTED_B), view.recipients.map { it.address })
        assertEquals("0.025907", view.recipients.first().amount)
    }

    @Test
    fun `replacing is the person's explicit choice`() {
        val send = typedThenImporting()

        send.toggleImportReplaces()
        send.batchApply()

        val view: SendView = send.send.await("the imported rows only") { it.split_mode && it.recipients.size == 2 }
        assertEquals(listOf(IMPORTED_A, IMPORTED_B), view.recipients.map { it.address })
        // A choice about ONE import: the next one adds again.
        assertEquals(false, send.importReplaces.value)
    }

    private companion object {
        const val ME = "0x576a2cc9e6adc0c95989fa6aa104290aa940c73f"
        const val TYPED = "0x576a2cc9e6adc0c95989fa6aa104290aa940c73e"
        const val IMPORTED_A = "0x1111111111111111111111111111111111111111"
        const val IMPORTED_B = "0x2222222222222222222222222222222222222222"
    }
}
