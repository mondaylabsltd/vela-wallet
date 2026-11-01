package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigNotifier
import app.getvela.wallet.feature.multisig.MultisigNotifyAsk
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MsAccount
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.inviteKnown
import app.getvela.wallet.feature.multisig.refusalLine
import app.getvela.wallet.feature.multisig.shownConcerns
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.settledHaptic
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.SendMultisigRef
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.ScannedLink
import java.io.File
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonElement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 9's Android shell fixes (r9-4, r9-5, r9-8, r9-12, r9-13, r9-14,
 * r9-15, r9-16 and the S4s), read through the same builders the screens draw
 * from — the core's readings in (the link door over JNA, the machines'
 * views as JSON), what the app does and says out.
 */
class R9ShellFixesTest {

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
    private val safe = "0x726726d55Aa06DB856d4E6a9e65e8773829BC302"
    private val hash = "0x" + "ab".repeat(32)
    private val cancel = "0x" + "cd".repeat(32)

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    private fun words(local: Map<String, String> = mapOf(alice.lowercase() to "Alice")) = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = local,
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    // -- r9-4 / r9-16: a scanned or pasted text is read by the core's link reading first --

    @Test
    fun a_scanned_invite_opens_the_add_sheet_never_send() {
        val invite = "https://wallet.getvela.app/en/wallet#msa=100:$safe&a=88cca0eedbf2c4426110bbfc998f048689266894pParallel%20One"
        val route = ScannedLink.route(invite)
        assertTrue("an invite is the link flow's: $route", route is ScannedLink.Route.Multisig)
        assertTrue((route as ScannedLink.Route.Multisig).hash.startsWith("#msa=100:$safe"))
        // The same invite from another page's address (a local web wallet, a
        // forwarded link) — what it is is the core's reading of its fragment.
        assertTrue(ScannedLink.route("http://localhost:5491/en/wallet#msa=100:$safe") is ScannedLink.Route.Multisig)
        // The bare fragment, and an encoded `=`, as a chat app mangles it.
        assertEquals(ScannedLink.Route.Multisig("#msa=$safe"), ScannedLink.route("msa=$safe"))
        assertTrue(ScannedLink.route("https://wallet.getvela.app/en/wallet#msa%3D100:$safe") is ScannedLink.Route.Multisig)
    }

    @Test
    fun a_scanned_transaction_link_opens_its_sheet() {
        val route = ScannedLink.route("https://wallet.getvela.app/zh/wallet#ms=100.$safe.$hash")
        assertEquals(ScannedLink.Route.Multisig("#ms=100.$safe.$hash"), route)
        // Damaged, it is still the link flow's (which says how), never a recipient.
        assertTrue(ScannedLink.route("https://wallet.getvela.app/en/wallet#ms=100.$safe") is ScannedLink.Route.Multisig)
    }

    @Test
    fun a_payment_the_connect_page_and_an_address_go_where_they_belong() {
        assertTrue(ScannedLink.route("https://getvela.app/pay?to=$bob&chain=100&amount=0.001") is ScannedLink.Route.Pay)
        assertTrue(ScannedLink.route("velawallet://pay?to=$bob&chain=100&amount=0.001") is ScannedLink.Route.Pay)
        assertTrue(ScannedLink.route("https://wallet.getvela.app/en/connect#for=$bob&ms=100.$safe.$hash") is ScannedLink.Route.Connect)
        assertEquals(ScannedLink.Route.Send(bob), ScannedLink.route(" $bob "))
        assertEquals(ScannedLink.Route.Send("ethereum:$bob@100"), ScannedLink.route("ethereum:$bob@100"))
        assertTrue(ScannedLink.route("https://app.uniswap.org/#/swap") is ScannedLink.Route.Send)
        assertNull("a text with spaces is no one link", ScannedLink.multisigHash("see #msa=$safe please"))
        assertNull("an address is not a multisig link", ScannedLink.multisigHash(bob))
    }

    // -- r9-5: a refusal is said where the press was, never only a haptic ------------------

