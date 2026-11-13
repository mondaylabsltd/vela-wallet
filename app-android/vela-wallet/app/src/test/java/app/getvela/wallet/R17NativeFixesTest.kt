package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.createNetworksOffered
import app.getvela.wallet.feature.multisig.createdSharedLine
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsLinkStand
import app.getvela.wallet.feature.multisig.core.MsPressOrder
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.HeldChains
import java.io.File
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 17's Android findings (docs/multisig-machines.md, the round-17 fixes):
 *
 * - r17-6: the chip says "Executing…" whenever the body does — before a finished one's count.
 * - r17-16: a failure a later press made stale is not drawn; a kept approval is always worded.
 * - r17-15: the create sheet's networks are the same in every language.
 * - r17-12: an execution the relay never took, or a read before the passkey that failed, is said by its kind.
 * - r17-14: the testnet space prices nothing in Send.
 * - r17-21: "Try again" for a read again, "Check again" only for a fee.
 * - r17-17 / r17-19: a link's failure and a refused create record are said in words, never a key.
 */
class R17NativeFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private fun runtime(tag: String): I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, t -> File(assets, I18nCatalogs.assetPath(t, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { t -> File(assets, "$t.json").readBytes() }.apply {
        initialize(tag)
        include(I18nCatalogs.MULTISIG)
    }

    private val strings: I18nRuntime = runtime("en")

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private fun source(path: String) = File(repoRoot, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/$path").readText()

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val safe = "0x7f3cE90000000000000000000000000000A0de87"
    private val hash = "0x" + "cd".repeat(32)

    private fun words(s: VelaStrings = strings) = MultisigWords(
        strings = s,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Interleave"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
    )

    private val words = words()

    /**
     * One transaction at [status] with [approved] of 2. [extra]: more of the
     * proposals view's fields (`executions`, `actions`, `outbox`), as JSON
     * members ending in a comma.
     */
    private fun proposal(
        status: String = "pending",
        approved: Int = 1,
        unsent: Boolean = false,
        extra: String = "",
        order: Map<String, Long> = emptyMap(),
        now: Long = 1_000_000,
    ): MsProposalModel {
        val view = MsProposalsView.of(
            json(
                """
                {"followed":"$safe","standings":{},$extra
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"5","proposer":"$bob","approvals":[{"owner":"$bob"}],"approved":$approved,"threshold":2,
                  "status":"$status","canApprove":[],"canExecute":["$alice"],"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":$unsent,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$bob","value":"0","data":"0x"},"sign":{"allowed":true,"anyway":false}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":true,"owners":["$alice","$bob"],"threshold":2,"nonce":"5"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        ).copy(pressOrder = order)
        return MsProposalModel(view.items.single(), view, words, null, nowMs = { now })
    }

    // -- r17-6 --------------------------------------------------------------------------------

    @Test
    fun `the mailbox says done while the chain has not answered - the chip says Executing as the body does, once`() {
        val m = proposal(status = "done", approved = 2, extra = """"executions":{"5042002:$hash":{"kind":"waiting"}},""")
        assertEquals(words.t("status.executing"), m.statusLine)
        // The body's own "Executing…" is not said a second time beside the chip.
        assertTrue(m.notes.any { it.id == "waiting" })
        assertTrue(m.sheetNotes.none { it.id == "waiting" })
        assertTrue(m.sheetNotes.none { it.text == m.statusLine })
    }

    @Test
    fun `an open one whose execution is followed here says Executing, never its count`() {
        val m = proposal(status = "ready", approved = 2, extra = """"executions":{"5042002:$hash":{"kind":"waiting"}},""")
        assertEquals(words.t("status.executing"), m.statusLine)
        assertTrue(m.sheetNotes.none { it.id == "waiting" })
        // The press out (its `execute:` working) says it before a finished one's count too.
        val pressed = proposal(status = "done", approved = 2, extra = """"actions":{"execute:$hash":{"kind":"working"}},""")
        assertEquals(words.t("status.executing"), pressed.statusLine)
    }

    @Test
    fun `an executed one keeps its own word`() {
        val m = proposal(status = "done", approved = 2, extra = """"executions":{"5042002:$hash":{"kind":"executed","txHash":"0x${"ab".repeat(32)}"}},""")
        assertEquals(m.approvalsCount, m.statusLine)
        assertNotEquals(words.t("status.executing"), m.statusLine)
    }

    // -- r17-16 (1): a failure a later press made stale ------------------------------------------

    private val failed = """{"kind":"failed","detail":"{\"kind\":\"unknown\"}"}"""
    private val done = """{"kind":"done","safe_tx_hash":"$hash","kept":false}"""

    @Test
    fun `a one press's failure is not drawn once a later Approve on the same transaction went through`() {
        val actions = """"actions":{"execute:$hash":$failed,"approve:$hash":$done},"""
        val later = proposal(status = "ready", approved = 2, extra = actions, order = mapOf("execute:$hash" to 1L, "approve:$hash" to 2L))
        assertNull("stale: a later press succeeded", later.executeFailure)
        // A failure newer than the success stands.
        val newer = proposal(status = "ready", approved = 2, extra = actions, order = mapOf("approve:$hash" to 1L, "execute:$hash" to 2L))
        assertEquals(words.t("proposal.reason.unknown"), newer.executeFailure?.text)
        // Order unknown (none seen pressed here): the failure is said, never hidden on a guess.
        assertEquals(words.t("proposal.reason.unknown"), proposal(status = "ready", approved = 2, extra = actions).executeFailure?.text)
    }

    @Test
    fun `presses are ordered by when their action went to working`() {
        var seq = 0L
        val next = { ++seq }
        val w = json("""{"kind":"working"}""")
        val f = json(failed)
        val d = json(done)
        var order = MsPressOrder.advance(emptyMap(), emptyMap(), mapOf("execute:$hash" to w), next)
        assertEquals(mapOf("execute:$hash" to 1L), order)
        order = MsPressOrder.advance(order, mapOf("execute:$hash" to w), mapOf("execute:$hash" to f), next)
        assertEquals("settling is no new press", mapOf("execute:$hash" to 1L), order)
        order = MsPressOrder.advance(order, mapOf("execute:$hash" to f), mapOf("execute:$hash" to f, "APPROVE:$hash" to w), next)
        assertEquals(2L, order["approve:$hash"])
        order = MsPressOrder.advance(order, mapOf("execute:$hash" to f, "approve:$hash" to w), mapOf("execute:$hash" to f, "approve:$hash" to d), next)
        assertEquals(mapOf("execute:$hash" to 1L, "approve:$hash" to 2L), order)
        // A press that settled before any view showed it working is still a press.
        order = MsPressOrder.advance(order, mapOf("execute:$hash" to f, "approve:$hash" to d), mapOf("execute:$hash" to json(done), "approve:$hash" to d), next)
        assertEquals(3L, order["execute:$hash"])
        // The hub keeps the order on the view it hands the sheets.
        assertTrue(source("feature/multisig/core/MultisigHub.kt").contains("MsPressOrder.advance("))
    }

    // -- r17-16 (2): a kept or refused approval record is worded ---------------------------------

    private fun outbox(entry: String) = """"outbox":{"5042002:${safe.lowercase()}:$hash:${bob.lowercase()}":$entry},"""

    @Test
    fun `an approval the service did not take is said, by its outbox entry keyed chain, multisig, hash and owner`() {
        val away = proposal(unsent = true, extra = outbox("""{"why":"away","retryAt":1012000,"sending":false}"""))
        val line = away.notes.single { it.id == "outbox" }
        assertEquals(words.t("proposal.outboxAway", "seconds" to "12"), line.text)
        assertEquals(MsTone.Warning, line.tone)
        assertFalse("sent again on its own: no second way", away.offersSendAgain)

        val kept = proposal(unsent = true, extra = outbox("""{"why":"away","retryAt":null,"sending":false}"""))
        assertEquals(words.t("proposal.outboxNotSent"), kept.notes.single { it.id == "outbox" }.text)
        assertTrue(kept.offersSendAgain)

        val sending = proposal(unsent = true, extra = outbox("""{"why":"sending","retryAt":null,"sending":true}"""))
        assertEquals(words.t("proposal.outboxSending"), sending.notes.single { it.id == "outbox" }.text)
        assertTrue(sending.outboxSending)
        assertFalse(sending.offersSendAgain)

        // No entry kept at all, and still not in the mailbox: never a bare Send again.
        val bare = proposal(unsent = true)
        assertEquals(words.t("proposal.outboxNotSent"), bare.notes.single { it.id == "outbox" }.text)
        assertTrue(bare.offersSendAgain)
        assertTrue(source("feature/multisig/MultisigProposalSheet.kt").contains("if (model.offersSendAgain)"))
    }

    // -- r17-12 -----------------------------------------------------------------------------

    @Test
    fun `an execution the relay never took is said as not sent, never Something went wrong`() {
        val unknown = words.t("proposal.reason.unknown")
        for (failure in listOf(UserOpSpine.Failure.NotSent, UserOpSpine.Failure.RelayerUnavailable)) {
            val detail = MultisigShellFailure.ofSubmit(failure, chainRead = false, signing = false, chainFailing = false, chainId = 5042002).detail
            assertEquals(words.t("proposal.notSent"), words.failureText(detail, 5042002))
        }
        // A read of the chain before the passkey that nobody answered: the network, named.
        val read = MultisigShellFailure.ofSubmit(UserOpSpine.Failure.Other("nonce"), chainRead = true, signing = false, chainFailing = false, chainId = 5042002).detail
        val away = words.failureText(read, 5042002)
        assertEquals(words.t("proposal.reason.chainUnreachable", "network" to "Arc Testnet"), away)
        assertNotEquals(unknown, away)
        // Before the passkey, on a network the pool finds failing: the same.
        val failing = MultisigShellFailure.ofSubmit(UserOpSpine.Failure.Other("x"), chainRead = false, signing = false, chainFailing = true, chainId = 5042002).detail
        assertEquals(away, words.failureText(failing, 5042002))
        // After the passkey a failure is not the network's alone: said as it is.
        val after = MultisigShellFailure.ofSubmit(UserOpSpine.Failure.Other("x"), chainRead = false, signing = true, chainFailing = true, chainId = 5042002).detail
        assertEquals(unknown, words.failureText(after, 5042002))
    }

    @Test
    fun `a gas estimate the relay did not answer is the relay unavailable, nothing sent`() {
        val spine = source("feature/send/core/UserOpSpine.kt")
        val unreachable = spine.substringAfter("RelayClient.EstimateAnswer.Unreachable ->").substringBefore("draft = userOpWithCalls")
        assertTrue(unreachable, unreachable.contains("Failure.RelayerUnavailable"))
        // The multisig shell hands the core the spine's refusal by its kind.
        assertTrue(source("feature/multisig/core/MultisigLive.kt").contains("MultisigShellFailure.ofSubmit("))
    }

    // -- r17-14 -----------------------------------------------------------------------------

    private val cny = CurrencyView(code = "CNY", rate = 7.1, committed = true)
    private val usdc = SendToken(network = "Arc Testnet", chain_id = 5042002, symbol = "USDC", balance = "0.2", decimals = 6, price_usd = 1.0)

    private fun ctx(money: WalletLive.Money) = SendLive.Context(strings, mapOf(5042002 to "Arc Testnet"), emptyMap(), money, "Main", alice)

    @Test
    fun `the testnet space's Send prices no amount and no fee`() {
        val testnet = WalletLive.Money.of(cny, "en").withoutValue()
        assertNull(testnet.fiat(1.0))
        assertTrue("nothing is withheld: there is no figure to wait for", testnet.settled)

        val pickFallback = (FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick).model
        val rows = SendLive.pick(pickFallback, SendView(tokens = listOf(usdc)), ctx(testnet)).rows
        assertTrue(rows.isNotEmpty() && rows.all { it.fiat == AssetFiatModel.None })
        val mainnetRows = SendLive.pick(pickFallback, SendView(tokens = listOf(usdc)), ctx(WalletLive.Money.of(cny, "en"))).rows
        assertTrue(mainnetRows.all { it.fiat is AssetFiatModel.Value })

        val formFallback = (FlowFixtures.build(FlowState.SD2N, strings).base as FlowBase.SendForm).model
        val typed = SendView(selected_token = usdc, amount = "0.1", token_amount = "0.1", denom_toggle_shown = true, denom_toggle_enabled = true)
        val amount = SendLive.form(formFallback, typed, app.getvela.wallet.feature.send.core.FeeView(), ctx(testnet)).amount!!
        assertEquals("" to false, amount.fiat to amount.fiatWithheld)
        assertFalse("no typing in a currency nothing here is worth", amount.denomShown)

        val confirmFallback = (FlowFixtures.build(FlowState.SD3K, strings).base as FlowBase.SendConfirm).model
        val confirm = SendLive.confirm(confirmFallback, typed.copy(confirm_amount = "0.1"), ctx(testnet))
        assertEquals("" to false, confirm.subline to confirm.sublineWithheld)

        val parts = SendLive.FeeParts(coin = "0.2 USDC", mark = WalletLive.mark(5042002, "USDC", null), units = 0.2, contract = null)
        assertEquals("0.2 USDC", SendLive.feeLine(parts, 1.0, testnet))
        // The flow's money is the space's (r18-9: by the core's word, `inSpace`).
        val host = source("navigation/VelaNavHost.kt")
        assertTrue(host.substringAfter("val sendMoney = ").substringBefore("\n").contains("inSpace(space)"))
    }

    // -- r17-15 -----------------------------------------------------------------------------

    @Test
    fun `the create sheet offers the same networks in every shipped language`() {
        val tags = File(assets, "multisig").list()!!.map { it.removeSuffix(".json") }.sorted()
        assertTrue(tags.size >= 15 && "zh" in tags)
        val mainnet = listOf(1, 10, 56, 100, 130, 137, 146, 988, 1868, 4326, 8453, 42161, 43114, 59144, 80094, 98866, 534352, 1440000)
        val contracts = mapOf(130 to "missing")
        val lists = tags.associateWith { tag ->
            // Each language's words, as the sheet has them: the list never reads them.
            words(runtime(tag))
            createNetworksOffered(mainnet, { contracts[it] }, mapOf("here" to emptyList<Int>(), "used" to emptyList<Int>(), "recent" to emptyList<Int>()))
        }
        assertEquals(1, lists.values.toSet().size)
        assertFalse(130 in lists.getValue("en"))
    }

    @Test
    fun `the shown networks settle as soon as the first holdings round is read, not at the next launch`() {
        val first = HeldChains.merge(before = null, held = listOf(5042002L))
        assertTrue(first.write && first.firstRead)
        assertEquals(listOf(5042002L), first.union)
        val again = HeldChains.merge(before = listOf(5042002L), held = listOf(5042002L))
        assertFalse(again.write || again.firstRead)
        val more = HeldChains.merge(before = listOf(1L), held = listOf(5042002L))
        assertTrue(more.write && !more.firstRead)
        // The app reads the network record again then — the core settles it.
        val app = source("VelaWalletApplication.kt")
        assertTrue(app.substringAfter("onHoldingsFirstRead =").substringBefore("\n").contains("startNetworks()"))
    }

    // -- r17-4 (the shell's half) -------------------------------------------------------------

    @Test
    fun `a network Activity could not read is named, never by its raw id`() {
        val feed = app.getvela.wallet.feature.wallet.core.FeedView(unread_chains = listOf(5042002), unread_key = "home.activityUnreadOne", account = alice)
        val mainnetNames = mapOf(1 to "Ethereum")
        // The whole list names it.
        val fromAll = WalletLive.unreadLine(feed, strings, mainnetNames, allNames = mapOf(5042002 to "Arc Testnet"))!!
        assertEquals(strings.t("home.activityUnreadOne", mapOf("network" to "Arc Testnet")), fromAll)
        // Neither list names it: the core's catalog does.
        val fromCatalog = WalletLive.unreadLine(feed, strings, mainnetNames)!!
        assertFalse(fromCatalog, fromCatalog.contains("5042002"))
        assertTrue(source("navigation/VelaNavHost.kt").contains("allChainNames = allNetworks.networks"))
    }

    // -- r17-21 -----------------------------------------------------------------------------

    @Test
    fun `a read again says Try again and only a fee asked again says Check again`() {
        for (file in listOf("MultisigHomeCard.kt", "MultisigProposalSheet.kt", "MultisigSettingsSheets.kt")) {
            assertFalse(file, source("feature/multisig/$file").contains("\"service.retry\""))
        }
        assertEquals("Try again", words.t("review.state.tryAgain"))
        assertEquals("Check again", words.t("proposal.feeRetry"))
        // Execute's failure: "Check again" only when what stopped it was its fee.
        val sheet = source("feature/multisig/MultisigProposalSheet.kt")
        val execute = sheet.substringAfter("modifier = Modifier.fillMaxWidth().testTag(\"multisig.proposal.execute\")").substringBefore("val look = model.preview(chosen)")
        assertTrue(execute, execute.contains("retryLabel = if (model.failureChecksFee) words.t(\"proposal.feeRetry\") else null"))
    }

    // -- r17-17 / r17-19: never a catalog key on screen -----------------------------------------

    /** The runtime's own fallback for a key it does not have is the key: recorded here. */
    private class Recording(val inner: VelaStrings) : VelaStrings by inner {
        val missed = mutableListOf<String>()
        override fun t(key: String): String = inner.t(key).also { if (it == key) missed += key }
        override fun t(key: String, vars: Map<String, String>): String = inner.t(key, vars).also { if (it.contains(key)) missed += key }
    }

    @Test
    fun `every link failure and damage is said in words, never its key`() {
        val rec = Recording(strings)
        val w = words(rec)
        val kinds = listOf(null, "notFound", "unavailable", "unsupported", "notMultisig", "chooseNetwork", "unchecked", "busy", "unverified", "refused", "highRisk", "something")
        for (kind in kinds) {
            val reason = kind?.let { json("""{"kind":"$it","chain_id":5042002,"findings":[]}""") }
            val line = w.linkFailed(reason)
            assertFalse("$kind: $line", line.contains("home.multisig.") || line.isBlank())
        }
        assertEquals(w.t("authority.refusedTitle"), w.linkFailed(json("""{"kind":"refused","findings":[]}""")))
        assertEquals(w.t("authority.riskTitle"), w.linkFailed(json("""{"kind":"highRisk","findings":[]}""")))
        assertEquals(w.t("authority.unverified", "network" to "Arc Testnet"), w.linkFailed(json("""{"kind":"unverified","chain_id":5042002}""")))
        assertEquals(w.t("add.unavailable"), w.linkFailed(json("""{"kind":"something"}""")))
        for (stand in listOf(
            """{"kind":"broken","link":"proposal","text":"#ms=abc","fault":null}""",
            """{"kind":"broken","link":"add","text":"#msa=abc","fault":"checksum"}""",
            """{"kind":"broken","link":"add","text":"#msa=abc","fault":"damaged"}""",
        )) {
            val line = w.linkBroken(MsLinkStand.of(json(stand)))
            assertFalse(line, line.contains("home.multisig.") || line.contains("#ms"))
        }
        assertEquals(w.t("add.badChecksum"), w.linkBroken(MsLinkStand.of(json("""{"kind":"broken","link":"add","text":"x","fault":"checksum"}"""))))
        assertEquals(w.t("add.badInvite"), w.linkBroken(MsLinkStand.of(json("""{"kind":"broken","link":"add","text":"x","fault":"damaged"}"""))))
        assertEquals(w.t("proposal.badLink"), w.linkBroken(MsLinkStand.of(json("""{"kind":"broken","link":"proposal","text":"x","fault":null}"""))))
        // Every failure kind the core names, too.
        for (detail in listOf(
            """{"kind":"not_sent"}""", """{"kind":"chain_unreachable","chainId":5042002}""", """{"kind":"unknown"}""", "plain english",
            """{"kind":"passkey","failure":"cancelled","timedOut":true}""", """{"kind":"page_sign_failed","page":"unanswered"}""",
        )) {
            val line = w.failureText(detail, 5042002)
            assertFalse("$detail: $line", line.contains("home.multisig.") || line.contains("componentsUi."))
        }
        assertEquals("no key missed: ${rec.missed}", emptyList<String>(), rec.missed)
        // The sheet draws the words, never the link's raw text.
        val sheets = source("feature/multisig/MultisigAccountSheets.kt")
        assertFalse(sheets.contains("words.t(\"add.\${reason.kind}\")"))
        assertTrue(sheets.contains("words.linkBroken(stand)"))
    }

    @Test
    fun `a create the service answered and did not record is said as Share it again, never as unreachable`() {
        val refused = createdSharedLine(words, shared = false, refused = "refused", working = false, solo = false)
        assertEquals(words.t("settings.shareAgainNote") to MsTone.Warning, refused)
        val later = createdSharedLine(words, shared = false, refused = null, working = false, solo = false)
        assertEquals(words.t("create.savedLater") to MsTone.Neutral, later)
        assertEquals(words.t("create.shared") to MsTone.Positive, createdSharedLine(words, shared = true, refused = null, working = false, solo = false))
        assertEquals(words.t("create.savedSharing") to MsTone.Neutral, createdSharedLine(words, shared = false, refused = null, working = true, solo = false))
        // Refused wins over a sharing still said as working.
        assertEquals(refused, createdSharedLine(words, shared = false, refused = "refused", working = true, solo = false))
        assertTrue(source("feature/multisig/MultisigAccountSheets.kt").contains("createdSharedLine(words,"))
    }
}
