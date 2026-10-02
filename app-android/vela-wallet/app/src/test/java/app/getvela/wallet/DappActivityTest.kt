package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.TxTechnicalLine
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.core.ClearRisk
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.GuardTokenMetaView
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignApproveOpts
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignRecord
import app.getvela.wallet.feature.signing.core.SignRecordKind
import app.getvela.wallet.feature.signing.core.SignRecordStatus
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignSponsorship
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.DappAction
import app.getvela.wallet.feature.wallet.core.DappSummary
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedFact
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedLine
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ActivityFeedCore
import uniffi.vela_core_uniffi.SignRequestCore

/**
 * Spec 093 on Android: every dApp interaction in Activity, said in the core's
 * words — one test per wiring point, on the REAL `sign_request` and
 * `activity_feed` machines wherever a machine is involved.
 *
 * approve → (the sign machine builds the summary, cuts the request, keeps no
 * signature) → the record row stores all of it verbatim → the store reads it
 * back → the feed decides title, place, second line, figure or allowance,
 * facts and technical lines → this shell words and draws them.
 */
class DappActivityTest {

    private val root: String = System.getProperty("vela.repo.root")
        ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")

    private fun strings(tag: String): VelaStrings =
        I18nRuntime { t -> File(root, "assets/i18n/$t.json").readBytes() }.apply { initialize(tag) }

    private val en by lazy { strings("en") }
    private val zh by lazy { strings("zh") }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private val chains = mapOf(1 to "Ethereum")

    // -- approve: what the sheet sends ------------------------------------------------

    /**
     * The approve copies three things and decides none: the core's
     * `record_intent` (never the reading's own intent — a best-effort guess
     * records nothing), the guard's token as it resolved, and the
     * simulation's judgments exactly as the sheet drew them.
     */
    @Test
    fun `the approve copies the core's record intent, the guard's token and the sheet's judgments`() {
        val guessed = Wire.json.decodeFromString(
            ClearSigningView.serializer(),
            """{"resolved":true,"result":{"intent":"transferFrom","best_effort":true},"record_intent":null}""",
        )
        assertNull("a reading the core will not record is not recorded", SigningController.approveOpts(FeeView(), guessed, GuardView()).intent)
        val swap = guessed.copy(record_intent = "Swap")
        assertEquals("Swap", SigningController.approveOpts(FeeView(), swap, GuardView()).intent)

        val usdc = GuardTokenMetaView(symbol = "USDC", decimals = 6, verified = true, loading = false)
        assertEquals(usdc, SigningController.approveOpts(FeeView(), swap, GuardView(meta = usdc)).token_meta)

        val judgments = listOf(
            TrustSimJudgment.Erc20Trusted(token = USDC, delta = "-100000000", symbol = "USDC", decimals = 6, in_trusted_set = true),
            TrustSimJudgment.Native(delta = "30000000000000000"),
        )
        val opts = SigningController.approveOpts(FeeView(), swap, GuardView(meta = usdc), SigningController.SimOutcome.Ready(judgments))
        assertEquals(judgments, opts.balance_changes)
        val wire = JSONObject(Wire.json.encodeToString(SignApproveOpts.serializer(), opts))
        assertEquals(true, wire.getJSONArray("balance_changes").getJSONObject(0).getBoolean("in_trusted_set"))
        assertEquals("USDC", wire.getJSONObject("token_meta").getString("symbol"))
        assertEquals("Swap", wire.getString("intent"))

        // Nothing drawn under "Balance changes" is nothing kept: still out, a notice, no move.
        assertNull(SigningController.approvedChanges(null))
        assertNull(SigningController.approvedChanges(SigningController.SimOutcome.Notice(ClearRisk.Caution, "componentsUi.signing.simUnavailable")))
        assertNull(SigningController.approvedChanges(SigningController.SimOutcome.Ready(emptyList())))
    }

    /** The trust machine's word that a coin is one the wallet already trusts survives the trip to the record. */
    @Test
    fun `a judgment keeps whether its coin is trusted`() {
        val judged = Wire.json.decodeFromString(
            TrustSimJudgment.serializer(),
            """{"type":"erc20_trusted","token":"$USDC","delta":"5","symbol":"USDC","decimals":6,"in_trusted_set":true}""",
        )
        assertEquals(true, (judged as TrustSimJudgment.Erc20Trusted).in_trusted_set)
    }