    private fun proposals(
        canApprove: String = "[\"$alice\"]",
        actions: String = "{}",
        sameNonce: String = "[]",
        cancelledBy: String? = null,
        proposer: String = bob,
        ruleChip: String? = null,
        status: String = "pending",
        membersHere: String? = null,
    ): MsProposalsView = MsProposalsView.of(
        json(
            """
            {"followed":"$safe","actions":$actions,"standings":{"100:$hash":{"kind":"yourTurn"}},${ruleChip?.let { "\"rule_chip\":$it," }.orEmpty()}
             "items":[{"chainId":100,"safeTxHash":"$hash","nonce":"4",
              "proposer":"$proposer","approvals":[{"owner":"$bob"}],"approved":1,"threshold":2,"status":"$status",
              "canApprove":$canApprove,"canExecute":[],"cancelledBy":${cancelledBy?.let { "\"$it\"" } ?: "null"},"sameNonce":$sameNonce,
              "executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,${membersHere?.let { "\"membersHere\":$it," }.orEmpty()}
              "review":{"action":{"kind":"transfer","to":"$bob","amount":"1","token":null},"sign":{"allowed":true,"anyway":false}}}],
             "chains":[{"chainId":100,"members":{"deployed":true,"owners":["$alice","$bob","$carol","0x00000000000000000000000000000000000000e5"],"threshold":2,"nonce":"4"},"items":[],"mailbox":true}]}
            """.trimIndent(),
        ),
    )

    private fun model(view: MsProposalsView, w: MultisigWords = words()) =
        MsProposalModel(view.items.single(), view, w, null, "https://wallet.getvela.app", "en")

    @Test
    fun a_refused_approve_is_said_under_its_button_with_its_reason() {
        val m = model(proposals(actions = """{"approve:$hash":{"kind":"signingUnchecked"}}"""))
        val line = m.approveFailure
        assertNotNull(line)
        assertEquals(strings.t("home.multisig.authority.signUnchecked", mapOf("network" to "Gnosis")), line!!.text)
        assertTrue("said once, under the button — not again at the top", m.notes.none { it.id == "approveFailed" })
        // The core's own reason (a stale "nothing to approve") is still words.
        val failed = model(proposals(actions = """{"approve:$hash":{"kind":"failed","detail":"nothing to approve"}}""")).approveFailure
        assertTrue(failed!!.text.contains("nothing to approve"))
        // Working and the person's own cancel say nothing.
        assertNull(model(proposals(actions = """{"approve:$hash":{"kind":"working"}}""")).approveFailure)
        assertNull(model(proposals(actions = """{"approve:$hash":{"kind":"cancelled"}}""")).approveFailure)
    }

    @Test
    fun the_refusal_buzz_goes_only_with_words() {
        assertEquals(VelaHaptic.Counted, settledHaptic("done"))
        assertEquals(VelaHaptic.Reject, settledHaptic("signingUnchecked"))
        assertEquals(VelaHaptic.Reject, settledHaptic("failed"))
        // The person's own "don't sign" or cancel: no refusal buzz with nothing said.
        assertNull(settledHaptic("riskDeclined"))
        assertNull(settledHaptic("cancelled"))
        assertNull(settledHaptic(null))
    }

    // -- r9-13: a rejected transaction and its cancellation, as the core says them -----

    @Test
    fun a_cancellation_waiting_is_said_beside_the_actions_with_the_conflict() {
        val w = words()
        val m = model(proposals(sameNonce = "[\"$cancel\"]", cancelledBy = cancel), w)
        assertEquals(listOf("conflict", "cancelPending"), m.cancelNotes.map { it.id })
        assertEquals(strings.t("home.multisig.proposal.conflict", mapOf("nonce" to "4")), m.cancelNotes.first().text)
        assertTrue("not said twice at the top", m.notes.none { it.id == "conflict" || it.id == "cancelPending" })
        // The core's standing, and "Only one of these can run" beside it (the web's row).
        assertEquals(
            "${strings.t("home.multisig.status.yourTurn")} · ${strings.t("home.multisig.status.conflict")}",
            m.statusLine,
        )
        // Alone at its number: the standing alone.
        assertEquals(strings.t("home.multisig.status.yourTurn"), model(proposals(), w).statusLine)
    }

    @Test
    fun ones_own_proposal_is_withdrawn_anothers_rejected() {
        assertTrue(model(proposals(proposer = alice)).ownProposal)
        assertFalse(model(proposals(proposer = bob)).ownProposal)
    }

    // -- r9-14: the rule the chain says, everywhere -------------------------------------

