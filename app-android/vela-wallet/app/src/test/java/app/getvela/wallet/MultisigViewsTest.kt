package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MsTone
import app.getvela.wallet.feature.multisig.MultisigNotifier
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.changeText
import app.getvela.wallet.feature.multisig.deltaText
import app.getvela.wallet.feature.multisig.memberRefusal
import app.getvela.wallet.feature.multisig.reviewConcern
import app.getvela.wallet.feature.multisig.stateWriteLines
import app.getvela.wallet.feature.multisig.waitingNote
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsPhase
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.str
import java.io.File
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The multisig screens' reading of the machines' views (native multisig
 * M1–M6): every view decodes leniently, and every line a sheet draws is the
 * core's verdict put in the corpus's words — in English, from the real
 * catalogs (the main one plus the on-demand `home.multisig` merged in).
 */
class MultisigViewsTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")

    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val carol = "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d"
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val hash = "0x" + "ab".repeat(32)

    private fun words(accounts: MsAccountsView = MsAccountsView()) = MultisigWords(
        strings = strings,
        accounts = accounts,
        localNames = mapOf(alice.lowercase() to "Alice"),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    private fun json(text: String): JsonObject = Wire.json.parseToJsonElement(text) as JsonObject

    private val accountsJson = """
        {"loaded":true,"active":"$safe","accounts":[{"account":{"address":"$safe","owners":["$alice","$bob","$carol"],"threshold":2,
         "recorded":true,"chainIds":[100],"firstRun":true,"genesis":{"name":"Treasury"},"members":{"${bob.lowercase()}":{"name":"Bob"}}},
         "name":null,"shared":true,"record_waiting":false,"local_members":["$alice"],"high_risk_on":[],"refused_on":[],
         "checked_on":[100],"checking_on":[],"invite_shared":false}],
         "create":{"phase":"working","draft_address":"$safe","networks":{"100":"present","8453":"missing"},"other_space":[10200],
                   "kept":{"owners":["$alice"],"members":{},"name":"T","threshold":1,"chainId":100}},
         "add":{"phase":"done","address":"$safe","outcome":{"kind":"added","address":"$safe"}},
         "link":{"kind":"missingNetwork","link":{"chainId":137,"multisig":"$safe","safeTxHash":"$hash"}},
         "known_members":{"${bob.lowercase()}":{"name":"Bob","kind":"wallet"}},
         "member_kinds":{"${bob.lowercase()}":{"kind":"wallet","suggested":null,"locked":false,"note":"likely"}},
         "rename":{"${safe.lowercase()}":{"chain_id":100,"deployed":true,"blocked":"pending","lands_on":100}},
         "share_again":{"${safe.lowercase()}":"working"}}
    """.trimIndent()

    private fun proposalsJson(extra: String = "") = """
        {"followed":"$safe","pace_ms":4000,"last_actor":"$alice","items":[{"chainId":100,"safeTxHash":"$hash","nonce":"3",
          "proposer":"$alice","approvals":[{"owner":"$alice"}],"approved":1,"threshold":2,"status":"ready",
          "canApprove":[],"canExecute":["$alice"],"sameNonce":[],"executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,
          "review":{"action":{"kind":"transfer","to":"$bob","amount":"1500000000000000000","token":null},"sign":{"allowed":true,"anyway":false}}}],
         "chains":[{"chainId":100,"members":{"deployed":true,"owners":["$alice","$bob","$carol"],"threshold":2,"nonce":"3"},"items":[],"mailbox":true}],
         "presence":{"${safe.lowercase()}":{"needs_you":1,"ready":2,"waiting_payer":0,"first":{"chain_id":100,"safe_tx_hash":"$hash"},"needs_you_keys":["100:${hash.lowercase()}"],"ready_keys":["100:0xr1","100:0xr2"],"read_at":10}},
         "standings":{"100:$hash":{"kind":"blocked","block":{"kind":"fails","why":{"kind":"short","token":{"symbol":"XDAI","decimals":18},"have":"280000000000000000","need":"500000000000000000"}}}},
         "signers":{"100:$hash":{"rows":[{"owner":"$bob","kind":"vela"},{"owner":"$carol","kind":"wallet"}],"in_vela":["$bob"],"outside":["$carol"],"either":[],"contract":[],"enough":true}},
         "waiting_parts":{"${safe.lowercase()}":[{"kind":"needsYou","count":1},{"kind":"ready","count":2}]},
         "banner":{"parts":[{"kind":"needsYou","count":1},{"kind":"ready","count":2}],"first":{"chain_id":100,"safe_tx_hash":"$hash"}},
         "rule_chip":{"kind":"needs","rule":{"m":2,"n":3,"solo":false}},
         "approve_execute":{"100:${hash.lowercase()}":["$alice"]},
         "set_up":true,
         "waiting_note":{"kind":"chainUnreachable","chain_ids":[100]}
         $extra}
    """.trimIndent()

    // -- the views decode ---------------------------------------------------------

    @Test
    fun the_accounts_view_reads_every_part_the_screens_draw() {
        val view = MsAccountsView.of(json(accountsJson))
        assertTrue(view.loaded)
        val row = view.activeRow!!
        assertEquals(2, row.account.threshold)
        assertEquals("Treasury", row.account.genesisName)
        assertEquals(listOf(alice), row.localMembers)
        assertEquals(MsPhase.Working, view.create.phase)
        assertEquals("missing", view.create.contracts(8453))
        assertEquals(listOf(10200), view.create.otherSpace)
        assertEquals("T", view.create.kept!!.name)
        assertEquals("added", (view.add.outcome as JsonObject)["kind"].toString().trim('"'))
        assertEquals("missingNetwork", view.link!!.kind)
        assertEquals(137, view.link!!.chainId)
        assertEquals("pending", view.lookup(view.rename, safe)!!.blocked)
        assertEquals(MsPhase.Working, view.lookup(view.shareAgain, safe))
        assertEquals("likely", view.memberKinds[bob.lowercase()]!!.note)
    }

    @Test
    fun a_view_missing_parts_reads_as_empty_never_throws() {
        val view = MsProposalsView.of(json("""{"items":[{"chainId":"100"}],"presence":{"x":null}}"""))
        assertEquals(100, view.items.single().chainId)
        assertEquals("pending", view.items.single().status)
        assertNull(view.presence["x"])
        assertEquals(12_000L, view.paceMs)
        assertFalse(MsAccountsView.of(json("{}")).loaded)
    }

    @Test
    fun the_proposals_view_looks_up_maps_in_either_case() {
        val view = MsProposalsView.of(json(proposalsJson()))
        val item = view.items.single()
        assertEquals("blocked", view.standing(item).let { (it as JsonObject)["kind"].toString().trim('"') })
        assertEquals(listOf(bob), view.signers(item)!!.inVela)
        assertEquals(1, view.presence[safe.lowercase()]!!.needsYou)
        assertEquals(hash, view.presence[safe.lowercase()]!!.firstHash)
        assertEquals(4_000L, view.paceMs)
    }

    // -- the words --------------------------------------------------------------------

    @Test
    fun a_transactions_one_line_is_the_cores_action_in_words() {
        val w = words()
        val transfer = json("""{"kind":"transfer","to":"$bob","amount":"1500000000000000000","token":null}""")
        assertEquals("Send 1.5 XDAI", w.actionTitle(100, transfer, "3"))
        assertEquals("Cancel transaction #3", w.actionTitle(100, json("""{"kind":"cancel"}"""), "3"))
        assertEquals("Require 3 approvals", w.actionTitle(100, json("""{"kind":"changeThreshold","threshold":3}"""), "1"))
        assertTrue(w.actionTitle(100, json("""{"kind":"addOwner","owner":"$alice"}"""), "1").contains("Alice"))
        assertEquals("A transaction Vela doesn't sign", w.actionTitle(100, null, "1"))
        assertEquals("2 transfers", w.actionTitle(100, json("""{"kind":"batch","calls":[{"action":{"kind":"transfer"}},{"action":{"kind":"transfer"}}]}"""), "1"))
    }

    @Test
    fun a_member_is_named_by_this_device_then_by_what_the_multisigs_keep_never_by_a_link() {
        val w = words(MsAccountsView.of(json(accountsJson)))
        assertEquals("Alice", w.label(alice))
        assertEquals("Bob", w.label(bob))
        assertTrue(w.who(bob).startsWith("Bob ("))
        assertEquals(MultisigWords.short(carol), w.label(carol))
        val row = MsAccountsView.of(json(accountsJson)).activeRow!!
        assertEquals("Treasury", w.name(row.account, row.name))
        assertEquals("Needs 2 of 3 members", w.ruleTitle(2, 3))
        assertEquals("One member", w.ruleChip(1, 1))
    }

    @Test
    fun a_shell_failure_is_said_in_the_corpus_words() {
        val w = words()
        assertEquals(w.t("proposal.reason.passkeyClosed"), w.failureText(MultisigShellFailure.Passkey("cancelled").detail))
        assertEquals(w.t("proposal.reason.passkeyTimedOut"), w.failureText(MultisigShellFailure.Passkey("cancelled", timedOut = true).detail))
        assertEquals(w.t("proposal.notSent"), w.failureText(MultisigShellFailure.NotSent.detail))
        assertEquals(w.t("proposal.failed", "reason" to "boom"), w.failureText("boom"))
        assertEquals(w.raw("componentsUi.signing.trustedSignerMismatch"), w.failureText(MultisigShellFailure.PageSignFailed("mismatch").detail))
        assertNull(w.actionFailure(json("""{"kind":"working"}"""), 100))
        assertEquals(w.t("proposal.mailboxDown"), w.actionFailure(json("""{"kind":"mailboxUnreadable"}"""), 100))
    }

    @Test
    fun a_ready_transaction_that_cannot_run_says_why_with_the_numbers() {
        val w = words()
        val view = MsProposalsView.of(json(proposalsJson()))
        val line = w.standing(view.standing(view.items.single()), view.items.single())
        assertEquals("Can’t run now — the multisig holds 0.28 XDAI, and this needs 0.5 XDAI", line)
    }

    @Test
    fun the_home_waiting_line_and_group_note_are_the_machines_words() {
        val w = words()
        val view = MsProposalsView.of(json(proposalsJson()))
        // core-ms-2: the parts, the banner, the chip and the one press are the machine's.
        assertEquals("1 needs your approval · 2 ready to execute", w.waitingParts(view.waitingPartsOf(safe.uppercase().replace("0X", "0x"))))
        assertEquals("1 needs your approval · 2 ready to execute", w.waitingParts(view.banner["parts"].arr))
        assertEquals(hash, view.banner["first"]["safe_tx_hash"].str)
        assertEquals("Needs 2 of 3", w.ruleChip(view.ruleChip))
        assertEquals(w.t("panel.chipSolo"), w.ruleChip(json("""{"kind":"needs","rule":{"m":1,"n":1,"solo":true}}""")))
        assertEquals(w.t("presence.ruleDiffers"), w.ruleChip(json("""{"kind":"differs","rules":[]}""")))
        assertEquals(listOf(alice), view.approveExecuteBy(view.items.single()))
        assertTrue(view.setUp)
        val (note, retry) = waitingNote(w, view.waitingNote) { "12:00" }!!
        assertTrue(note.contains("Gnosis"))
        assertTrue("chainUnreachable goes with Retry", retry)
        assertNull(w.waitingParts(emptyList()))
    }

    // -- the transaction sheet's model ------------------------------------------------

    @Test
    fun the_sheet_model_says_the_executor_fee_and_who_is_still_to_approve() {
        val previews = ""","previews":{"100:${hash}:${alice.lowercase()}":{"kind":"priced","fee":{"amount":"2000000000000000"},"canPay":true,
            "setup":{"multisig":true,"members":["$bob"],"executor":false,"amount":"1000000000000000"}}}"""
        val view = MsProposalsView.of(json(proposalsJson(previews)))
        val account = MsAccountsView.of(json(accountsJson)).activeRow!!.account
        val model = MsProposalModel(view.items.single(), view, words(MsAccountsView.of(json(accountsJson))), account, "https://wallet.getvela.app", "en")
        assertTrue(model.isReady)
        assertEquals(listOf(bob, carol), model.stillNeeded)
        val (fee, tone) = model.feeLine(alice)!!
        assertEquals(MsTone.Neutral, tone)
        assertTrue(fee, fee.contains("0.002 XDAI") && fee.contains("Alice"))
        val setup = model.setupLines(alice)
        assertEquals(3, setup.size)
        assertFalse(model.executeHeld(alice))
        assertEquals(listOf("inVela"), model.signerLines.map { it.id })
        assertTrue(model.outsideCanSign)
        assertTrue(model.notes.any { it.id == "blocked" })
        val link = model.link!!
        assertTrue(link, link.contains("#ms="))
    }

    @Test
    fun an_executor_who_cannot_pay_or_a_look_that_fails_holds_execute() {
        val cantPay = ""","previews":{"100:${hash}:${alice.lowercase()}":{"kind":"priced","fee":{"amount":"2000000000000000"},"canPay":false,"balance":"1"}}"""
        val view = MsProposalsView.of(json(proposalsJson(cantPay)))
        val model = MsProposalModel(view.items.single(), view, words(), null)
        assertTrue(model.executeHeld(alice))
        assertEquals(MsTone.Warning, model.feeLine(alice)!!.second)
        val fails = ""","previews":{"100:${hash}:${alice.lowercase()}":{"kind":"fails","why":{"kind":"reverts","reason":"GS013"}}}"""
        val failing = MsProposalsView.of(json(proposalsJson(fails)))
        val m2 = MsProposalModel(failing.items.single(), failing, words(), null)
        assertTrue(m2.executeHeld(alice))
        assertTrue(m2.feeLine(alice)!!.first.contains("GS013"))
        // Not looked at yet: checking, never a guess.
        val none = MsProposalsView.of(json(proposalsJson()))
        assertEquals(words().t("proposal.feeChecking"), MsProposalModel(none.items.single(), none, words(), null).feeLine(alice)!!.first)
    }

    @Test
    fun an_execution_followed_here_is_said_as_it_ended() {
        val executed = ""","executions":{"100:$hash":{"kind":"executed","txHash":"0xtx"}}"""
        val view = MsProposalsView.of(json(proposalsJson(executed)))
        val model = MsProposalModel(view.items.single(), view, words(), null)
        assertEquals("0xtx", model.executedTx)
        assertTrue(model.notes.any { it.id == "executed" && it.tone == MsTone.Positive })
    }

    @Test
    fun the_history_lines_name_who_did_what() {
        val view = MsProposalsView.of(json(proposalsJson()))
        val model = MsProposalModel(view.items.single(), view, words(), null)
        val approved = model.timelineLine(json("""{"kind":"approved","who":"$alice","counted":true}"""))!!
        assertTrue(approved, approved.contains("Alice"))
        assertNull(model.timelineLine(json("""{"kind":"somethingNew"}""")))
    }

    // -- the review --------------------------------------------------------------------

    @Test
    fun the_review_blocks_are_the_cores_verdicts_in_words() {
        val w = words()
        assertEquals(MsTone.Danger, reviewConcern(w, json("""{"kind":"lookalike","address":"$bob","label":"Bob","of":"$alice"}"""))!!.second)
        assertNull(reviewConcern(w, json("""{"kind":"stateChange"}""")))
        assertEquals("-1.5 XDAI", deltaText(json("""{"delta":"-1500000000000000000"}"""), 100, w, emptyMap()))
        assertEquals("+2 USDC", deltaText(json("""{"delta":"2000000","token":"0xA"}"""), 100, w, mapOf("0xa" to ("USDC" to 6))))
        assertTrue(changeText(w, json("""{"kind":"add","added":"$alice"}""")).contains("Alice"))
        val writes: List<JsonElement> = listOf(json("""{"kind":"module","enabled":true,"address":"0xM"}"""), json("""{"kind":"module","enabled":true,"address":"0xM"}"""))
        assertEquals(1, stateWriteLines(w, writes).size)
        assertTrue(memberRefusal(w, json("""{"kind":"refused","reason":{"kind":"lastMember"}}""")).isNotEmpty())
    }

    // -- notifications (M6) ------------------------------------------------------------

    @Test
    fun a_notification_is_said_once_for_what_changed_while_away() = runBlocking {
        val store = FakeStore()
        val posted = mutableListOf<MultisigNotifier.Note>()
        val notifier = MultisigNotifier(store, { words() }) { posted += it }
        val accounts = MsAccountsView.of(json(accountsJson))
        // First look: on screen — recorded, nothing said.
        assertEquals(emptyList<Any>(), notifier.evaluate(accounts, MsProposalsView.of(json(proposalsJson())), MultisigNotifier.Look.OnScreen))
        assertTrue(posted.isEmpty())
        // Away: a second approval counted, and one more needs this device.
        val moved = proposalsJson().replace("\"approved\":1", "\"approved\":2").replace("\"needs_you\":1", "\"needs_you\":2")
            .replace("\"needs_you_keys\":[", "\"needs_you_keys\":[\"100:0xnew\",")
        val notes = notifier.evaluate(accounts, MsProposalsView.of(json(moved)), MultisigNotifier.Look.Away(since = 5))
        assertEquals(setOf(MultisigNotifier.Channel.NeedsYou, MultisigNotifier.Channel.Counted), notes.map { it.channel }.toSet())
        assertEquals(2, posted.size)
        assertTrue(notes.all { it.safeTxHash == hash })
        assertEquals("the count is the number waiting now", "2 need your approval", notes.single { it.channel == MultisigNotifier.Channel.NeedsYou }.body)
        // Said once: the same views again say nothing.
        assertTrue(notifier.evaluate(accounts, MsProposalsView.of(json(moved)), MultisigNotifier.Look.Away(since = 5)).isEmpty())
        // An execution is said only away.
        notifier.executed("Send 1.5 XDAI", 100, hash, away = false)
        assertEquals(2, posted.size)
        notifier.executed("Send 1.5 XDAI", 100, hash, away = true)
        assertEquals(MultisigNotifier.Channel.Executed, posted.last().channel)
        assertTrue(JSONObject(store.values[MultisigNotifier.STATE_KEY]!!).has("approved"))
    }
}