    // -- the record: what the sign machine keeps, and what the row stores -------------

    /**
     * The REAL sign machine signs a Permit2 permit and a sign-in: each record
     * carries the core's summary — read from the whole request, against the
     * site that asked — and an empty result, because the disk keeps that a
     * signature was given, never the signature. The row stores the summary,
     * the core's request and its truncation flag verbatim.
     */
    @Test
    fun `a signature's record keeps the core's summary and never the signature`() = runBlocking {
        val usdc = GuardTokenMetaView(symbol = "USDC", decimals = 6, verified = true, loading = false)
        val permit = signedByCore("eth_signTypedData_v4", permitParams(), SignApproveOpts(token_meta = usdc, unlimited_approved = true))
        assertEquals(SignRecordKind.SignTypedData, permit.kind)
        assertEquals("the signature stays with the page", "", permit.result)
        val summary = permit.summary!!
        assertEquals(DappAction.Permit, summary.action)
        assertEquals(ROUTER, summary.spender)
        assertEquals(USDC, summary.token)
        assertEquals("USDC", summary.symbol)
        assertTrue(summary.unlimited)
        assertEquals("PermitSingle", summary.primary_type)
        // The core keeps the final params as it re-wrote them — the same request, its keys in its order.
        assertEquals(requestOf(permitParams()), requestOf(permit.stored_request))
        assertFalse(permit.request_truncated)

        val row = SignExecutor.recordRow(permit, "ETH")
        assertEquals("", row.getString("txHash"))
        assertEquals(permit.stored_request, row.getString("signedRequest"))
        assertEquals(false, row.getBoolean("requestTruncated"))
        assertEquals(summary, Wire.json.decodeFromString(DappSummary.serializer(), row.getJSONObject("dappSummary").toString()))
        assertEquals("sign_typed_data", row.getString("type"))

        val siwe = signedByCore("personal_sign", siweParams(), SignApproveOpts())
        assertEquals(SignRecordKind.SignMessage, siwe.kind)
        assertEquals("", siwe.result)
        assertEquals(DappAction.SignIn, siwe.summary?.action)
        assertEquals("app.uniswap.org", siwe.summary?.signin_domain)
    }

    /**
     * A request past the core's 8 KB: the row stores the core's cut — never
     * one of its own — and says it was cut; the summary was read from the
     * whole request before anything was cut.
     */
    @Test
    fun `a long request is stored as the core cut it`() = runBlocking {
        val long = JSONArray().put("x".repeat(20_000)).put(ME).toString()
        val record = signedByCore("personal_sign", long, SignApproveOpts())
        assertTrue(record.request_truncated)
        assertTrue("within the core's 8 KB", record.stored_request.toByteArray().size <= 8 * 1024)
        assertEquals(DappAction.Message, record.summary?.action)
        val row = SignExecutor.recordRow(record, "ETH")
        assertEquals(record.stored_request, row.getString("signedRequest"))
        assertEquals(true, row.getBoolean("requestTruncated"))
        assertNotEquals("never this shell's old 4096-character cut", long.take(4096), row.getString("signedRequest"))
    }

    // -- reading it back ---------------------------------------------------------------

