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

    /** The person's own accounts on this device, and the contacts they named (the web's ports). */
    private val ownAccounts = mutableMapOf<String, String>()
    private val namedContacts = mutableMapOf<String, String>()

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
                override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
                override fun haptic(kind: SendHapticKind) = Unit
                override fun alert(kind: SendAlertKind) = Unit
                override fun closed() = Unit
                override fun refreshBalances() = Unit
                override fun recordsPersisted() = Unit
            },
            knownName = { address ->
                ownAccounts[address.lowercase()] ?: namedContacts[address.lowercase()]
            },
            ownAddress = { address -> address.lowercase() in ownAccounts },
        )
    }

    private fun risk(address: String, chainId: Int = 137) = runBlocking {
        (executor().perform(SendOperation.ResolveRisk(chain_id = chainId, address = address)) as SendShellResult.RiskResolved).risk!!
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

    // -- native parity A1: the web's recipient-risk.ts, port for port -------

    @Test
    fun `one of the person's own accounts is said as theirs — never a stranger`() {
        val own = "0x88cca0eedbf2c4426110bbfc998f048689266894"
        ownAccounts[own] = "Savings"
        val answer = risk("0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        assertEquals(true, answer.own)
        assertEquals("Savings", answer.known_name)
        assertEquals(false, answer.history_unread)
    }

    @Test
    fun `a contact the person named is their word for it, and not their own account`() {
        namedContacts["0x2222222222222222222222222222222222222222"] = "Bob"
        val answer = risk("0x2222222222222222222222222222222222222222")
        assertEquals("Bob", answer.known_name)
        assertEquals(false, answer.own)
        assertEquals("a named contact never paid is still a first transfer (the core decides what to say)", true, answer.first_time)
    }

    @Test
    fun `a history that could not be read is said, not guessed`() {
        // Round 3: a failed read is never "never sent here" — first_time is
        // unknown and history_unread says so, for the confirm's Check again.
        store.failReads = true
        val answer = risk("0x2222222222222222222222222222222222222222")
        assertEquals(null, answer.first_time)
        assertEquals(true, answer.history_unread)
    }

    @Test
    fun `history bytes that are not a list are unreadable too, but no history at all is an empty one`() {
        runBlocking { store.write(KeyValueStore.Keys.TRANSACTIONS, "{not a list") }
        assertEquals(true, risk("0x2222222222222222222222222222222222222222").history_unread)
        store.values.remove(KeyValueStore.Keys.TRANSACTIONS)
        val empty = risk("0x2222222222222222222222222222222222222222")
        assertEquals(false, empty.history_unread)
        assertEquals(true, empty.first_time)
    }

    @Test
    fun `the person's own name is the identity before any lookup`() {
        ownAccounts["0x88cca0eedbf2c4426110bbfc998f048689266894"] = "Savings"
        val identity = runBlocking {
            (executor().perform(SendOperation.ResolveIdentity("0x88cCA0EeDbF2C4426110bbFc998F048689266894")) as SendShellResult.IdentityResolved).identity
        }
        assertEquals(app.getvela.wallet.feature.send.core.SendRecipientIdentity(name = "Savings", source = "self"), identity)
    }

    @Test
    fun `a multisig proposal is answered at once, never left to spin`() {
        // Multisig orchestration is web-only until the core carries it; the
        // answer is the wire's nearest reason, never silence.
        val answer = runBlocking {
            executor().perform(
                SendOperation.ProposeSafeTx(
                    chain_id = 100,
                    multisig = "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                    transfers = listOf(app.getvela.wallet.feature.send.core.ReviewedTransfer("0x2222222222222222222222222222222222222222", null, "1")),
                ),
            )
        }
        assertEquals(
            SendShellResult.ProposeFailed(cancelled = false, reason = app.getvela.wallet.feature.send.core.SendProposeFailure.MailboxDown),
            answer,
        )
        assertEquals(answer, executor().neutralAnswer(SendOperation.ProposeSafeTx(100, "0x1", emptyList())))
    }
}
