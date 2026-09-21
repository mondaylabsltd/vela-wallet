package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
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
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SendRowFieldState
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
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

    /** The send core with a shell that holds one coin and never finishes a fee quote. */
    private fun host(): CoreHost<SendView> {
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
                    is SendOperation.ResolveIdentity -> SendShellResult.IdentityResolved(null)
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
        h.settle { it.tokens.isNotEmpty() }
        h.send(SendEvent.SelectToken(SendLive.tokenId(xdai)))
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
        assertEquals("0.41697 XDAI left", SendLive.form(drawn.model, under, FeeView(), ctx()).summary?.remaining)
    }

    private fun ctx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = emptyMap(),
        money = WalletLive.Money.of(CurrencyView(code = "USD")),
        fromName = "Me",
        fromAddress = ME,
    )

    private companion object {
        const val ME = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val PAYEE = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
        const val TIMEOUT = 20_000L
    }
}
