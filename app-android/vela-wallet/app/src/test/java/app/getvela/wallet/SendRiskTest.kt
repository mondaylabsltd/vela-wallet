package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendHapticKind
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.settings.core.NetView
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
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord

/**
 * The send machine's recipient-risk probe, answered from this device's own
 * history (the web's `resolveRecipientRisk`): an address never sent to is a
 * "first time" — the tag against address poisoning — and an address sent to
 * before is not. Android answered `first_time = null` for every recipient, so
 * the tag the send form draws (`SendLive.recipientNote`) never showed.
 */
class SendRiskTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun tearDown() = scope.cancel()

    private val store = FakeStore()
    private val port = FakeRelayPort()

    private fun executor(): SendExecutor {
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = object : RpcEndpointSource {
                override suspend fun forChain(chainId: Int) =
                    RpcSeeds(rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)))
            },
            scope = scope,
            transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
        )
        return SendExecutor(
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accounts = object : SendExecutor.AccountPort {
                override suspend fun keysOf(address: String): List<WalletKeyRecord> = emptyList()
                override suspend fun routingOf(address: String): Pair<String, KeyMethod> = error("unused")
                override suspend fun publicKeyOf(accountId: String): String? = null
            },
            balances = { BalanceView() },
            networks = { NetView(loaded = true) },
            signer = { error("no signing here") },
            feeQuoter = object : SendExecutor.FeeQuoter {
                override suspend fun quote(
                    chainId: Int,
                    account: String,
                    calls: List<app.getvela.wallet.feature.send.core.FeeCall>,
                    gasFeeToken: String?,
                    publicKeyAvailable: Boolean,
                    autoFeeToken: Boolean,
                ): SendFeeOutcome = error("unused")
            },
            ports = object : SendExecutor.SendPorts {
                override fun signingStarted() = Unit
                override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) = Unit
                override fun haptic(kind: SendHapticKind) = Unit
                override fun alert(kind: SendAlertKind) = Unit
                override fun closed() = Unit
                override fun refreshBalances() = Unit
                override fun recordsPersisted() = Unit
            },
        )
    }

    private fun firstTime(address: String): Boolean? = runBlocking {
        (executor().perform(SendOperation.ResolveRisk(chain_id = 137, address = address)) as SendShellResult.RiskResolved)
            .risk?.first_time
    }

    @Test
    fun `an address this device sent to before is not a first time`() {
        runBlocking {
            store.write(
                KeyValueStore.Keys.TRANSACTIONS,
                JSONArray().put(
                    JSONObject()
                        .put("id", "s1")
                        .put("type", "send")
                        .put("to", "0xAbCdEf0000000000000000000000000000000001")
                        .put("timestamp", 1_756_900_000_000L),
                ).toString(),
            )
        }
        assertEquals(false, firstTime("0xabcdef0000000000000000000000000000000001"))
        assertEquals("a stranger is a first time", true, firstTime("0x2222222222222222222222222222222222222222"))
    }

    @Test
    fun `an empty history makes every address a first time, and a non-address none`() {
        assertEquals(true, firstTime("0x2222222222222222222222222222222222222222"))
        assertEquals("not an address: no verdict to tag", false, firstTime("vitalik.eth"))
    }

    /**
     * Device-found: an EIP-7702 delegated account (code `0xef0100 ++ impl`)
     * was answered `is_contract = true`. The web's rule: it is a wallet.
     */
    @Test
    fun `a 7702-delegated recipient is a wallet and real code is a contract`() {
        val to = "0x2222222222222222222222222222222222222222"
        port.answer(
            "eth_getCode",
            FakeRelayPort.body("0xef0100" + "63c0c19a282a1b52b07dd5a65b58948a07dae32b"),
            FakeRelayPort.body("0x6080604052"),
            FakeRelayPort.body("0x"),
        )
        val contract = { runBlocking {
            (executor().perform(SendOperation.ResolveRisk(chain_id = 100, address = to)) as SendShellResult.RiskResolved)
                .risk?.is_contract
        } }
        assertEquals(false, contract())
        assertEquals(true, contract())
        assertEquals(false, contract())
        assertEquals("the chain could not be asked: no verdict", null, contract())
    }
}
