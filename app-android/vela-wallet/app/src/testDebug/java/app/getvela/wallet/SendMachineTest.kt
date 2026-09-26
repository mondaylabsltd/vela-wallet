package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendHapticKind
import app.getvela.wallet.feature.send.core.SendReceiptStatus
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcPool
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureAssert
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * A whole send through the real `send` and `fee_policy` machines (spec 043
 * T029): pick → form → Continue quotes → confirm shows the quoted fee →
 * confirm signs ONCE with the parallel space's keyset → the relay accepts →
 * the pending row is on disk BEFORE the tracker is handed the hash.
 *
 * The relay is a script; the signer is the fixture keyset through the
 * debug-only library, which is what lets this run without a finger and why
 * the test lives in the debug source set.
 */
class SendMachineTest {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val recipient = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    // Appended to from the machines' threads while the test iterates it: a plain
    // list threw ConcurrentModificationException about once in 150 loaded runs.
    private val events = java.util.concurrent.CopyOnWriteArrayList<String>()
    private var signs = 0

    @After
    fun stop() = scope.cancel()

    /** [signedInWith]: the key the account signed in with (2026-09-26); `null` = a record from before. */
    private fun seedAccount(signedInWith: JSONObject? = null) {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject()
            .put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
        if (signedInWith != null) account.put("signed_in_with", signedInWith)
        store.values["vela.accounts"] = JSONArray().put(account).toString()
        store.values["vela.activeAccountIndex"] = "0"
    }

    private fun scriptRelay() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            FakeRelayPort.body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", "0x77359400").put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.always("eth_sendUserOperation") { params ->
            // The row must already be on disk? No — the row is written AFTER
            // the relay accepts (persist follows submit); what must hold is
            // that tracking follows the write. Recorded below.
            events += "relay.send"
            // Spec 069: the speed the displayed fee was priced at, by name.
            events += "relay.tier:${params.getOrNull(2) ?: "-"}"
            FakeRelayPort.body("0xhash")
        }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    /** Where each ceremony went: the pinned credential, its transports, its method. */
    private val routes = java.util.concurrent.CopyOnWriteArrayList<Triple<String?, String, KeyMethod>>()

    private val fixtureSigner = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            signs += 1
            events += "sign"
            routes += Triple(credentialIdHex, transports, method)
            val signed = fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
            return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
        }
    }

    private fun controller(): SendController {
        val pool = RpcPool(store = FakeStore(), endpoints = FakeEndpointSource(listOf("https://rpc.test")), scope = scope, transport = FakeRpcTransport { _, _ -> FakeRpcTransport.network() })
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        return SendController(
            scope = scope,
            relay = relay,
            pool = pool,
            feed = feed,
            accountStore = AccountStore(store),
            balances = { BalanceView(tokens = listOf(BalanceToken(chain_id = 100, symbol = "XDAI", name = "xDAI", balance = "0.71697", decimals = 18, token_address = null, price_usd = 1.0))) },
            networks = { NetView(loaded = true, networks = listOf(NetNetworkRow(id = "gnosis", chain_id = 100, display_name = "Gnosis", native_symbol = "xDAI", rpc_url = "https://rpc.test", explorer_url = "https://gnosisscan.io", bundler_url = "https://relay.test/100"))) },
            signer = { fixtureSigner },
            haptic = { kind -> events += "haptic:$kind" },
            refreshBalances = { events += "refresh" },
        ).also { controller ->
            controller.onTrackSubmitted = { hash, ids, chain ->
                // Invariant ⑥: the pending row is on disk before tracking begins.
                val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                events += if (stored.contains(hash)) "track:$hash:persisted" else "track:$hash:NOT-PERSISTED"
                assertEquals(100, chain)
                assertTrue(ids.isNotEmpty())
            }
        }
    }

    @Test
    fun `dust leaves the fixture Safe through the real machines`() = runBlocking {
        seedAccount(); scriptRelay()
        val c = controller()
        val trace = scope.launch { c.send.collect { println("send view: stage=${it.stage} loading=${it.loading} tokens=${it.tokens.size} fee=${it.fee != null} canContinue=${it.can_continue} canConfirm=${it.can_confirm} tx=${it.tx_status} err=${it.tx_error} receipt=${it.receipt?.status}") } }
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))

        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.setRecipient(recipient)
        c.setAmount("0.001")
        withTimeout(10_000) { c.send.first { it.can_continue } }

        c.continueTapped()
        val confirm = withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.fee != null && it.can_confirm } }
        assertEquals("0.001", confirm.confirm_amount)
        assertTrue(confirm.fee!!.quoted)

        c.slideConfirm()
        val receipt = withTimeout(30_000) { c.send.first { it.stage == SendStage.Receipt && it.receipt != null } }
        assertEquals(SendReceiptStatus.Submitted, receipt.receipt!!.status)
        assertEquals("0xhash", receipt.user_op_hash)
        assertEquals("signed exactly once", 1, signs)
        // A record from before the sign-in key: the first key, over its stored route.
        assertEquals(listOf(Triple(fixtureAccounts().first().credentialIdHex, "internal", KeyMethod.Platform)), routes)
        assertEquals(1, port.calls.count { it.endsWith("eth_sendUserOperation") })
        // The stored default (the factory Fast, here) priced the quote, and the
        // same tier is named on the wire beside the fee it priced (spec 069).
        assertEquals(FeeTier.Fast, confirm.fee!!.tier)
        assertTrue("the wire names the tier: $events", "relay.tier:fast" in events)

        // The pending row, in the feed's own shape, and the handoff after it.
        // The handoff happens after the last view emission, so poll rather than wait on the flow.
        withTimeout(10_000) { while (events.none { it.startsWith("track:") }) kotlinx.coroutines.delay(50) }
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals("0xhash", row.optString("userOpHash"))
        assertEquals("send", row.optString("type"))
        assertEquals("", row.optString("status"))
        assertTrue(row.optString("to").equals(recipient, ignoreCase = true))
        assertNotNull(events.firstOrNull { it == "track:0xhash:persisted" })
        assertTrue(events.indexOf("sign") < events.indexOf("relay.send"))
        trace.cancel()
    }

    /**
     * Founder, 2026-09-26: the person's own send signs with the key the account
     * signed in with, over the route that reached it — its SECOND key, reached
     * from a phone by scanning a code. There is no picker to say otherwise.
     */
    @Test
    fun `a send signs with the key the account signed in with, over its route`() = runBlocking {
        val second = fixtureAccounts()[1].credentialIdHex
        seedAccount(signedInWith = JSONObject().put("credential_id", second).put("method", "hybrid")); scriptRelay()
        val c = controller()
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.setRecipient(recipient)
        c.setAmount("0.001")
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.fee != null && it.can_confirm } }
        c.slideConfirm()
        val receipt = withTimeout(30_000) { c.send.first { it.stage == SendStage.Receipt && it.receipt != null } }
        assertEquals(SendReceiptStatus.Submitted, receipt.receipt!!.status)
        assertEquals(listOf(Triple<String?, String, KeyMethod>(second, "hybrid,internal", KeyMethod.Hybrid)), routes)
    }

    @Test
    fun `a relay rejection is the core's failure, and nothing was persisted`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { FakeRelayPort.error("AA25 invalid account nonce") }
        val c = controller()
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.setRecipient(recipient); c.setAmount("0.001")
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.can_confirm } }
        c.slideConfirm()
        val failed = withTimeout(30_000) { c.send.first { it.tx_error != null || it.receipt != null } }
        assertNotNull("the relay refused; the core must say so", failed.tx_error)
        assertEquals(1, signs)
        assertTrue(store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty())
    }
}
