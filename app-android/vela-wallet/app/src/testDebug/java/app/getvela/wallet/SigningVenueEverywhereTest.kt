package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.TrustedSigner
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.core.GuardChoice
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.PageFailure
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.trustedsigner.PageSignFailure
import app.getvela.wallet.feature.signing.trustedsigner.SigningPlan
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.WalletKeyRecord
import uniffi.vela_dev_fixtures.fixtureAccounts
import uniffi.vela_dev_fixtures.fixtureMultiAddress

/**
 * R7 (owner, 2026-10-30: the page everywhere) on Android, through the REAL
 * sign_request + clear_signing + approval_guard + fee_policy machines and
 * the one submit spine: for an account that reviews and signs on its trusted
 * page, EVERY kind of signature asks the core where it goes
 * (`signing_route(account, method)`) and goes to the page — a site's
 * transaction, a batch, `personal_sign`, typed data — never to the device
 * passkey. What the page says back is the core's to name: the person's "no"
 * is `PageDeclined` (the site hears 4001), anything else `PageFailed` with
 * the core's `PageFailure`, said on the sheet in the core's words.
 *
 * Plus the two events this shell never sent before: the guard's per-leg
 * grant (`leg_grant_deliberately_chosen`) and the sign machine's
 * `landing_closed`.
 */
class SigningVenueEverywhereTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = fixtureMultiAddress()
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val origin = "http://127.0.0.1:8137"
    private val official = "https://sign.getvela.app/"
    private val answers = java.util.Collections.synchronizedList(ArrayList<Pair<String, SignResponsePayload>>())

    /** Every native passkey ceremony: none may run for a page account. */
    private val passkeys = java.util.concurrent.atomic.AtomicInteger()

    /** What the page was handed: the request's method, and the page it went to. */
    private val handed = java.util.concurrent.CopyOnWriteArrayList<Pair<String, String>>()

    @After
    fun stop() = scope.cancel()

    /** An account that reviews and signs on the official page (spec 102 R4), with its sign-in key (R5). */
    private fun seedPageAccount(venue: String = """{"type":"page","url":"$official"}""") {
        val keyset = fixtureAccounts()
        val keys = JSONArray()
        keyset.forEach { keys.put(JSONObject().put("credential_id", it.credentialIdHex).put("public_key_hex", it.publicKeyHex).put("name", it.name).put("transports", "internal")) }
        val account = JSONObject().put("id", keyset.first().credentialIdHex).put("name", "Parallel space").put("address", safe)
            .put("public_key_hex", keyset.first().publicKeyHex).put("created_at_iso", "2026-09-12T00:00:00Z").put("keys", keys)
            .put("signing_domain", "getvela.app")
            .put("signing_venue", JSONObject(venue))
            .put("sign_in_key", JSONObject().put("credential_id", keyset.first().credentialIdHex).put("method", "platform").put("transports", "internal"))
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
            fun row(max: String) = JSONObject().put("maxFeePerGas", max).put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")
            FakeRelayPort.body(JSONObject().put("fast", row("0x77359400")).put("standard", row("0x59682f00")).put("slow", row("0x4a817c80")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.always("eth_sendUserOperation") { FakeRelayPort.body("0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1") }
        port.always("eth_getUserOperationReceipt") { FakeRelayPort.body(JSONObject.NULL) }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    private val noPasskey = object : UserOpSigner {
        override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
            passkeys.incrementAndGet()
            error("a page account never signs with the device passkey")
        }
    }

    /** The trusted page, as the spine reaches it: it records what it was handed and answers [page] (`null`: the person said no). */
    private fun page(failure: PageFailure?) = object : TrustedSigner {
        override suspend fun sign(
            requestJson: String,
            digest: ByteArray,
            keys: List<WalletKeyRecord>,
            page: String,
            key: SigningPlan.KeyLabel?,
            askFirst: Boolean,
        ): Assertion {
            handed += JSONObject(requestJson).getJSONObject("intent").getString("method") to page
            throw PageSignFailure(failure, "the page did not sign")
        }
    }

    private fun controller(trusted: TrustedSigner): SigningController {
        val credential = fixtureAccounts().first().credentialIdHex
        return SigningController(
            scope = scope,
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accounts = StoreAccountPort(AccountStore(store)),
            signer = { noPasskey },
            knownChains = { listOf(1, 100) },
            wallet = SignAccountRef(address = safe, credential_id = credential),
            receiptWaitMs = 60_000L, receiptPollMs = 100L,
            trustedSigner = { trusted },
            ports = object : SigningController.Ports {
                override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                    answers += id to payload
                }
                override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) = Unit
                override fun recordsPersisted() = Unit
                override fun recordPersisted(recordId: String) = Unit
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            },
        )
    }

    private fun transaction() = IncomingRequest(
        "tx", "eth_sendTransaction",
        JSONArray().put(JSONObject().put("from", safe).put("to", founder).put("value", "0x38d7ea4c68000")).toString(),
        origin, "tab-1", 100,
    )

    private fun batch() = IncomingRequest(
        "batch", "wallet_sendCalls",
        JSONArray().put(
            JSONObject().put("version", "2.0.0").put("chainId", "0x64").put("from", safe).put("atomicRequired", true)
                .put("calls", JSONArray().put(JSONObject().put("to", founder).put("value", "0x1")).put(JSONObject().put("to", founder).put("value", "0x2"))),
        ).toString(),
        origin, "tab-1", 100,
    )

    private fun personal() = IncomingRequest("msg", "personal_sign", JSONArray().put("0x48656c6c6f2c2056656c61").put(safe).toString(), origin, "tab-1", 100)

    private fun typed(): IncomingRequest {
        val data = JSONObject()
            .put("types", JSONObject()
                .put("EIP712Domain", JSONArray().put(JSONObject().put("name", "name").put("type", "string")).put(JSONObject().put("name", "chainId").put("type", "uint256")))
                .put("Mail", JSONArray().put(JSONObject().put("name", "contents").put("type", "string"))))
            .put("primaryType", "Mail")
            .put("domain", JSONObject().put("name", "Example").put("chainId", 100))
            .put("message", JSONObject().put("contents", "Hello"))
        return IncomingRequest("typed", "eth_signTypedData_v4", JSONArray().put(safe).put(data.toString()).toString(), origin, "tab-1", 100)
    }

    private suspend fun answerTo(id: String): SignResponsePayload = withTimeout(60_000) {
        while (answers.none { it.first == id }) delay(50)
        answers.first { it.first == id }.second
    }

    private fun assertDeclined(answer: SignResponsePayload) {
        val err = answer as SignResponsePayload.Err
        assertEquals("the page is the venue: its 'no' is the person's 4001", 4001, err.code)
        assertEquals(SignErrorKind.UserRejected, err.kind)
    }

    @Test
    fun `a site's transaction goes to the trusted page, never the device passkey`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(null))
        c.open(transaction())
        // D4: the sheet IS the hand-off card for this method's route.
        assertEquals(official, withTimeout(10_000) { c.handoff.first { it != null } }!!.page)
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        c.approveWhenOpen()
        assertDeclined(answerTo("tx"))
        assertEquals(listOf("eth_sendTransaction" to official), handed.toList())
        assertEquals(0, passkeys.get())
        assertEquals("nothing reached the relay", 0, port.calls.count { it.endsWith("eth_sendUserOperation") })
    }

    @Test
    fun `a batch goes to the trusted page`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(null))
        c.open(batch())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        c.approveWhenOpen()
        assertDeclined(answerTo("batch"))
        assertEquals(listOf("wallet_sendCalls" to official), handed.toList())
        assertEquals(0, passkeys.get())
    }

    @Test
    fun `personal_sign goes to the trusted page`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(null))
        c.open(personal())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        c.approveWhenOpen()
        assertDeclined(answerTo("msg"))
        assertEquals(listOf("personal_sign" to official), handed.toList())
        assertEquals(0, passkeys.get())
    }

    @Test
    fun `typed data goes to the trusted page`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(null))
        c.open(typed())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        c.approveWhenOpen()
        assertDeclined(answerTo("typed"))
        assertEquals(listOf("eth_signTypedData_v4" to official), handed.toList())
        assertEquals(0, passkeys.get())
    }

    /**
     * Round 7: the page did not answer — not the person's "no". The site
     * hears -32603 with the core's reason, and the sheet says what happened
     * there in the core's words, never "couldn't submit".
     */
    @Test
    fun `a page that never answered is said as that, on the sheet and to the site`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(PageFailure.Unanswered))
        c.open(personal())
        withTimeout(20_000) { c.sign.first { it.surface == SignSurface.Sheet } }
        c.approveWhenOpen()
        val failed = withTimeout(30_000) { c.sign.first { it.error?.kind == SignErrorKind.PageUnanswered } }
        assertEquals(
            "componentsUi.signing.trustedSignerUnanswered",
            app.getvela.wallet.feature.signing.SigningLive.pageFailureKey(failed.error?.kind),
        )
        // The site's words are the core's (`sign_error_words`) — the developer's, never a success.
        assertTrue(uniffi.vela_core_uniffi.signErrorWords("page_unanswered", null)!!.isNotBlank())
        assertEquals(0, passkeys.get())
    }

    @Test
    fun `a page whose answer was not this request's is a mismatch, never a success`() = runBlocking<Unit> {
        seedPageAccount(); scriptRelay()
        val c = controller(page(PageFailure.Mismatch))
        c.open(transaction())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        c.approveWhenOpen()
        withTimeout(30_000) { c.sign.first { it.error?.kind == SignErrorKind.PageMismatch } }
        assertEquals("nothing was sent", 0, port.calls.count { it.endsWith("eth_sendUserOperation") })
    }

    // -- the guard's per-leg grant -------------------------------------------------

    /**
     * cs-17 in a batch: a leg that grants a whole collection
     * (`setApprovalForAll(operator, true)`) waits for its own deliberate
     * "Allow all" — the chip the sheet draws for THAT leg dispatches
     * `leg_grant_deliberately_chosen`, and the core records the grant on
     * that leg alone.
     */
    @Test
    fun `a batch leg's Allow all is the deliberate grant of that leg`() = runBlocking<Unit> {
        seedPageAccount(venue = """{"type":"in_vela"}"""); scriptRelay()
        val c = controller(page(null))
        val operator = "0x1111111111111111111111111111111111111111"
        val nft = "0x22c1f6050e56d2876009903609a2cc3fef83b415"
        val approveAll = "0xa22cb465" + operator.drop(2).padStart(64, '0') + "1".padStart(64, '0')
        c.open(
            IncomingRequest(
                "legs", "wallet_sendCalls",
                JSONArray().put(
                    JSONObject().put("version", "2.0.0").put("chainId", "0x64").put("from", safe).put("atomicRequired", true)
                        .put("calls", JSONArray().put(JSONObject().put("to", nft).put("data", approveAll).put("value", "0x0")).put(JSONObject().put("to", founder).put("value", "0x1"))),
                ).toString(),
                origin, "tab-1", 100,
            ),
        )
        val legs = withTimeout(20_000) { c.guard.first { view -> view.batch?.legs?.firstOrNull()?.approval?.is_boolean_grant == true } }
        assertTrue("the grant-all leg waits for a choice", legs.batch!!.legs[0].choice != GuardChoice.Grant)
        // The sheet draws this leg's chips as grant / revoke, the grant routed to its own leg.
        val blocks = app.getvela.wallet.feature.signing.SigningLive.guardBlocks(legs, strings())
        val card = blocks.filterIsInstance<app.getvela.wallet.feature.signing.SigningBlock.Allowance>().single { it.leg == 0 }
        assertEquals(listOf("grant", "revoke"), card.chips.map { it.id })
        c.guardChip("grant", leg = 0)
        val granted = withTimeout(10_000) { c.guard.first { it.batch?.legs?.get(0)?.choice == GuardChoice.Grant } }
        assertEquals(GuardChoice.Grant, granted.batch!!.legs[0].choice)
    }

    // -- landing_closed --------------------------------------------------------------

    /**
     * Round 3/5: the ✕ after approving closes the landing too. The core is
     * told (`landing_closed`) and keeps it for as long as the hand-off names
     * that operation — so the finished transaction is not raised again, and
     * the ending does not come back over what the person went to.
     */
    @Test
    fun `closing the sheet after approving tells the core the landing is closed`() = runBlocking<Unit> {
        // A device-passkey account: the transaction is signed and handed to the tracker.
        seedPageAccount(venue = """{"type":"in_vela"}"""); scriptRelay()
        val signing = object : UserOpSigner {
            override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                val signed = uniffi.vela_dev_fixtures.fixtureAssert(challenge, listOfNotNull(credentialIdHex), 0u)
                return Assertion(signed.credentialIdHex, signed.signatureDerHex, signed.authenticatorDataHex, signed.clientDataJsonHex, null, "platform")
            }
        }
        val credential = fixtureAccounts().first().credentialIdHex
        val c = SigningController(
            scope = scope,
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accounts = StoreAccountPort(AccountStore(store)),
            signer = { signing },
            knownChains = { listOf(1, 100) },
            wallet = SignAccountRef(address = safe, credential_id = credential),
            receiptWaitMs = 60_000L, receiptPollMs = 100L,
            ports = object : SigningController.Ports {
                override fun respond(transportId: String, id: String, payload: SignResponsePayload) { answers += id to payload }
                override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) = Unit
                override fun recordsPersisted() = Unit
                override fun recordPersisted(recordId: String) = Unit
                override suspend fun switchAccount(address: String) = true
                override fun nativeSymbol(chainId: Int) = "XDAI"
                override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
                override fun dataBase() = ""
                override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            },
        )
        c.open(transaction())
        withTimeout(30_000) { c.fee.first { it.confirm_fee_ready } }
        c.approveWhenOpen()
        // The op is on its way: the core holds its hand-off; the receipt has not landed.
        val inFlight = withTimeout(30_000) { c.sign.first { it.tracker_handoff != null && it.pending_op_hash != null } }
        assertTrue(!inFlight.landing_closed)
        c.swipeDismissed()
        val closed = withTimeout(10_000) { c.sign.first { it.landing_closed } }
        assertTrue("the core keeps the close for the hand-off's operation", closed.landing_closed)
        assertTrue("the ending stays off screen", c.landingClosed)
    }

    private fun strings(): app.getvela.wallet.core.i18n.VelaStrings {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        return app.getvela.wallet.core.i18n.I18nRuntime { tag -> java.io.File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
}
