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
            FakeRelayPort.body("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1")
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

    /** Every tracker handoff, in order (spec 082: the two submit facts ride along). */
    private val handoffs = java.util.concurrent.CopyOnWriteArrayList<app.getvela.wallet.feature.send.core.TrackHandoff>()

    /** Spec 082 RJ1: every op the tracker was told to forget — proven never sent. */
    private val withdraws = java.util.concurrent.CopyOnWriteArrayList<Pair<String, List<String>>>()

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
            controller.onTrackSubmitted = { handoff ->
                // Invariant ⑥: the pending row is on disk before tracking begins.
                val hash = handoff.userOpHash
                val stored = store.values[KeyValueStore.Keys.TRANSACTIONS].orEmpty()
                events += if (stored.contains(hash)) "track:$hash:persisted" else "track:$hash:NOT-PERSISTED"
                assertEquals(100, handoff.chainId)
                assertTrue(handoff.recordIds.isNotEmpty())
                // Spec 082 RJ1: the write-ahead hands the op over "may have
                // been sent" before its POST; the relay's verdict follows.
                handoffs += handoff
            }
            controller.onTrackWithdrawn = { hash, ids -> withdraws += hash to ids }
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
        assertEquals("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", receipt.user_op_hash)
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
        // Spec 082 RJ1: the rows were written ahead under the LOCAL hash; this
        // relay answers another one (`userop.hash_mismatch`), so they were
        // withdrawn and the relay's written and handed over.
        withTimeout(10_000) { while (events.none { it.startsWith("track:0xa1a1") }) kotlinx.coroutines.delay(50) }
        assertTrue("the write-ahead was handed over first, before the POST: $events", events.indexOf(events.first { it.startsWith("track:") }) < events.indexOf("relay.send"))
        assertEquals(listOf(handoffs.first().userOpHash), withdraws.map { it.first })
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", row.optString("userOpHash"))
        assertEquals("send", row.optString("type"))
        assertEquals("", row.optString("status"))
        assertTrue(row.optString("to").equals(recipient, ignoreCase = true))
        assertNotNull(events.firstOrNull { it == "track:0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1:persisted" })
        assertTrue(events.indexOf("sign") < events.indexOf("relay.send"))
        trace.cancel()
    }

    private suspend fun toTheSlide(c: SendController) {
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.setRecipient(recipient)
        c.setAmount("0.001")
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        try {
            withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.fee != null && it.can_confirm } }
        } catch (timeout: kotlinx.coroutines.TimeoutCancellationException) {
            throw AssertionError("DIAG never reached confirm: alert=${c.alert.value} || send=${c.send.value} || fee=${c.fee.value}", timeout)
        }
        c.slideConfirm()
    }

    /**
     * Spec 082 RA1/RA10 (owner ruling 1): the relay's reply was lost after a
     * POST that may have been acted on. The receipt says "may have been
     * sent" — never "failed, try again" — the row is kept under the LOCAL
     * hash with both submit facts (T184), and the tracker is handed it as a
     * lost reply, from the head read before the POST. Posted once.
     */
    @Test
    fun `a lost reply is may-have-been-sent, kept with the row and handed over as one`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_blockNumber") { FakeRelayPort.body("0x3e8") }
        port.always("eth_sendUserOperation") { events += "relay.send"; app.getvela.wallet.feature.wallet.core.RpcResult.Failed(rateLimited = false, maybeDelivered = true) }
        val c = controller()
        toTheSlide(c)
        val receipt = withTimeout(30_000) { c.send.first { it.stage == SendStage.Receipt && it.receipt != null } }
        assertEquals(SendReceiptStatus.MaybeSent, receipt.receipt!!.status)
        val local = receipt.user_op_hash!!
        assertTrue("the local op hash: $local", local.matches(Regex("^0x[0-9a-f]{64}$")))
        withTimeout(10_000) { while (handoffs.isEmpty()) kotlinx.coroutines.delay(50) }
        val row = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).getJSONObject(0)
        assertEquals(local, row.optString("userOpHash"))
        assertTrue("kept with the row for a restart", row.getBoolean("maybeSent"))
        assertEquals(1000L, row.getLong("submitBlock"))
        // Written ahead and handed over "may have been sent" before the POST;
        // the lost reply adds nothing — the row already says so.
        val handoff = handoffs.single()
        assertEquals(local, handoff.userOpHash)
        assertTrue(handoff.maybeSent)
        assertEquals(1000L, handoff.submitBlock)
        assertEquals("posted once: nobody pays twice", 1, events.count { it == "relay.send" })
    }

    /** Nothing left the device: a plain refusal the person may retry — no row, no tracker. */
    @Test
    fun `a relay that was never reached is not sent and leaves nothing to follow`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") { events += "relay.send"; app.getvela.wallet.feature.wallet.core.RpcResult.Failed(rateLimited = false, maybeDelivered = false) }
        val c = controller()
        toTheSlide(c)
        val refused = withTimeout(30_000) { c.send.first { it.tx_error != null } }
        assertTrue("not a receipt: ${refused.receipt}", refused.receipt?.status != SendReceiptStatus.MaybeSent)
        kotlinx.coroutines.delay(300)
        assertTrue(store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty() || JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).length() == 0)
        // Spec 082 RJ1: written ahead, then withdrawn — nothing is left to follow.
        assertEquals(handoffs.map { it.userOpHash }, withdraws.map { it.first })
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
        try {
            withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.can_confirm } }
        } catch (timeout: kotlinx.coroutines.TimeoutCancellationException) {
            // Which way it stopped: the core's own refusal (an estimate that
            // outran its 15 s budget says `estimate_failed{timeout}`), or a
            // wait that never settled (no alert, the fee view still busy).
            throw AssertionError("DIAG never reached confirm: alert=${c.alert.value} || send=${c.send.value} || fee=${c.fee.value}", timeout)
        }
        c.slideConfirm()
        val failed = withTimeout(30_000) { c.send.first { it.tx_error != null || it.receipt != null } }
        assertNotNull("the relay refused; the core must say so", failed.tx_error)
        assertEquals(1, signs)
        kotlinx.coroutines.delay(300)
        // Spec 082 RJ1: the rows written ahead of the POST were withdrawn.
        assertTrue(store.values[KeyValueStore.Keys.TRANSACTIONS].isNullOrEmpty() || JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS)).length() == 0)
        assertEquals(handoffs.map { it.userOpHash }, withdraws.map { it.first })
    }

    /**
     * Spec 082 RJ1 (G34, P0): the wallet's own Send writes ahead too. At the
     * moment of the POST the row is already on disk, pending, "may have been
     * sent", and the tracker holds it; the relay taking the op (under the
     * hash the wallet computed) marks it sent, in place, and hands the same op
     * over again, admitted.
     */
    @Test
    fun `the rows are on disk before the POST, and the relay taking the op marks them sent`() = runBlocking {
        seedAccount(); scriptRelay()
        port.always("eth_sendUserOperation") {
            events += "relay.send"
            val rows = JSONArray(store.values[KeyValueStore.Keys.TRANSACTIONS] ?: "[]")
            val ahead = (0 until rows.length()).map { rows.getJSONObject(it) }.singleOrNull { it.optBoolean("maybeSent") }
            events += if (ahead != null && handoffs.isNotEmpty()) "relay.saw-record-and-tracker" else "relay.NOT-WRITTEN-AHEAD"
            FakeRelayPort.body(ahead?.getString("userOpHash"))
        }
        val c = controller()
        toTheSlide(c)
        val receipt = withTimeout(30_000) { c.send.first { it.stage == SendStage.Receipt && it.receipt != null } }
        assertEquals(SendReceiptStatus.Submitted, receipt.receipt!!.status)
        assertTrue("written ahead and tracked when the POST left: $events", "relay.saw-record-and-tracker" in events)
        withTimeout(10_000) { while (handoffs.size < 2) kotlinx.coroutines.delay(20) }
        kotlinx.coroutines.delay(200)
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals("one row, never a second for the same op", 1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals(receipt.user_op_hash, row.getString("userOpHash"))
        assertEquals("the relay took it", false, row.getBoolean("maybeSent"))
        assertEquals(listOf(true to false, false to true), handoffs.map { it.maybeSent to it.admitted })
        assertTrue(withdraws.isEmpty())
        assertEquals(1, events.count { it == "relay.send" })
    }

    /**
     * Spec 082 RJ1 (DX9): the app is gone while the POST is out. One pending
     * "may have been sent" row is on disk under the op's hash, and the next
     * launch's tracker reads it back as exactly that.
     */
    @Test
    fun `a quit while the POST is out leaves one pending may-have-been-sent row`() = runBlocking {
        seedAccount(); scriptRelay()
        val posting = kotlinx.coroutines.CompletableDeferred<Unit>()
        port.before = { method -> if (method == "eth_sendUserOperation") { posting.complete(Unit); kotlinx.coroutines.awaitCancellation() } }
        val c = controller()
        toTheSlide(c)
        withTimeout(30_000) { posting.await() }
        scope.cancel()
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals(1, rows.length())
        val row = rows.getJSONObject(0)
        assertEquals("send", row.getString("type"))
        assertTrue(row.getBoolean("maybeSent"))
        val pending = app.getvela.wallet.feature.send.core.TrackerExecutor.pendingRecord(row)!!
        assertTrue(pending.maybe_sent)
        assertEquals(handoffs.single().userOpHash, pending.user_op_hash)
    }

    /**
     * Spec 082 RJ1 (DX9) for a split Send: every recipient's row is written
     * ahead, in one write, under the one op's hash, before its POST — a quit
     * while it is out leaves them all pending "may have been sent", handed to
     * the tracker together.
     */
    @Test
    fun `a split send's rows are all written ahead, and a quit leaves them all may-have-been-sent`() = runBlocking {
        seedAccount(); scriptRelay()
        val posting = kotlinx.coroutines.CompletableDeferred<Unit>()
        port.before = { method -> if (method == "eth_sendUserOperation") { posting.complete(Unit); kotlinx.coroutines.awaitCancellation() } }
        val c = controller()
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.enterSplit()
        c.seedSplit(
            listOf(
                app.getvela.wallet.feature.send.core.SendRecipientDraft(id = "a", address = recipient, amount = "0.001"),
                app.getvela.wallet.feature.send.core.SendRecipientDraft(id = "b", address = "0x9F3c000000000000000000000000000000021aE0", amount = "0.002"),
            ),
        )
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        try {
            withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.fee != null && it.can_confirm } }
        } catch (timeout: kotlinx.coroutines.TimeoutCancellationException) {
            throw AssertionError("DIAG never reached confirm: alert=${c.alert.value} || send=${c.send.value} || fee=${c.fee.value}", timeout)
        }
        c.slideConfirm()
        withTimeout(30_000) { posting.await() }
        scope.cancel()
        val rows = JSONArray(store.values.getValue(KeyValueStore.Keys.TRANSACTIONS))
        assertEquals("one row per recipient", 2, rows.length())
        val hashes = (0 until rows.length()).map { rows.getJSONObject(it).getString("userOpHash") }.toSet()
        assertEquals("one op", 1, hashes.size)
        assertTrue((0 until rows.length()).all { rows.getJSONObject(it).getBoolean("maybeSent") })
        val handoff = handoffs.single()
        assertEquals(hashes.single(), handoff.userOpHash)
        assertEquals(2, handoff.recordIds.size)
        assertTrue(handoff.maybeSent)
    }

    /** Spec 082 RJ1: rows the store would not take are never posted — "not sent", nothing to follow. */
    @Test
    fun `rows the store would not take are never posted`() = runBlocking {
        seedAccount(); scriptRelay()
        val c = controller()
        c.open(SendAccountRef(id = safe, address = safe, name = "Parallel space"), SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2))
        val picked = withTimeout(10_000) { c.send.first { it.tokens.isNotEmpty() } }
        c.selectToken(SendLive.tokenId(picked.tokens.single()))
        withTimeout(10_000) { c.send.first { it.stage == SendStage.EnterDetails } }
        c.setRecipient(recipient); c.setAmount("0.001")
        withTimeout(10_000) { c.send.first { it.can_continue } }
        c.continueTapped()
        withTimeout(30_000) { c.send.first { it.stage == SendStage.Confirm && it.fee != null && it.can_confirm } }
        store.refuseWrites = true
        c.slideConfirm()
        val failed = withTimeout(30_000) { c.send.first { it.tx_error != null || it.receipt != null } }
        assertNotNull("not sent: ${failed.receipt}", failed.tx_error)
        assertEquals("zero POSTs", 0, events.count { it == "relay.send" })
    }
}
