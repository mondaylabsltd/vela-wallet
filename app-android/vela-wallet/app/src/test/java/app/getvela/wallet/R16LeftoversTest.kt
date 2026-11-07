package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.HistoryMode
import app.getvela.wallet.feature.multisig.LinkedStands
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsLinkedView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.TrustView
import java.io.File
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 16's leftovers, adopted on Android (core commit "every press over no
 * figure signs nothing; busy on the sheet; failures are kinds; Activity names
 * unread networks; one "Updated" answer"):
 *
 * - 3: a rate-limited multisig service is said busy on the proposal sheet
 *   (`ProposalsView.busy`, `review.missing {busy}`, a link's `busyNote`),
 *   never "Couldn't reach the multisig service".
 * - 4: a failed multisig action is a stable KIND (`review.failure`), worded
 *   from the catalog — never a detail's or a message's English.
 * - 5: Activity says which networks its scan could not read
 *   (`FeedView.unread_key`), over the rows or in the empty state's place.
 * - 6: "Updated <ago>" is the core's one answer (`BalanceView.updated_at_ms`).
 */
class R16LeftoversTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val vectors = File(repoRoot, "rust/crates/vela-core/tests/vectors/multisig/review-state.json")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private val words = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    // -- item 3: a busy multisig service on the sheet ---------------------------------------

    @Test
    fun `the missing vectors hold through the link door, busy included`() {
        val cases = json(vectors.readText())["missing"] as JsonArray
        assertTrue(cases.any { it.jsonObject["args"]!!.jsonObject["busy"] == JsonPrimitive(true) })
        for (case in cases) {
            val name = case.jsonObject["name"].toString()
            val args = case.jsonObject["args"]!!.jsonObject
            val expect = case.jsonObject["expect"]!!.jsonObject
            assertEquals(name, expect, MsRules.call("review.missing", args))
            val mailbox = args["mailbox"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content?.toBooleanStrict()
            val said = MsRules.missingSaid(
                loading = args["loading"]!!.jsonPrimitive.content.toBooleanStrict(),
                unreadable = args["unreadable"]!!.jsonPrimitive.content.toBooleanStrict(),
                mailbox = mailbox,
                busy = args["busy"]?.jsonPrimitive?.content?.toBooleanStrict() ?: false,
            )
            assertEquals(name, expect["key"]!!.jsonPrimitive.content, said.key)
            assertEquals(name, expect["retry"]!!.jsonPrimitive.content.toBooleanStrict(), said.retry)
        }
    }

    private fun proposals(busy: Boolean, mailbox: Boolean = false, linked: String? = null) = MsProposalsView.of(
        json(
            """{"followed":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","busy":$busy,"loading":false,"unreadable":[],
               "chains":[{"chainId":100,"members":null,"items":[],"mailbox":$mailbox}]${linked?.let { ""","linked":$it""" } ?: ""}}""",
        ),
    )

    @Test
    fun `the proposals view decodes busy`() {
        assertTrue(proposals(busy = true).busy)
        assertFalse(proposals(busy = false).busy)
        assertFalse(MsProposalsView.of(json("{}")).busy)
    }

    @Test
    fun `a sheet opened without a link over a busy service says busy with Try again, never couldn't reach`() {
        val busy = LinkedStands.missing(proposals(busy = true), 100, words)!!
        assertEquals(words.t("service.busyNote"), busy.text)
        assertTrue(busy.retry)
        assertNotEquals(words.t("proposal.unread.linkService", "network" to "Gnosis"), busy.text)
        assertFalse("no {{network}} left unfilled", busy.text.contains("{{"))
        // The service silent (not busy): said as the service, with Try again.
        val silent = LinkedStands.missing(proposals(busy = false), 100, words)!!
        assertEquals(words.t("proposal.unread.linkService", "network" to "Gnosis"), silent.text)
        assertTrue(silent.retry)
        // Read, and not there: not in the mailbox, final.
        val read = LinkedStands.missing(proposals(busy = true, mailbox = true), 100, words)!!
        assertEquals(words.t("proposal.notInMailbox"), read.text)
        assertFalse(read.retry)
        // A network this multisig is not read on: not here, nothing to press.
        val notHere = LinkedStands.missing(proposals(busy = false), 1, words)!!
        assertEquals(words.t("proposal.notHere", "network" to words.network(1)), notHere.text)
        assertFalse(notHere.retry)
        // Still loading: nothing said but "Opening…" (the spinner).
        assertNull(LinkedStands.missing(MsProposalsView.of(json("""{"loading":true}""")), 100, words))
        // A sheet opened from a link says the link's stand, not this.
        val link = """{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"reading"}"""
        assertNull(LinkedStands.missing(proposals(busy = true, linked = link), 100, words))
    }

    @Test
    fun `a link whose service is busy says busy, and any core key is worded whole`() {
        val linked = MsLinkedView.of(
            json("""{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"notFound","reason_key":"home.multisig.service.busyNote","retry":true}"""),
        )
        assertEquals(words.t("service.busyNote"), LinkedStands.failure(linked, words))
        // A key outside home.multisig is the catalog's own, never prefixed again.
        assertEquals(words.raw("componentsUi.signing.signerDown"), words.unreadText("componentsUi.signing.signerDown", 100))
        assertEquals(words.t("proposal.unread.chain", "network" to "Gnosis"), words.unreadText(null, 100))
    }

    /**
     * On the real proposals machine in the simulated world: the mailbox
     * answers its rate limit (429) — the view says `busy`, and a sheet opened
     * without a link over a transaction not in the unread list says busy with
     * Try again, never "Couldn't reach the multisig service".
     */
    @Test
    fun `a rate limited mailbox is said busy on a sheet opened without a link`() {
        val world = app.getvela.wallet.MultisigWorld { name -> File(repoRoot, "rust/crates/vela-core/tests/vectors/safe-code/$name.hex").readText().trim() }
        world.locals += listOf(0, 1)
        val owners = (0..2).map(world::address)
        val g = world.genesis(owners, 2, "Quiet Harbor", "0x61")
        val safe = g["address"]!!.jsonPrimitive.content
        world.deploy(100, safe, app.getvela.wallet.MultisigWorld.Safe(owners, 2))
        world.store["vela.multisigAccounts"] = "[${world.storedRow(g, listOf(100))}]"
        val scope = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.Default)
        val hub = app.getvela.wallet.feature.multisig.core.MultisigHub(
            live = world,
            facts = { app.getvela.wallet.feature.multisig.core.MultisigHub.Facts(app.getvela.wallet.MultisigWorld.SERVICE, world.localAccounts(), listOf(100), listOf(100), emptyList()) },
            scope = scope,
        )
        try {
            kotlinx.coroutines.runBlocking {
                kotlinx.coroutines.withTimeout(120_000) {
                    hub.boot()
                    hub.activate(safe)
                    while (hub.proposalsView.value.followed == null) kotlinx.coroutines.delay(25)
                    world.serviceBusy = true
                    hub.sendProposal(app.getvela.wallet.feature.multisig.core.ev("retry"))
                    while (!hub.proposalsView.value.busy) kotlinx.coroutines.delay(25)
                }
            }
            val view = hub.proposalsView.value
            assertTrue(view.busy)
            val said = LinkedStands.missing(view, 100, words)
            assertNotNull("the core says why: $view", said)
            assertEquals(words.t("service.busyNote"), said!!.text)
            assertTrue(said.retry)
            assertNotEquals(words.t("proposal.unread.linkService", "network" to "Gnosis"), said.text)
        } finally {
            hub.dispose()
            scope.coroutineContext[kotlinx.coroutines.Job]?.cancel()
        }
    }

    @Test
    fun `the sheet draws the core's missing line with its Try again`() {
        val sheet = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigProposalSheet.kt").readText()
        assertTrue(sheet.contains("LinkedStands.missing(view, chainId, words)"))
        assertTrue(sheet.contains("multisig.proposal.missing\""))
        assertTrue(sheet.contains("multisig.proposal.missingRetry\""))
    }

    // -- item 4: failures are kinds ------------------------------------------------------

    @Test
    fun `the failure vectors hold through the link door`() {
        val cases = json(vectors.readText())["failure"] as JsonArray
        assertTrue(cases.size >= 15)
        for (case in cases) {
            val name = case.jsonObject["name"].toString()
            val args = case.jsonObject["args"]!!.jsonObject
            val expect = case.jsonObject["expect"]!!.jsonObject
            assertEquals(name, expect, MsRules.call("review.failure", args))
            val said = MsRules.failureSaid(args["detail"]!!.jsonPrimitive.content)
            assertEquals(name, expect["kind"]!!.jsonPrimitive.content, said.kind)
            assertEquals(name, expect["key"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content, said.key)
        }
    }

    @Test
    fun `every failure is worded by its kind, never by its text`() {
        val unknown = words.t("proposal.reason.unknown")
        val cases = json(vectors.readText())["failure"] as JsonArray
        for (case in cases) {
            val name = case.jsonObject["name"].toString()
            val detail = case.jsonObject["args"]!!.jsonObject["detail"]!!.jsonPrimitive.content
            val text = words.failureText(detail, 100)
            assertTrue(name, text.isNotBlank())
            assertFalse(name, text.contains("{{"))
            // The key the core named is the words (fee_moved, cannot_pay and
            // venue_blocked are worded from their own facts).
            val key = case.jsonObject["expect"]!!.jsonObject["key"]?.takeUnless { it is JsonNull }?.jsonPrimitive?.content
            if (key != null) assertEquals(name, words.raw(key, "network" to "Gnosis"), text)
        }
        // Plain text, an older core's English, a shell's message: never drawn.
        assertEquals(unknown, words.failureText("boom"))
        assertEquals(unknown, words.failureText("the passkey's signature did not check out: bad s"))
        assertEquals(unknown, words.failureText(MultisigShellFailure.Unknown("TypeError: x is undefined").detail))
        assertEquals(unknown, words.failureText("""{"kind":"unknown","reason":"cannot_execute_now"}"""))
        assertEquals(unknown, words.failureText(null))
        assertFalse(words.failureText(MultisigShellFailure.Unknown("The relay is unavailable right now.").detail).contains("relay"))
        // A passkey failure's English is never words (a catalog refinement may be).
        val other = words.failureText(MultisigShellFailure.Passkey("other", message = "NotAllowedError: blah").detail)
        assertFalse(other.contains("NotAllowedError"))
        assertFalse(other.contains("blah"))
        // The core's own check is the passkey's failure, in words.
        assertEquals(words.t("proposal.reason.passkeyFailed"), words.failureText("""{"kind":"passkey_check"}"""))
        // A network failing before the passkey names it.
        assertEquals(
            words.t("proposal.reason.chainUnreachable", "network" to "Gnosis"),
            words.failureText("""{"kind":"chain_unreachable","chain_id":100}"""),
        )
        // The action's failed state reads the same way.
        val failed = json("""{"kind":"failed","detail":"Error: relay said 500"}""")
        assertEquals(unknown, words.actionFailure(failed, 100))
    }

    @Test
    fun `the create sheet never draws the core's or the service's text`() {
        // (a) the draft the core could not make.
        assertEquals(words.t("proposal.reason.unknown"), words.createDraftFailed)
        // (b) the service's refusal: what is true and what to do.
        assertEquals(words.t("settings.shareAgainNote"), words.createRefused)
        val sheet = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigAccountSheets.kt").readText()
        assertFalse("never the core's draft_error", sheet.contains("MsNote(sheet.draftError"))
        assertFalse("never the service's refusal text", sheet.contains("\"reason\" to sheet.refused"))
        assertTrue(sheet.contains("words.createDraftFailed"))
        assertTrue(sheet.contains("words.createRefused"))
    }

    @Test
    fun `a refused draft is said in the catalog's words, a member refusal by its kind`() {
        // (c) the core's English (a review's refusal, an encoding error): never drawn.
        val other = words.t("review.refusedWhy.other")
        assertEquals(other, words.actionFailure(json("""{"kind":"refused","reason":"the transaction does not decode: bad data"}"""), 100))
        assertEquals(other, words.actionFailure(json("""{"kind":"refused","reason":""}"""), 100))
        assertEquals(other, words.refusedText(null))
        assertEquals(other, words.refusedText("""{"kind":"notAnAddress","address":"0x1"}"""))
        // A member change's refusal is the core's JSON, worded by its kind.
        assertEquals(words.t("members.refuse.alreadyMember"), words.refusedText("""{"kind":"alreadyMember","owner":"0x1"}"""))
        assertEquals(
            words.t("members.refuse.thresholdTooHigh", "owners" to "2", "threshold" to "3"),
            words.refusedText("""{"kind":"thresholdTooHigh","threshold":3,"owners":2}"""),
        )
        assertEquals(words.t("members.refuse.changeWaiting"), words.actionFailure(json("""{"kind":"refused","reason":"{\"kind\":\"changeWaiting\"}"}"""), 100))
    }

    @Test
    fun `the outside sheet's relay line says the catalog's reason or the relay's host, never transport text`() {
        fun relay(kind: String, why: String?, detail: String?) =
            json("""{"kind":"$kind","why":${why?.let { "\"$it\"" } ?: "null"},"detail":${detail?.let { Wire.json.encodeToString(kotlinx.serialization.json.JsonPrimitive.serializer(), JsonPrimitive(it)) } ?: "null"}}""")
        // (d) refused: the catalog's refusal.
        assertEquals(
            words.t("external.relayFailed", "reason" to words.t("external.relayRefused")),
            words.relayLine(relay("failed", "refused", "HTTP 4403 forbidden")),
        )
        // Unreached: the relay's host, when the detail is one.
        assertEquals(
            words.t("external.relayUnreachable", "reason" to "relay.getvela.app:443"),
            words.relayLine(relay("retrying", "unreachable", "relay.getvela.app:443")),
        )
        // A transport error's text is no host: nothing sayable, Vela reconnecting.
        val transport = words.relayLine(relay("retrying", "unreachable", "java.net.SocketTimeoutException: timeout"))
        assertEquals(words.t("external.velaOffline"), transport)
        assertEquals(words.t("external.velaOffline"), words.relayLine(relay("failed", "unreachable", null)))
        // A pass not got (and any other why): the catalog's.
        assertEquals(
            words.t("external.relayFailed", "reason" to words.t("external.relayNoToken")),
            words.relayLine(relay("failed", "noToken", "Error: 500 Internal Server Error")),
        )
        // Not retrying or failed: no line.
        assertNull(words.relayLine(relay("connected", null, null)))
        assertNull(words.relayLine(null))
        // A room not opened: its transport text never drawn.
        val noRoom = words.outsideFailure(json("""{"kind":"noRoom","reason":"WebSocket closed: 1006"}"""), chain = "Gnosis", wallet = "MetaMask")
        assertEquals(words.t("proposal.reason.unknown"), noRoom)
        assertFalse(noRoom.contains("1006"))
        val host = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigHost.kt").readText()
        assertTrue(host.contains("words.relayLine(view[\"relay\"])"))
        assertTrue(host.contains("words.outsideFailure(f,"))
    }

    @Test
    fun `no multisig words read a failure's message`() {
        val source = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/multisig/MultisigWords.kt").readText()
        val failureText = source.substringAfter("fun failureText(").substringBefore("// -- authority findings")
        assertFalse("never a failure's message as words", failureText.contains("\"reason\" to it"))
        assertFalse("never the detail as words", failureText.contains("\"reason\" to detail"))
        assertTrue("worded by the core's kind", failureText.contains("MsRules.failureSaid("))
    }

    // -- item 5: Activity names the networks it could not read -----------------------------

    private val account = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private val chainNames = mapOf(137 to "Polygon", 100 to "Gnosis")

    private fun received(id: String) = FeedRow.Item(
        FeedItem(
            id = id, direction = FeedDirection.In, counterparty = "0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D",
            value = "0.3", symbol = "USDC", decimals = 6, usd_value = 0.3, chain_id = 100,
            timestamp = 1_790_000_000.0, day_start_ms = 1_789_948_800_000.0,
        ),
    )

    private fun feed(unread: List<Int>, rows: List<FeedRow> = emptyList(), settled: Boolean = unread.isEmpty()): FeedView {
        val all = if (rows.isEmpty()) rows else listOf(FeedRow.Header(id = "day-1", day_start_ms = 1_789_948_800_000.0, timestamp = 1_790_000_000.0)) + rows
        val key = when (unread.size) {
            0 -> null
            1 -> "home.activityUnreadOne"
            else -> "home.activityUnreadMany"
        }
        return FeedView(rows = all, home_rows = all, account = account, settled = settled, unread_chains = unread, unread_key = key)
    }

    private fun home(feed: FeedView) = WalletLive.home(
        WalletFixtures.buildMobileState(WalletScreenState.H1, strings),
        BalanceView(address = account),
        feed,
        CurrencyView(code = "USD", committed = true),
        strings,
        chainNames,
        shownAccount = account,
    )

    @Test
    fun `the feed and trust views and the scan's answer carry the unread networks`() {
        val view = Wire.json.decodeFromString(FeedView.serializer(), """{"unread_chains":[137],"unread_key":"home.activityUnreadOne","settled":false}""")
        assertEquals(listOf(137), view.unread_chains)
        assertEquals("home.activityUnreadOne", view.unread_key)
        assertEquals(listOf(100, 137), Wire.json.decodeFromString(TrustView.serializer(), """{"unread_chains":[100,137]}""").unread_chains)
        val answer = Wire.json.encodeToString(FeedShellResult.serializer(), FeedShellResult.SyncCompleted(new_count = 2, unread_chains = listOf(137)))
        assertTrue(answer, answer.contains("\"unread_chains\":[137]"))
        // A view from a core before it: nothing unread.
        assertEquals(emptyList<Int>(), Wire.json.decodeFromString(FeedView.serializer(), "{}").unread_chains)
        assertNull(Wire.json.decodeFromString(FeedView.serializer(), "{}").unread_key)
    }

    @Test
    fun `an Activity with no rows and an unread network says so in the empty state's place`() {
        val one = home(feed(listOf(137)))
        assertEquals("not the skeleton, not 'No activity yet'", SectionMode.Empty, one.activitySection.mode)
        assertEquals("Activity can't be read on Polygon right now.", one.activitySection.empty?.title)
        assertEquals(strings.t("home.activityUnreadOne", mapOf("network" to "Polygon")), one.activitySection.empty?.title)
        assertNotEquals(strings.t("home.emptyNoActivity"), one.activitySection.empty?.title)
        assertNull("no line over rows that are not there", one.activitySection.note)
        val many = home(feed(listOf(100, 137)))
        assertEquals(SectionMode.Empty, many.activitySection.mode)
        assertEquals(strings.t("home.activityUnreadMany", mapOf("n" to "2")), many.activitySection.empty?.title)
        assertTrue(many.activitySection.empty!!.title.contains("2"))
        // r17-4: a network neither this space's list nor the core's catalog
        // names (Arc is the catalog's: "Arc Testnet"): its id, never a blank.
        val catalogued = home(feed(listOf(5042002)))
        assertFalse(catalogued.activitySection.empty!!.title.contains("5042002"))
        val unnamed = home(feed(listOf(987_654_321)))
        assertTrue(unnamed.activitySection.empty!!.title.contains("987654321"))
        // Every network read and nothing there: the core's own empty line.
        assertEquals(strings.t("home.emptyNoActivity"), home(feed(emptyList())).activitySection.empty?.title)
        // Another account's feed is never said under this one.
        assertEquals(SectionMode.Loading, home(feed(listOf(137)).copy(account = "0xother")).activitySection.mode)
    }

    @Test
    fun `an Activity with rows and an unread network says the line above them`() {
        val model = home(feed(listOf(137), rows = listOf(received("r1"))))
        assertEquals(SectionMode.Rows, model.activitySection.mode)
        assertEquals("Activity can't be read on Polygon right now.", model.activitySection.note)
        assertEquals(1, model.activityGroups.single().rows.size)
        assertNull(home(feed(emptyList(), rows = listOf(received("r1")))).activitySection.note)
    }

    @Test
    fun `History says the unread networks the same way`() {
        val fallback = (FlowFixtures.build(FlowState.A1, strings).base as FlowBase.History).model
        val empty = FlowLive.history(fallback, feed(listOf(137)), strings, chainNames = chainNames, shownAccount = account)
        assertEquals(HistoryMode.Empty, empty.mode)
        assertEquals("Activity can't be read on Polygon right now.", empty.emptyText)
        assertNull(empty.note)
        val rows = FlowLive.history(fallback, feed(listOf(100, 137), rows = listOf(received("r1"))), strings, chainNames = chainNames, shownAccount = account)
        assertEquals(HistoryMode.Rows, rows.mode)
        assertEquals(strings.t("home.activityUnreadMany", mapOf("n" to "2")), rows.note)
        assertNull(FlowLive.history(fallback, feed(emptyList(), rows = listOf(received("r1"))), strings, chainNames = chainNames, shownAccount = account).note)
    }

    @Test
    fun `the unread keys are registered`() {
        assertEquals("home.activityUnreadOne", I18nKeys.Wallet.ACTIVITY_UNREAD_ONE)
        assertEquals("home.activityUnreadMany", I18nKeys.Wallet.ACTIVITY_UNREAD_MANY)
        for (key in listOf(I18nKeys.Wallet.ACTIVITY_UNREAD_ONE, I18nKeys.Wallet.ACTIVITY_UNREAD_MANY)) {
            assertNotEquals(key, strings.t(key))
        }
    }

    @Test
    fun `the home and History draw the unread line`() {
        val screen = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/wallet/WalletScreen.kt").readText()
        assertTrue(screen.contains("home.activity.unread"))
        val flows = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/flows/FlowScreens.kt").readText()
        assertTrue(flows.contains("history.unread"))
    }

    // -- item 6: "Updated" is one core answer --------------------------------------------

    @Test
    fun `Updated is drawn only from the core's updated_at_ms`() {
        val now = 1_000_000L
        // A round settled, but a network was not read: the core gives no stamp.
        val unread = WalletLive.refresh(BalanceView(last_refreshed_at_ms = (now - 60_000).toDouble(), updated_at_ms = null), strings, now)
        assertNull(unread.updated)
        // The core's stamp, drawn as said.
        val read = WalletLive.refresh(BalanceView(last_refreshed_at_ms = (now - 60_000).toDouble(), updated_at_ms = (now - 60_000).toDouble()), strings, now)
        assertNotNull(read.updated)
        assertTrue(read.updated!!.startsWith(strings.t(I18nKeys.Wallet.LAST_UPDATED, mapOf("ago" to "")).trim().substringBefore(" ")))
        // The view decodes it.
        assertEquals(5.0, Wire.json.decodeFromString(BalanceView.serializer(), """{"updated_at_ms":5}""").updated_at_ms!!, 0.0)
        assertNull(Wire.json.decodeFromString(BalanceView.serializer(), "{}").updated_at_ms)
    }
}