    @Test
    fun the_switcher_says_the_rule_the_chains_say_for_the_one_followed() {
        val w = words()
        val account = MsAccount.of(json("""{"address":"$safe","owners":["$alice","$bob","$carol"],"threshold":2,"chainIds":[100]}"""))
        val chip = """{"kind":"needs","rule":{"m":2,"n":4,"solo":false}}"""
        assertEquals(strings.t("home.multisig.panel.chip", mapOf("m" to "2", "n" to "4")), w.rowRuleChip(account, proposals(ruleChip = chip)))
        // A multisig the machine does not follow: its record (no chain read for it).
        val other = MsAccount.of(json("""{"address":"$bob","owners":["$alice","$bob","$carol"],"threshold":2,"chainIds":[100]}"""))
        assertEquals(strings.t("home.multisig.panel.chip", mapOf("m" to "2", "n" to "3")), w.rowRuleChip(other, proposals(ruleChip = chip)))
        assertEquals(strings.t("home.multisig.panel.title", mapOf("m" to "2", "n" to "4")), w.ruleTitleOf(json(chip)))
        assertNull(w.ruleTitleOf(json("""{"kind":"differs","rules":[]}""")))
    }

    // -- r9-15: a multisig's send words, its confirm, the fee coin -----------------------

    private fun ctx(membersOn: (Int) -> Pair<Int, Int>? = { null }, symbols: Map<Int, String> = emptyMap()) = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis", 5042002 to "Arc Testnet"),
        explorers = emptyMap(),
        money = WalletLive.Money.of(CurrencyView(code = "USD", committed = true)),
        fromName = "R9 Team",
        fromAddress = safe,
        nativeSymbols = symbols,
        multisigMembersOn = membersOn,
    )

    private fun fee(chain: Int) = FeeEstimateView(
        chain_id = chain, total_wei = "82500000000000000", max_fee_per_gas = "1", network_fee_per_gas = "1", relayer_fee_per_gas = "1",
        bundler_gas_price = "1", in_band_gas_basis = "1", total_gas = "1", deployed = true, tier = FeeTier.Fast, quoted = true,
        fee_asset = FeeAssetView.Native, fee_recipient = "0x2222222222222222222222222222222222222222",
    )

    @Test
    fun a_multisig_confirm_draws_who_must_approve_never_a_fee() {
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "1", decimals = 18, price_usd = 1.0)
        val view = SendView(
            stage = SendStage.Confirm, selected_token = xdai, tokens = listOf(xdai), recipient = bob, amount = "0.01",
            confirm_amount = "0.01", multisig = SendMultisigRef(2, 3), fee = fee(100),
        )
        val speed = FeeSpeedView(tier = FeeTier.Fast, picked = true)
        // The chain says 2 of 4 now (a member added): that, not the founding 2 of 3.
        val page = SendLive.confirm(drawn, view, ctx(membersOn = { if (it == 100) 2 to 4 else null }), FeeView(fee = fee(100)), speed)
        val labels = page.facts.map { it.label }
        assertFalse("no fee row: ${page.facts}", I18nKeys.Flows.EST_FEE.let(strings::t) in labels)
        assertFalse("no speed row", strings.t(I18nKeys.Flows.FEE_SPEED_LABEL) in labels)
        val approvals = page.facts.single { it.label == strings.t("home.multisig.proposal.approvals") }
        assertEquals(strings.t("home.multisig.panel.title", mapOf("m" to "2", "n" to "4")), approvals.value)
        assertEquals(strings.t("home.multisig.proposal.feeNote"), approvals.note)
        assertEquals(strings.t("home.multisig.send.propose"), page.cta)
        assertFalse("no raw key anywhere", page.facts.any { it.value.startsWith("home.") || (it.note ?: "").startsWith("home.") })
    }

    @Test
    fun an_own_send_on_arc_names_arcs_fee_coin() {
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val usdc = SendToken(network = "chain-5042002", chain_id = 5042002, symbol = "USDC", balance = "1", decimals = 6, price_usd = 1.0)
        val view = SendView(stage = SendStage.Confirm, selected_token = usdc, tokens = listOf(usdc), recipient = bob, amount = "0.2", confirm_amount = "0.2", fee = fee(5042002))
        val page = SendLive.confirm(drawn, view, ctx(symbols = mapOf(5042002 to "USDC")), FeeView(fee = fee(5042002)))
        val feeRow = page.facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertTrue("Arc pays its fee in USDC: ${feeRow.value}", feeRow.value.contains("USDC"))
        assertFalse(feeRow.value.contains("ETH"))
    }

    // -- r9-12: the invite carries every member's label, by the core's rule -------------

    @Test
    fun the_invite_names_this_devices_own_account_too() {
        val w = words(local = mapOf(alice.lowercase() to "Parallel One"))
        val account = MsAccount.of(
            json("""{"address":"$safe","owners":["$alice","$bob"],"threshold":2,"chainIds":[100],"members":{"${bob.lowercase()}":{"name":"Eve","kind":"wallet"}}}"""),
        )
        val known = inviteKnown(w, account, emptyList())
        @Suppress("UNCHECKED_CAST")
        assertEquals("Parallel One", (known[alice.lowercase()] as Map<String, Any?>)["name"])
        @Suppress("UNCHECKED_CAST")
        val eve = known[bob.lowercase()] as Map<String, Any?>
        assertEquals("Eve", eve["name"])
        assertEquals("wallet", eve["kind"])
        // An unnamed member travels with no name (the short address is not one).
        assertNull(inviteKnown(words(local = emptyMap()), MsAccount.of(json("""{"address":"$safe","owners":["$carol"],"threshold":1,"chainIds":[100]}""")), emptyList())[carol.lowercase()])
    }

    // -- r9-8: the poll says a new proposal another member made; asked once ---------------

    private val accountsJson = """{"loaded":true,"active":"$safe","accounts":[{"account":{"address":"$safe","owners":["$alice","$bob"],"threshold":2,"chainIds":[100],"label":"R9 Team"},"name":"R9 Team","shared":true,"record_waiting":false,"local_members":["$alice"]}]}"""

    private fun presence(needs: Int, first: String?) =
        """{"followed":"$safe","items":[],"presence":{"${safe.lowercase()}":{"needs_you":$needs,"ready":0,"waiting_payer":0,"first":${first?.let { "{\"chain_id\":100,\"safe_tx_hash\":\"$it\"}" } ?: "null"}}}}"""

    @Test
    fun a_new_proposal_from_another_member_is_said_once_with_its_transaction() = runBlocking {
        val store = FakeStore()
        val posted = mutableListOf<MultisigNotifier.Note>()
        val notifier = MultisigNotifier(store, { words() }) { posted += it }
        val accounts = MsAccountsView.of(json(accountsJson))
        // The app was on screen with nothing waiting.
        notifier.evaluate(accounts, MsProposalsView.of(json(presence(0, null))), post = false)
        // Away (a cold process): Eve proposed — it needs this device.
        val notes = notifier.evaluate(accounts, MsProposalsView.of(json(presence(1, hash))), post = true)
        assertEquals(1, notes.size)
        val note = notes.single()
        assertEquals(MultisigNotifier.Channel.NeedsYou, note.channel)
        assertEquals(hash, note.safeTxHash)
        assertEquals(100, note.chainId)
        assertEquals(safe, note.address)
        assertEquals(posted, notes)
        assertTrue("said once", notifier.evaluate(accounts, MsProposalsView.of(json(presence(1, hash))), post = true).isEmpty())
    }

    @Test
    fun the_multisig_asks_for_notifications_once_on_its_own() = runBlocking {
        val store = FakeStore()
        // A send receipt's ask (087 F14) does not use the multisig's.
        store.write(app.getvela.wallet.core.data.NotificationAsk.KEY, "1")
        assertTrue(MultisigNotifyAsk.claim(store))
        assertFalse(MultisigNotifyAsk.claim(store))
    }

    @Test
    fun the_debug_poll_trigger_is_not_in_the_app() {
        val app = File(repoRoot, "app-android/vela-wallet/app/src")
        assertFalse("no release manifest names it", File(app, "main/AndroidManifest.xml").readText().contains("MultisigPollTrigger"))
        assertTrue("the debug manifest does", File(app, "debug/AndroidManifest.xml").readText().contains("MultisigPollTrigger"))
        assertTrue(File(app, "debug/java/app/getvela/wallet/dev/MultisigPollTrigger.kt").isFile)
        assertFalse("the class lives in the debug source set only", File(app, "main/java").walkTopDown().any { it.isFile && it.readText().contains("class MultisigPollTrigger") })
        val check = File(repoRoot, "app-android/vela-wallet/scripts/check-release-apk.sh").readText()
        assertTrue("check-release-apk.sh refuses a package carrying it", check.contains("Lapp/getvela/wallet/dev/MultisigPollTrigger;"))
    }

    // -- the core's r9 fields (core §8), drawn not chosen ---------------------------------

    @Test
    fun a_refusal_is_said_once_by_its_own_reason() {
        val w = words()
        val delegate = json(
            """{"sign":{"allowed":false,"anyway":false},"refused":{"why":"delegatecall","key":"home.multisig.review.refusedWhy.delegatecall"},
               "concerns":[{"kind":"notSigned","why":"delegatecall to a contract Vela does not sign for"}],"recipients":[],
               "simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"withheld"}}}""",
        )
        assertEquals(strings.t("home.multisig.review.refusedWhy.delegatecall"), refusalLine(delegate, w))
        assertNotEquals(strings.t("home.multisig.review.blocked"), refusalLine(delegate, w))
        assertTrue("the English reason is not said again", shownConcerns(delegate).none { it.toString().contains("notSigned") })
        // An older core: the old line, when it may not be signed; nothing when it may.
        assertEquals(strings.t("home.multisig.review.blocked"), refusalLine(json("""{"sign":{"allowed":false}}"""), w))
        assertNull(refusalLine(json("""{"sign":{"allowed":true}}"""), w))
    }

    @Test
    fun not_a_member_here_only_when_the_core_says_none_is() {
        // A refused delegatecall: nobody can approve it, but this device's member is one.
        val refused = model(proposals(canApprove = "[]", membersHere = "[\"$alice\"]"))
        assertTrue(refused.notes.none { it.id == "notMember" })
        val none = model(proposals(canApprove = "[]", membersHere = "[]"))
        assertTrue(none.notes.any { it.id == "notMember" })
    }

    @Test
    fun one_name_per_member_from_the_accounts_machine() {
        val labels = MsAccountsView.of(json("""{"loaded":true,"accounts":[],"member_labels":{"${bob.lowercase()}":"Interleave"},"index_wanted":["$carol"]}"""))
        assertEquals(listOf(carol), labels.indexWanted)
        val w = MultisigWords(strings = strings, accounts = labels, localNames = mapOf(bob.lowercase() to "Parallel space"))
        assertEquals("the core's label, never this device's own name", "Interleave", w.label(bob))
    }

    @Test
    fun the_invite_is_the_cores_members_through_add_link() {
        val members = listOf(json("""{"address":"$alice","name":"Parallel One","kind":"passkey"}"""), json("""{"address":"$bob","kind":"wallet"}"""))
        val link = MsRules.addLink("https://wallet.getvela.app", "en", safe, listOf(100), members)!!
        assertTrue(link, link.startsWith("https://wallet.getvela.app/en/wallet#msa=100:$safe"))
        assertTrue("its name travels: $link", link.contains("Parallel%20One"))
    }

    @Test
    fun one_approval_is_one_approval() {
        val w = words()
        assertEquals("Require 1 approval", w.actionTitle(100, json("""{"kind":"changeThreshold","threshold":1}"""), "4"))
        assertEquals("Require 3 approvals", w.actionTitle(100, json("""{"kind":"changeThreshold","threshold":3}"""), "4"))
        assertTrue(w.plural("proposal.rejectHint", 1).contains("1 approval too"))
    }

    @Test
    fun a_site_starts_where_the_account_lives() {
        // r9 (core `site_start_chain_for`): an Arc-only wallet in the testnet space starts a new site on Arc.
        assertEquals(5042002u, uniffi.vela_core_uniffi.siteStartChainFor(listOf(11155111u, 84532u, 5042002u), null, listOf(5042002u)))
        // Nothing held or used: the space's first network.
        assertEquals(11155111u, uniffi.vela_core_uniffi.siteStartChainFor(listOf(11155111u, 5042002u), null, emptyList()))
        // The browser machine is told the account's networks after the switch.
        val events = app.getvela.wallet.feature.browser.core.BrowserAccounts.events(listOf(alice), alice, 1.0, isSpaceAccount = { false }, networks = listOf(5042002))
        assertEquals(app.getvela.wallet.feature.browser.core.DbrEvent.AccountNetworks(listOf(5042002)), events.last())
    }
}