    /**
     * The store hands a dApp record's own fields back to the core — the
     * summary and the judgments verbatim, the intent, the site's origin
     * (falling back to `dappOrigin` for a record from before 083: this client
     * never stored a dApp's own name there) — and nothing for other kinds. A
     * part that does not read is absent, never a lost record.
     */
    @Test
    fun `a dApp record's fields cross back to the core, and an unreadable part is only absent`() = runBlocking {
        val store = FakeStore()
        val summary = DappSummary(action = DappAction.Call, calls = 1, contract = ROUTER)
        val judgments = listOf(TrustSimJudgment.Native(delta = "-1"))
        val swap = SignExecutor.recordRow(swapRecord().copy(summary = summary, balance_changes = judgments), "ETH")
        val legacy = JSONObject()
            .put("id", "legacy-sig").put("type", "sign_message").put("timestamp", 1_790_000_000)
            .put("txHash", "0x" + "ab".repeat(65)).put("dappOrigin", "https://app.uniswap.org").put("chainId", 1)
        val odd = JSONObject(swap.toString()).put("id", "odd")
            .put("dappSummary", JSONObject().put("action", "teleport"))
            .put("balanceChanges", JSONArray().put(JSONObject().put("type", "quantum").put("delta", "1")))
        val send = JSONObject().put("id", "send").put("type", "send").put("timestamp", 1_790_000_000)
            .put("intent", "Swap").put("dappUrl", "https://app.uniswap.org").put("chainId", 1)
        store.write(KeyValueStore.Keys.TRANSACTIONS, JSONArray().put(swap).put(legacy).put(odd).put(send).toString())
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })

        val records = (feed.perform(FeedOperation.ReadTxStore(ME, 1)) as FeedShellResult.StoreLoaded).records.associateBy { it.id }
        val read = records.getValue(swap.getString("id"))
        assertEquals(summary, read.summary)
        assertEquals(judgments, read.balance_changes)
        assertEquals("Swap", read.intent)
        assertEquals(SITE, read.dapp_url)

        assertEquals(FeedTxKind.SignMessage, records.getValue("legacy-sig").kind)
        assertEquals("an old record's origin", SITE, records.getValue("legacy-sig").dapp_url)

        assertNull("a summary this build cannot read is absent", records.getValue("odd").summary)
        assertNull("judgments are all kept or none", records.getValue("odd").balance_changes)
        assertEquals("the record itself stays", "Swap", records.getValue("odd").intent)

        assertNull("only a dApp record carries them", records.getValue("send").intent)
        assertNull(records.getValue("send").dapp_url)

        assertEquals(swap.getString("signedRequest"), feed.storedRequest(swap.getString("id")))
        assertNull(feed.storedRequest("nobody"))
    }

    // -- the rows and the detail, on the real feed ---------------------------------------

    /**
     * The three fixture rows — a swap on Uniswap, a Permit2 permit and a
     * sign-in, the last two signed by the real sign machine — through the
     * real feed: each title is the core's verb "on" the core's place, the
     * second line its parts, the right column the money (≈, with the coin
     * back beside it) or the allowance (unlimited, in the danger tone).
     */
    @Test
    fun `the three fixture rows read in the core's words, in English and Chinese`() = runBlocking {
        val (view, _) = fixtureFeed()
        val enRows = rowsById(view, en)
        val swap = enRows.getValue("dapp-swap")
        assertEquals("Swap on Uniswap", swap.title)
        assertEquals("app.uniswap.org · Ethereum", swap.subtitle)
        assertEquals("≈ −100", swap.amount)
        assertEquals("USDC", swap.unit)
        assertEquals("≈ +0.03 ETH", swap.received)
        assertFalse(swap.danger)

        val permit = enRows.values.single { it.title.startsWith(en.t("componentsUi.signing.permitIntent")) }
        assertEquals("Spending permit on Uniswap", permit.title)
        assertEquals("app.uniswap.org · Ethereum", permit.subtitle)
        assertEquals("Unlimited", permit.amount)
        assertEquals("USDC", permit.unit)
        assertTrue("an unlimited allowance is the danger tone", permit.danger)

        val siwe = enRows.values.single { it.title.startsWith(en.t("componentsUi.signing.signInIntent")) }
        assertEquals("Sign in on app.uniswap.org", siwe.title)
        assertEquals("the place is the site, said once", "Ethereum", siwe.subtitle)
        assertFalse("a sign-in moved nothing", siwe.hasFigure)

        val zhTitles = rowsById(view, zh).values.map { it.title }.toSet()
        assertEquals(setOf("在 Uniswap 兑换", "在 Uniswap 授权签名", "在 app.uniswap.org 登录"), zhTitles)
        assertTrue(rowsById(view, zh).values.any { it.amount == "无限额" && it.danger })
    }

    /**
     * The permit, opened: no status chip (nothing settles a signature) and the
     * off-chain note instead; the core's facts in its order — site, network,
     * spender by name, the unlimited cap in the danger tone, "never expires",
     * the date; and folded "Technical details": the operation, the stored
     * request (read by id only when opened), the typed data's type. The swap
     * keeps its chip, its contract, its balance changes, and its hashes.
     */
    @Test
    fun `the permit's detail is the core's facts, and its technical details hold the stored request`() = runBlocking {
        val (view, feed) = fixtureFeed()
        val permitId = view.rows.filterIsInstance<FeedRow.Item>().map { it.item }.single { it.kind == FeedTxKind.SignTypedData }.id
        val fallback = (FlowFixtures.build(FlowState.A2, en).sheet as FlowSheet.TxDetail).model
        val detail = FlowLive.txDetail(fallback, view, permitId, en, chains, mapOf(1 to "https://etherscan.io"))!!

        assertNull("a signature has no status chip", detail.status)
        assertEquals(en.t(I18nKeys.Flows.OFF_CHAIN_NOTE), detail.note)
        assertEquals("Spending permit on Uniswap", detail.title)
        assertEquals("Unlimited USDC", detail.amount)
        assertTrue(detail.amountDanger)
        assertFalse("no transaction, no explorer", detail.explorerShown)
        assertEquals(
            listOf(I18nKeys.Flows.DETAIL_APP, I18nKeys.Flows.DETAIL_CHAIN, I18nKeys.Flows.DETAIL_SPENDER, I18nKeys.Flows.SPENDING_CAP, I18nKeys.Flows.EXPIRES, I18nKeys.Flows.DETAIL_DATE).map(en::t),
            detail.facts.map { it.label },
        )
        assertEquals("app.uniswap.org", detail.facts[0].value)
        assertEquals("Ethereum", detail.facts[1].value)
        assertEquals("Uniswap Universal Router", detail.facts[2].value)
        assertEquals(ROUTER, detail.facts[2].copyValue)
        assertEquals("Unlimited USDC", detail.facts[3].value)
        assertTrue(detail.facts[3].danger)
        assertEquals(en.t(I18nKeys.Flows.NO_EXPIRY), detail.facts[4].value)

        val technical = detail.technical!!
        assertEquals(en.t(I18nKeys.Flows.TECHNICAL), technical.title)
        val operation = technical.lines[0] as TxTechnicalLine.Fact
        assertEquals(en.t(I18nKeys.Flows.OPERATION), operation.fact.label)
        assertEquals(en.t(I18nKeys.Flows.OP_TYPED_DATA), operation.fact.value)
        val content = technical.lines[1] as TxTechnicalLine.Content
        assertEquals(en.t(I18nKeys.Flows.CONTENT_TYPED_DATA), content.label)
        assertEquals(permitId, content.recordId)
        assertEquals(en.t(I18nKeys.Flows.CONTENT_MISSING), content.missing)
        assertEquals("PermitSingle", (technical.lines[2] as TxTechnicalLine.Fact).fact.value)
        assertEquals(3, technical.lines.size)
        // What opening the section reads: the request as the core kept it.
        assertEquals(requestOf(permitParams()), requestOf(feed.storedRequest(content.recordId)!!))

        val swap = FlowLive.txDetail(fallback, view, "dapp-swap", en, chains, mapOf(1 to "https://etherscan.io"))!!
        assertEquals(en.t(I18nKeys.Flows.STATUS_CONFIRMED), swap.status?.text)
        assertNull(swap.note)
        assertEquals("Swap on Uniswap", swap.title)
        assertEquals("≈ −100 USDC", swap.amount)
        assertEquals("≈ +0.03 ETH", swap.received)
        val changes = swap.facts.single { it.label == en.t(I18nKeys.Flows.BALANCE_CHANGES) }
        assertEquals(listOf("≈ −100 USDC", "≈ +0.03 ETH"), listOf(changes.value) + changes.lines)
        assertEquals("Uniswap Universal Router", swap.facts.single { it.label == en.t(I18nKeys.Flows.DETAIL_CONTRACT) }.value)
        assertEquals("https://etherscan.io/tx/$SWAP_TX", swap.explorerUrl)
        val swapTechnical = swap.technical!!.lines.filterIsInstance<TxTechnicalLine.Fact>().map { it.fact }
        assertEquals(SWAP_TX, swapTechnical.single { it.label == en.t(I18nKeys.Flows.DETAIL_HASH) }.copyValue)
        assertEquals(SWAP_OP, swapTechnical.single { it.label == en.t(I18nKeys.Flows.USER_OP_HASH) }.copyValue)
    }

    // -- fail-soft decoding, and the words --------------------------------------------

    /**
     * A newer core may add a second-line part or a detail line this build
     * does not know: that part is left out, the row and the rest of its
     * detail stay; a dApp payload that does not read at all is absent, and
     * the row still draws.
     */
    @Test
    fun `an unknown part of a row is left out, never the row`() {
        val item = Wire.json.decodeFromString(
            FeedItem.serializer(),
            """
            {"id":"x","direction":"out","counterparty":null,"alias":null,"value":null,"symbol":"","decimals":null,
             "usd_value":0,"chain_id":1,"timestamp":1790000000,"day_start_ms":1789948800000,"tx_hash":null,"batch":null,
             "kind":"sign_message","status":"confirmed",
             "subtitle":[{"type":"weather"},{"type":"network","chain_id":1}],
             "dapp":{"site":"app.uniswap.org","intent":null,"intent_term":"signInIntent","place":"app.uniswap.org","off_chain":true,
                     "facts":[{"type":"site","site":"app.uniswap.org"},{"type":"horoscope"}],
                     "technical":[{"type":"operation","operation":{"type":"quantum"}},{"type":"content","content":"message"}]}}
            """.trimIndent(),
        )
        assertEquals(listOf<FeedLine>(FeedLine.Network(1)), item.subtitle)
        assertEquals(listOf<FeedFact>(FeedFact.Site("app.uniswap.org")), item.dapp?.facts)
        assertEquals(1, item.dapp?.technical?.size)

        val unreadable = Wire.json.decodeFromString(
            FeedItem.serializer(),
            """{"id":"y","direction":"out","chain_id":1,"timestamp":1,"day_start_ms":0,"kind":"dapp_tx","dapp":{"off_chain":"maybe"}}""",
        )
        assertNull(unreadable.dapp)
        val row = WalletLive.activity(FeedView(rows = listOf(FeedRow.Item(unreadable))), en).single().rows.single()
        assertEquals(en.t(I18nKeys.Wallet.LABEL_DAPP_TX), row.title)
    }

    /** The one new key and every reused key resolve in English and Chinese (spec 093 FR-006). */
    @Test
    fun `every word the dApp rows use is in the corpus`() {
        val keys = listOf(
            I18nKeys.Wallet.DAPP_ROW_TITLE, I18nKeys.Wallet.UNLIMITED, I18nKeys.Wallet.TO_NAME, I18nKeys.Wallet.FROM_NAME,
            I18nKeys.Flows.OFF_CHAIN_NOTE, I18nKeys.Flows.DETAIL_APP, I18nKeys.Flows.DETAIL_SPENDER, I18nKeys.Flows.SPENDING_CAP,
            I18nKeys.Flows.EXPIRES, I18nKeys.Flows.NO_EXPIRY, I18nKeys.Flows.UNLIMITED, I18nKeys.Flows.BALANCE_CHANGES,
            I18nKeys.Flows.UNVERIFIED_TOKEN, I18nKeys.Flows.TECHNICAL, I18nKeys.Flows.OPERATION, I18nKeys.Flows.OP_CONTRACT,
            I18nKeys.Flows.OP_BATCH, I18nKeys.Flows.OP_SIGNATURE, I18nKeys.Flows.OP_TYPED_DATA, I18nKeys.Flows.CONTENT_CALL_DATA,
            I18nKeys.Flows.CONTENT_TYPED_DATA, I18nKeys.Flows.CONTENT_MESSAGE, I18nKeys.Flows.CONTENT_MISSING, I18nKeys.Flows.TYPE,
            I18nKeys.Flows.USER_OP_HASH,
        ) + listOf("permitIntent", "signInIntent", "messageIntent", "typedDataIntent", "ethSignIntent", "batchIntent", "intentContractCall")
            .map { I18nKeys.Wallet.SIGNING_TERM_PREFIX + it }
        for (strings in listOf(en, zh)) {
            for (key in keys) assertNotEquals("$key resolves", key, strings.t(key))
        }
    }

    // -- helpers -------------------------------------------------------------------

    /** A request as data — the typed-data document inside it too — so key order is not compared. */
    private fun requestOf(params: String): kotlinx.serialization.json.JsonElement {
        fun open(element: kotlinx.serialization.json.JsonElement): kotlinx.serialization.json.JsonElement = when (element) {
            is kotlinx.serialization.json.JsonArray -> kotlinx.serialization.json.JsonArray(element.map(::open))
            is kotlinx.serialization.json.JsonObject -> kotlinx.serialization.json.JsonObject(element.mapValues { open(it.value) })
            is kotlinx.serialization.json.JsonPrimitive -> element.takeIf { it.isString && it.content.trimStart().startsWith("{") }
                ?.let { runCatching { open(Wire.json.parseToJsonElement(it.content)) }.getOrNull() } ?: element
            else -> element
        }
        return open(Wire.json.parseToJsonElement(params))
    }

    private fun rowsById(view: FeedView, strings: VelaStrings): Map<String, ActivityRowModel> =
        WalletLive.activity(view, strings, chainNames = chains).flatMap { it.rows }.associateBy { it.id }

    /** The swap's record, as the sign machine writes one for a transaction that landed. */
    private fun swapRecord() = SignRecord(
        record_id = "dapp-swap", kind = SignRecordKind.DappTx, method = "eth_sendTransaction",
        params_json = SWAP_PARAMS, result = SWAP_TX, from = ME, chain_id = 1,
        now_ms = System.currentTimeMillis().toDouble() - 120_000, status = SignRecordStatus.Confirmed,
        user_op_hash = SWAP_OP, dapp_origin = SITE, dapp_url = SITE, intent = "Swap",
        balance_changes = listOf(
            TrustSimJudgment.Erc20Trusted(token = USDC, delta = "-100000000", symbol = "USDC", decimals = 6, in_trusted_set = true),
            TrustSimJudgment.Native(delta = "30000000000000000"),
        ),
        summary = DappSummary(action = DappAction.Call, calls = 1, contract = ROUTER),
        stored_request = SWAP_PARAMS,
    )

    /** The three fixtures on the real feed: the swap's record, and a permit and a sign-in the real sign machine signed. */
    private suspend fun fixtureFeed(): Pair<FeedView, FeedExecutor> {
        val usdc = GuardTokenMetaView(symbol = "USDC", decimals = 6, verified = true, loading = false)
        val permit = signedByCore("eth_signTypedData_v4", permitParams(), SignApproveOpts(token_meta = usdc, unlimited_approved = true))
        val siwe = signedByCore("personal_sign", siweParams(), SignApproveOpts())
        val store = FakeStore()
        val feed = FeedExecutor(store = store, ownAccounts = { emptyList() })
        assertTrue(feed.writeRecords(listOf(swapRecord(), permit, siwe).map { SignExecutor.recordRow(it, "ETH") }))
        val host = CoreHost(
            bridge = ActivityFeedCore().asBridge(),
            scope = scope,
            initial = FeedView(),
            serializer = FeedView.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), feed::perform),
            escapedFailure = JsonShell.escapedFailure(FeedOperation.serializer(), FeedShellResult.serializer(), fallback = FeedShellResult.HapticPlayed, answer = feed::neutralAnswer),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
        host.dispatch(FeedEvent.AccountSwitched(ME), FeedEvent.serializer())
        host.dispatch(FeedEvent.ReconcileCompleted(1), FeedEvent.serializer())
        val view = withTimeout(20_000) {
            host.view.first { view -> view.rows.count { it is FeedRow.Item } == 3 }
        }
        return view to feed
    }

    /**
     * One request through the REAL `sign_request` machine to its record: it
     * arrives from the site, is approved with [opts], is signed (the shell's
     * answer: a signature), and the machine writes its record — returned as
     * the machine wrote it.
     */
    private suspend fun signedByCore(method: String, params: String, opts: SignApproveOpts): SignRecord {
        val persisted = kotlinx.coroutines.CompletableDeferred<SignRecord>()
        val sign = CoreHost(
            bridge = SignRequestCore().asBridge(),
            scope = scope,
            initial = SignView(),
            serializer = SignView.serializer(),
            perform = JsonShell.perform(SignOperation.serializer(), SignShellResult.serializer()) { op -> answer(op, persisted) },
            escapedFailure = JsonShell.escapedFailure(SignOperation.serializer(), SignShellResult.serializer(), fallback = SignShellResult.Responded) { op -> answer(op, persisted) },
            onFault = { error -> throw AssertionError("sign fault: $error", error) },
        )
        sign.dispatch(SignEvent.NetworksChanged(listOf(1)), SignEvent.serializer())
        sign.dispatch(SignEvent.AccountsChanged(listOf(SignAccountRef(ME, "cred-1")), 0), SignEvent.serializer())
        sign.dispatch(
            SignEvent.RequestArrived(
                id = "req-$method", method = method, params_json = params, origin = SITE, transport_id = "tab-1",
                dedicated_transport = true, per_request_chain = 1, granted_address = ME, now_ms = System.currentTimeMillis().toDouble(),
            ),
            SignEvent.serializer(),
        )
        withTimeout(10_000) { sign.view.first { it.surface == SignSurface.Sheet && !it.reconcile_pending } }
        sign.dispatch(SignEvent.ApproveTapped(opts), SignEvent.serializer())
        return withTimeout(20_000) { persisted.await() }
    }

    private fun answer(op: SignOperation, persisted: kotlinx.coroutines.CompletableDeferred<SignRecord>): SignShellResult = when (op) {
        is SignOperation.SendResponse -> SignShellResult.Responded
        is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
        is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(SignSponsorship.Denied(null))
        // The ceremony's answer: a signature, which the record must not keep.
        is SignOperation.SignAndSubmit -> SignShellResult.Submit(SignSubmitOutcome.Succeeded("0x" + "5a".repeat(65)), System.currentTimeMillis().toDouble())
        is SignOperation.PersistRecord -> {
            persisted.complete(op.record)
            SignShellResult.RecordPersisted
        }
        is SignOperation.UpdateRecord -> SignShellResult.RecordUpdated
        is SignOperation.ClearToPost -> SignShellResult.Responded
        is SignOperation.DeleteRecord -> SignShellResult.RecordUpdated
        is SignOperation.SwitchActiveAccount -> SignShellResult.AccountSwitched
    }

    private companion object {
        const val ME = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val ROUTER = "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad"
        const val PERMIT2 = "0x000000000022d473030f116ddee9f6b43ac78ba3"
        const val USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        const val SITE = "https://app.uniswap.org"
        val SWAP_TX = "0x" + "c4".repeat(32)
        val SWAP_OP = "0x" + "0b".repeat(32)
        val SWAP_PARAMS = """[{"to":"$ROUTER","value":"0x0","data":"0x3593564c0000000000000000000000000000000000000000000000000000000000000060"}]"""

        /** A Permit2 `PermitSingle` for USDC, unlimited and never expiring, to the Universal Router. */
        fun permitParams(): String {
            val typed = JSONObject()
                .put(
                    "types",
                    JSONObject()
                        .put("EIP712Domain", fields("name" to "string", "chainId" to "uint256", "verifyingContract" to "address"))
                        .put("PermitSingle", fields("details" to "PermitDetails", "spender" to "address", "sigDeadline" to "uint256"))
                        .put("PermitDetails", fields("token" to "address", "amount" to "uint160", "expiration" to "uint48", "nonce" to "uint48")),
                )
                .put("primaryType", "PermitSingle")
                .put("domain", JSONObject().put("name", "Permit2").put("chainId", "1").put("verifyingContract", PERMIT2))
                .put(
                    "message",
                    JSONObject()
                        .put(
                            "details",
                            JSONObject()
                                .put("token", USDC)
                                .put("amount", "1461501637330902918203684832716283019655932542975")
                                .put("expiration", "281474976710655")
                                .put("nonce", "0"),
                        )
                        .put("spender", ROUTER)
                        .put("sigDeadline", "1900000000"),
                )
            return JSONArray().put(ME).put(typed.toString()).toString()
        }

        fun siweParams(): String {
            val message = listOf(
                "app.uniswap.org wants you to sign in with your Ethereum account:",
                ME,
                "",
                "Sign in to Uniswap",
                "",
                "URI: https://app.uniswap.org",
                "Version: 1",
                "Chain ID: 1",
                "Nonce: 32891756",
                "Issued At: 2026-10-02T10:00:00.000Z",
            ).joinToString("\n")
            return JSONArray().put(message).put(ME).toString()
        }

        private fun fields(vararg pairs: Pair<String, String>) =
            JSONArray().apply { pairs.forEach { (name, type) -> put(JSONObject().put("name", name).put("type", type)) } }
    }
}
