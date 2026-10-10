package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.SendNameSource
import app.getvela.wallet.feature.send.core.SendPayee
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SendRecipientIdentity
import app.getvela.wallet.feature.send.core.SendRowFieldState
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.SplitRows
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SendCore

/**
 * The split form's verdicts, on the real `send` machine (issues 203–206).
 *
 * The wire is the risk here, not the wording: `ordinal` and `first_ordinal`
 * are `u32`, and a Kotlin `Double` there would be refused by serde only when a
 * real core answers (gotcha ②). So the rows go through the bridge, the view
 * comes back through it, and the builder words what came back.
 */
class SendSplitVerdictsTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private val strings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "0.71697", decimals = 18, price_usd = 1.0)

    /**
     * The send core with a shell that holds one coin and never finishes a fee
     * quote; [identity] is what its resolver says of any recipient.
     */
    private fun host(identity: SendRecipientIdentity? = null): CoreHost<SendView> {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        return CoreHost(
            bridge = SendCore().asBridge(),
            scope = scope,
            initial = SendView(),
            serializer = SendView.serializer(),
            perform = JsonShell.perform(SendOperation.serializer(), SendShellResult.serializer()) { operation ->
                when (operation) {
                    is SendOperation.FetchTokens -> SendShellResult.TokensLoaded(
                        tokens = listOf(xdai),
                        chains = listOf(SendChainInfo(100, "chain-100", "XDAI")),
                    )
                    is SendOperation.ResolveIdentity -> SendShellResult.IdentityResolved(identity)
                    is SendOperation.ResolveRisk -> SendShellResult.RiskResolved(null)
                    is SendOperation.Haptic -> SendShellResult.HapticPlayed
                    is SendOperation.ShowAlert -> SendShellResult.AlertAcknowledged
                    // Fees, probes, timers: left in flight — the verdicts under
                    // test are the rows', not the pre-check's.
                    else -> awaitCancellation()
                }
            },
            escapedFailure = JsonShell.escapedFailure(
                SendOperation.serializer(),
                SendShellResult.serializer(),
                fallback = SendShellResult.Closed,
                answer = { SendShellResult.Closed },
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<SendView>.settle(predicate: (SendView) -> Boolean): SendView =
        runBlocking { withTimeout(TIMEOUT) { view.first(predicate) } }

    private fun CoreHost<SendView>.send(event: SendEvent) = dispatch(event, SendEvent.serializer())

    private fun splitWith(rows: List<SendRecipientDraft>): CoreHost<SendView> {
        val h = host()
        h.send(SendEvent.Open(account = SendAccountRef("acct", ME), display = SendDisplayContext("USD", 1.0, 2)))
        // The id the core LISTS: it spells a built-in chain's coin the
        // registry's way (xDAI), and the id carries the symbol.
        val listed = h.settle { it.tokens.isNotEmpty() }
        h.send(SendEvent.SelectToken(SendLive.tokenId(listed.tokens.single())))
        h.settle { it.selected_token != null }
        h.send(SendEvent.EnterSplitMode)
        h.settle { it.split_mode }
        h.send(SendEvent.RecipientsChanged(rows))
        return h
    }

    @Test
    fun theCoreSaysWhichRowIsUnfinishedAndWhichRepeats() {
        val h = splitWith(
            listOf(
                SendRecipientDraft("", PAYEE, "0.1"),
                SendRecipientDraft("", PAYEE.lowercase(), "0.2"),
                SendRecipientDraft("", "0x1234", "1,5"),
                SendRecipientDraft("", "", ""),
            ),
        )
        val view = h.settle { it.recipients.size == 4 && it.split_row_issues.isNotEmpty() }

        assertFalse(view.can_continue)
        assertEquals(listOf(3, 4), view.split_row_issues.map { it.ordinal })
        assertEquals(SendRowFieldState.Invalid, view.split_row_issues[0].address)
        assertEquals(SendRowFieldState.Invalid, view.split_row_issues[0].amount)
        assertEquals(SendRowFieldState.Empty, view.split_row_issues[1].address)
        assertEquals(listOf(view.recipients[1].id), view.split_duplicates.map { it.id })
        assertEquals(1, view.split_duplicates.single().first_ordinal)

        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val live = SendLive.form(drawn.model, view, FeeView(), ctx())
        assertEquals("Recipient 3 needs an address.", live.hint)
        assertEquals("Same address as recipient 1", live.recipients[1].duplicateNote)
        assertNull(live.recipients[0].duplicateNote)
    }

    @Test
    fun theCoreSaysWhatIsLeftAndWhenTheRowsAreOver() {
        val h = splitWith(listOf(SendRecipientDraft("", PAYEE, "0.5"), SendRecipientDraft("", ME, "0.5")))
        val over = h.settle { it.recipients.size == 2 && it.split_over_balance }
        assertNull(over.split_remaining)
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val overLive = SendLive.form(drawn.model, over, FeeView(), ctx())
        assertTrue(overLive.summary!!.over)
        assertEquals("The total exceeds your balance.", overLive.warning)

        h.send(SendEvent.RecipientsChanged(over.recipients.mapIndexed { i, row -> row.copy(amount = if (i == 0) "0.1" else "0.2") }))
        val under = h.settle { !it.split_over_balance && it.split_remaining != null }
        assertEquals("0.41697", under.split_remaining)
        assertEquals("0.41697 xDAI left", SendLive.form(drawn.model, under, FeeView(), ctx()).summary?.remaining)
    }

    /** "Use X for the empty rows", end to end: the core flags the empty row, the offer copies the typed figure, the core accepts it. */
    @Test
    fun theEmptyRowsTakeTheTypedFigure() {
        val h = splitWith(listOf(SendRecipientDraft("", PAYEE, "0.1"), SendRecipientDraft("", ME, "")))
        // Past the two blank rows split mode opens with: the typed list, judged.
        val gap = h.settle { view -> view.recipients.any { it.amount == "0.1" } && view.split_row_issues.any { it.amount == SendRowFieldState.Empty } }
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val fill = SendLive.form(drawn.model, gap, FeeView(), ctx()).fillEmpty
        assertEquals("Use 0.1 xDAI for the empty rows", fill?.label)

        h.send(SendEvent.RecipientsChanged(SplitRows.emptyFilled(gap.recipients, fill!!.amount)))
        val filled = h.settle { view -> view.recipients.all { it.amount == "0.1" } }
        assertEquals(gap.recipients.map { it.id }, filled.recipients.map { it.id })
        assertTrue(filled.split_row_issues.isEmpty())
        assertEquals("0.2", filled.confirm_amount)
        assertNull(SendLive.form(drawn.model, filled, FeeView(), ctx()).fillEmpty)
    }

    /**
     * Spec 097 F (S2), end to end: the resolver answers as Android's does for
     * the public registry (`source = "passkey"`); the core names the payee and
     * whose word the name is, and the form line and the confirm's To row draw
     * "Wallet · Vela User" with the address — never "passkey", never the name
     * alone.
     */
    @Test
    fun theCoreNamesTheRegistryPayeeAndTheScreensKeepTheAddress() {
        val h = host(SendRecipientIdentity(name = "Wallet", source = "passkey"))
        h.send(SendEvent.Open(account = SendAccountRef("acct", ME), display = SendDisplayContext("USD", 1.0, 2)))
        val listed = h.settle { it.tokens.isNotEmpty() }
        h.send(SendEvent.SelectToken(SendLive.tokenId(listed.tokens.single())))
        h.settle { it.selected_token != null }
        h.send(SendEvent.SetRecipient(WALLET))
        val view = h.settle { it.payees.singleOrNull()?.name != null }

        assertEquals(SendPayee(WALLET, "Wallet", SendNameSource.Registry), view.payees.single())

        val form = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val note = SendLive.form(form.model, view, FeeView(), ctx()).recipient!!.note
        assertEquals("Wallet · Vela User", note)
        assertFalse(note!!.contains("passkey"))

        val confirm = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val to = SendLive.confirm(confirm.model, view, ctx()).facts.first { it.label == strings.t(I18nKeys.Flows.TO_LABEL) }
        // The name may be cut; the tag rides with the address on the line that never is.
        assertEquals("Wallet", to.value)
        assertEquals("Vela User · 0x14fB…eA5c", to.detail)
    }

    private fun ctx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = emptyMap(),
        money = WalletLive.Money.of(CurrencyView(code = "USD", committed = true)),
        fromName = "Me",
        fromAddress = ME,
    )

    private companion object {
        const val ME = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val PAYEE = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
        const val WALLET = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        const val TIMEOUT = 20_000L
    }
}
