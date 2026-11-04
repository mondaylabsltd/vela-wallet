package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.crashBody
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.multisig.MsProposalModel
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.anywayLines
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalsView
import app.getvela.wallet.feature.multisig.reviewUnread
import app.getvela.wallet.feature.multisig.shownConcerns
import app.getvela.wallet.feature.multisig.waitingConnectLink
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.SendMultisigRef
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Round 8's Android shell fixes (r8-1 shell side, r8-3, r8-8, r8-14, r8-18,
 * r8-19), read through the same builders the screens draw from — the core's
 * verdicts in, what the sheet says and offers out.
 */
class R8ShellFixesTest {

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

    private fun words() = MultisigWords(
        strings = strings,
        accounts = MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Alice"),
        networkName = { if (it == 100) "Gnosis" else null },
        nativeSymbol = { if (it == 100) "XDAI" else null },
    )

    private fun json(text: String): JsonElement = Wire.json.parseToJsonElement(text)

    /** One transaction, the review and signers given (raw JSON parts). */
    private fun proposals(approved: Int, canApprove: String, review: String?, signers: String? = null): MsProposalsView {
        // §12: the core says where the review stands beside it — read, or not
        // readable (`null`); nothing asked yet has no entry (loading).
        val state = when (review) {
            null -> null
            "null" -> """{"kind":"failed","reason_key":"home.multisig.review.state.unreadable"}"""
            else -> """{"kind":"ready"}"""
        }
        val reviews = review?.let { """"reviews":{"100:$hash":$it},"review_states":{"100:$hash":$state},""" }.orEmpty()
        val split = signers?.let { """"signers":{"100:$hash":$it},""" }.orEmpty()
        return MsProposalsView.of(
            json(
                """
                {"followed":"$safe",$reviews$split"items":[{"chainId":100,"safeTxHash":"$hash","nonce":"3",
                  "proposer":"$alice","approvals":[{"owner":"$alice"}],"approved":$approved,"threshold":2,"status":"${if (approved >= 2) "ready" else "pending"}",
                  "canApprove":$canApprove,"canExecute":["$alice"],"sameNonce":[],"executing":false,"createdMs":1,"unsent":false,"renameUndeployed":false,
                  "review":{"action":{"kind":"transfer","to":"$bob","amount":"1","token":null},"sign":{"allowed":true,"anyway":false}}}],
                 "chains":[{"chainId":100,"members":{"deployed":true,"owners":["$alice","$bob","$carol"],"threshold":2,"nonce":"3"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
    }

    private fun model(view: MsProposalsView) = MsProposalModel(view.items.single(), view, words(), null, "https://wallet.getvela.app", "en")

    private val plainReview = """{"sign":{"allowed":true,"anyway":false},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""

    // -- r8-3: every formatter total -------------------------------------------------

    @Test
    fun a_figure_that_is_not_a_number_is_said_as_unknown_never_a_crash() {
        val f = Formats(NumberFormatKey.CommaDot)
        assertEquals(Formats.UNKNOWN, f.fixed2(Double.NaN))
        assertEquals(Formats.UNKNOWN, f.fixed2(Double.POSITIVE_INFINITY))
        assertEquals(Formats.UNKNOWN, f.fixed2(Double.NEGATIVE_INFINITY))
        assertEquals("12.34", f.fixed2(12.34))
        val money = WalletLive.Money.of(CurrencyView(code = "USD", committed = true))
        assertEquals("no currency sign dressing up an unknown", Formats.UNKNOWN, money.fiat(Double.NaN))
        assertEquals(Formats.UNKNOWN, money.fiat(Double.POSITIVE_INFINITY))
        assertNull("the hero draws no figure for a total that is not a number", money.parts(Double.NaN))
        assertEquals("$1.50", money.fiat(1.5))
    }

    @Test
    fun the_multisig_send_form_with_a_price_that_is_not_a_number_draws_without_a_crash() {
        // The crash's path (SendLive.form → Money.fiat → Formats.fixed2): a
        // multisig's Send, USDC, 0.05, a recipient typed.
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val usdc = SendToken(network = "chain-100", chain_id = 100, symbol = "USDC", balance = "1.35", decimals = 6, token_address = "0x" + "36".repeat(20), price_usd = Double.NaN)
        val view = SendView(
            stage = SendStage.EnterDetails, selected_token = usdc, tokens = listOf(usdc),
            recipient = bob, amount = "0.05", token_amount = "0.05", multisig = SendMultisigRef(2, 3),
        )
        val form = SendLive.form(drawn.model, view, FeeView(), ctx())
        assertNotNull(form.amount)
        val inf = view.copy(selected_token = usdc.copy(price_usd = Double.POSITIVE_INFINITY))
        SendLive.form(drawn.model, inf, FeeView(), ctx())
    }

    @Test
    fun the_crash_sheet_says_calm_words_never_a_stack_trace() {
        val body = crashBody(strings)
        assertEquals(strings.t(I18nKeys.Flows.FEE_REASON_INTERNAL), body)
        assertFalse(body.contains("Exception"))
        assertFalse(body.contains("at app."))
    }

    // -- r8-18: the multisig send form offers no choice it does not have ---------------

    private fun ctx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = emptyMap(),
        money = WalletLive.Money.of(CurrencyView(code = "USD", committed = true)),
        fromName = "Team",
        fromAddress = safe,
    )

    @Test
    fun a_multisig_send_has_no_fee_coin_chip_and_no_speed() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "1", decimals = 18, price_usd = 1.0)
        val view = SendView(stage = SendStage.EnterDetails, selected_token = xdai, tokens = listOf(xdai), recipient = bob, amount = "0.1", multisig = SendMultisigRef(2, 3))
        val speed = SendLive.SpeedInputs(FeeSpeedView(tier = FeeTier.Fast, picked = true)) { null }
        val form = SendLive.form(drawn.model, view, FeeView(), ctx(), speed)
        assertFalse("no coin is named for a fee the multisig does not pay", form.fee.showMark)
        assertFalse("no chevron into the coins", form.fee.opens)
        // r9-15: who must approve stands where the fee would; who pays it is the note.
        assertEquals(strings.t("home.multisig.panel.title", mapOf("m" to "2", "n" to "3")), form.fee.value)
        assertEquals(strings.t("home.multisig.proposal.feeNote"), form.fee.staleNote)
        assertNull("no speed for a proposal", form.speed)
        // A Vela account's send keeps both.
        val own = SendLive.form(drawn.model, view.copy(multisig = null), FeeView(), ctx(), speed)
        assertTrue(own.fee.showMark)
        assertNotNull(own.speed)
    }

    // -- r8-18: a ready transaction is executed, not approved again -----------------------

    @Test
    fun a_ready_transactions_main_button_is_execute_never_approve() {
        val ready = model(proposals(approved = 2, canApprove = """["$alice"]""", review = plainReview))
        assertFalse("2/2: Approve is not offered", ready.showApprove)
        assertTrue(ready.isReady && ready.canExecute.isNotEmpty())
        val short = model(proposals(approved = 1, canApprove = """["$alice"]""", review = plainReview))
        assertTrue("1/2: Approve is offered", short.showApprove)
    }

    // -- r8-1: the core's sign rule decides the friction ----------------------------------

    @Test
    fun nothing_signs_before_the_review_is_on_screen() {
        val none = model(proposals(approved = 1, canApprove = """["$alice"]""", review = null))
        assertFalse(none.reviewReady)
        assertFalse("no review, no signature", none.canSign)
        val failed = model(proposals(approved = 1, canApprove = """["$alice"]""", review = "null"))
        assertFalse(failed.canSign)
        assertTrue(model(proposals(approved = 1, canApprove = """["$alice"]""", review = plainReview)).canSign)
    }

    @Test
    fun an_anyway_review_asks_again_with_its_danger_lines() {
        val danger = """{"sign":{"allowed":true,"anyway":true},"concerns":[{"kind":"blind"}],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
        val m = model(proposals(approved = 1, canApprove = """["$alice"]""", review = danger))
        assertTrue(m.canSign)
        assertTrue("allowed only through the … anyway friction", m.signAnyway)
        assertEquals(listOf(words().t("action.unsupported")), m.dangerLines)
        assertEquals(m.dangerLines, anywayLines(m.dangerLines, words()))
        // A refusal is no override at all.
        val refused = """{"sign":{"allowed":false,"anyway":false},"concerns":[],"recipients":[],"simulation":{"kind":"deltas","deltas":[]},"state":{"verdict":{"kind":"unchanged"}}}"""
        assertFalse(model(proposals(approved = 1, canApprove = """["$alice"]""", review = refused)).canSign)
        // With no danger line of its own, the confirmation still says why it asks.
        assertEquals(1, anywayLines(emptyList(), words()).size)
    }

    // -- r8-14: each concern said once ---------------------------------------------------

    @Test
    fun a_concern_another_block_says_is_not_said_again() {
        val review = json(
            """
            {"concerns":[{"kind":"unsimulated"},{"kind":"firstTime","address":"$bob"},{"kind":"firstTime","address":"$carol"},{"kind":"blind"},{"kind":"blind"}],
             "recipients":[{"address":"$bob","role":"recipient","firstTimeCue":true}],
             "simulation":{"kind":"unavailable","notice_key":"componentsUi.signing.simUnavailableWarning"}}
            """.trimIndent(),
        )
        val shown = shownConcerns(review).map { it.kind to (it as JsonObject)["address"]?.toString()?.trim('"') }
        assertEquals(
            "unsimulated (the moves block's notice) and Bob's first time (his row's chip) go; Carol's, with no row, stays; blind once",
            listOf("firstTime" to carol, "blind" to null),
            shown,
        )
        // Nothing else says it: kept.
        val bare = json("""{"concerns":[{"kind":"unsimulated"}],"recipients":[],"simulation":{"kind":"deltas","deltas":[]}}""")
        assertEquals(1, shownConcerns(bare).size)
    }

    // -- r8-19: "Checking again…" is bounded --------------------------------------------

    @Test
    fun the_review_spinner_ends() {
        // §12: the spinner's bound is the core's (`failed {timedOut}`); the
        // sheet draws the door: not asked yet — loading; not readable — failed,
        // with Try again; read — the review.
        assertTrue(model(proposals(approved = 1, canApprove = """["$alice"]""", review = null)).door.loading)
        val failed = model(proposals(approved = 1, canApprove = """["$alice"]""", review = "null")).door
        assertTrue("could not be gathered: failed, with Try again", failed.failed && failed.retry)
        assertTrue(model(proposals(approved = 1, canApprove = """["$alice"]""", review = plainReview)).door.showsReview)
        // Read, but a chain read did not come back: asked again soon. Failed is
        // never asked again behind the person's back (it waits for Try again).
        assertFalse(reviewUnread(JsonNull))
        assertTrue(reviewUnread(json("""{"state":{"verdict":{"kind":"unavailable"}},"simulation":{"kind":"deltas"}}""")))
        assertFalse(reviewUnread(null))
        assertFalse(reviewUnread(json(plainReview)))
    }

    // -- r8-8: the outside-wallet path ----------------------------------------------------

    @Test
    fun an_outside_member_gets_the_connect_link_and_vela_members_the_vela_link() {
        val split = """{"rows":[{"owner":"$bob","kind":"vela"},{"owner":"$carol","kind":"wallet"}],"in_vela":["$bob"],"outside":["$carol"],"either":[],"contract":[],"enough":false}"""
        val m = model(proposals(approved = 1, canApprove = "[]", review = plainReview, signers = split))
        assertEquals(listOf(carol), m.outsideMembers)
        assertTrue(m.outsideCanSign)
        val link = m.outsideLink
        assertNotNull(link)
        assertTrue(link!!, link.startsWith("https://wallet.getvela.app/en/connect#"))
        assertTrue("names the outside member", link.contains(carol.lowercase()))
        assertTrue("names the transaction", link.contains(hash.lowercase()))
        assertTrue("Bob approves in Vela: the Vela link", m.showVelaLink)

        // Only an outside member still to approve: the Vela link is not offered to them.
        val onlyOutside = """{"rows":[{"owner":"$bob","kind":"wallet"},{"owner":"$carol","kind":"wallet"}],"in_vela":[],"outside":["$bob","$carol"],"either":[],"contract":[],"enough":false}"""
        val o = model(proposals(approved = 1, canApprove = "[]", review = plainReview, signers = onlyOutside))
        assertFalse(o.showVelaLink)
        assertNotNull(o.outsideLink)

        // Nothing read yet: the Vela link, and no outside link.
        val unread = model(proposals(approved = 1, canApprove = "[]", review = plainReview))
        assertNull(unread.outsideLink)
        assertTrue(unread.showVelaLink)
    }

    @Test
    fun a_transaction_link_on_a_device_with_no_account_is_the_connect_page() {
        val page = "https://wallet.getvela.app/en/connect"
        val link = waitingConnectLink("#ms=100.$safe.$hash", page)
        assertNotNull(link)
        assertTrue(link!!, link.startsWith("$page#"))
        assertTrue(link.contains(hash.lowercase()))
        assertNull("not a transaction link: Welcome keeps it", waitingConnectLink("#nothing", page))
    }
}
