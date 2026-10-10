package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.assertNull
import app.getvela.wallet.feature.send.core.SendReceiptOutcome
import app.getvela.wallet.feature.send.core.SendReceiptOutcomes
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.send.core.TrackOutcome
import app.getvela.wallet.feature.send.core.TrackStatus
import org.junit.Test

/**
 * The send controller over the REAL send core (the bridge, not a fake), with
 * the relay, the RPC pool and storage faked. What these pin is the shell's
 * half of a contract the core tests cannot see: the ids and events Android
 * hands the core.
 */
class SendControllerTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private class NoEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun token(symbol: String, chainId: Int, balance: String, address: String? = null) = BalanceToken(
        chain_id = chainId,
        symbol = symbol,
        name = symbol,
        balance = balance,
        decimals = if (address == null) 18 else 6,
        token_address = address,
        price_usd = 1.0,
    )

    private fun row(chainId: Long, symbol: String, name: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
    )

    private val holdings = BalanceView(
        tokens = listOf(
            token("USDT", 1, "53.48", "0xdac17f958d2ee523a2206206994597c13d831ec7"),
            token("POL", 137, "19.194439"),
            token("ETH", 42161, "0.002"),
        ),
    )

    private fun controller(): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val store = FakeStore()
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = NoEndpoints(),
            scope = scope,
            transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
        )
        return SendController(
            scope = scope,
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accountStore = AccountStore(store),
            balances = { holdings },
            networks = { NetView(loaded = true, networks = listOf(row(1, "ETH", "Ethereum"), row(137, "POL", "Polygon"), row(42161, "ETH", "Arbitrum"))) },
            signer = { error("no signing in these tests") },
            haptic = {},
            refreshBalances = {},
        )
    }

    private val me = SendAccountRef(id = ME, address = ME, name = "Me")
    private val usd = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)

    private fun SendController.awaitView(what: String, test: (SendView) -> Boolean): SendView = runBlocking {
        withTimeout(10_000) { send.first(test) }.also { assertEquals(what, true, test(it)) }
    }

    /**
     * Issue #268: a token detail's Send opens on the form for THAT token. The
     * token rides into the open as the web's preselection, in the id format
     * the executor gives the core's token list (`chain-<id>`).
     */
    @Test
    fun `a token detail's send opens on the form for that token`() {
        val send = controller()
        send.open(
            account = me,
            display = usd,
            params = SendOpenParams(preselected_symbol = "POL", preselected_network = SendExecutor.network(137)),
        )

        val view = send.awaitView("on the form with POL") { it.stage == SendStage.EnterDetails && it.selected_token?.symbol == "POL" }
        assertEquals(137, view.selected_token?.chain_id)
    }

    /**
     * PR 3 final note F25: the Send machine is told the money it shows once,
     * at open — and `displayChanged` had no caller. A Send opened on a cold
     * start, before the display currency committed, kept the placeholder
     * (dollars with no rate: nothing to type a fiat amount against) for as
     * long as it stayed open. The screen hands over the display on every
     * change now; this says it to an open Send when it is news, and the real
     * machine re-denominates by its own rule.
     */
    @Test
    fun `a send opened before the display currency commits follows it when it does`() {
        val send = controller()
        val placeholder = SendDisplayContext(code = "USD", rate = null, fiat_decimals = 2)
        val cny = SendDisplayContext(code = "CNY", rate = 7.1, fiat_decimals = 2)

        // No Send is open: nobody to tell.
        assertFalse(send.displayChanged(cny))

        send.open(
            account = me,
            display = placeholder,
            params = SendOpenParams(preselected_symbol = "POL", preselected_network = SendExecutor.network(137)),
        )
        val before = send.awaitView("on the form, no rate to type fiat against") { it.stage == SendStage.EnterDetails && it.selected_token?.symbol == "POL" }
        assertFalse("the placeholder has no rate: ⇄ is shut", before.denom_toggle_enabled)
        // The same display again is not news.
        assertFalse(send.displayChanged(placeholder))

        // The currency commits while the form is open.
        assertTrue(send.displayChanged(cny))
        send.awaitView("⇄ opens once the machine knows the rate") { it.denom_toggle_enabled }
        send.toggleFiatInput()
        val fiat = send.awaitView("the amount is typed in the person's currency") { it.amount_fiat_code != null }
        assertEquals("CNY", fiat.amount_fiat_code)
        assertFalse("said once", send.displayChanged(cny))

        // The person changes it while the Send is open: the figure does not keep the old unit.
        send.setAmount("50")
        send.awaitView("50 CNY typed") { it.amount == "50" }
        assertTrue(send.displayChanged(SendDisplayContext(code = "EUR", rate = 0.9, fiat_decimals = 2)))
        val moved = send.awaitView("re-denominated by the core's rule") { it.amount_fiat_code == "EUR" }
        assertEquals("50 CNY is not 50 EUR: the field empties", "", moved.amount)

        // Left: the journey is over, and the next open carries its own display.
        send.left()
        assertFalse(send.displayChanged(cny))
        send.open(account = me, display = cny)
        assertFalse("told by its own open", send.displayChanged(cny))
    }

    /**
     * The device's version of #268: a send opened and closed earlier leaves
     * its token list on the machine. The shell used to pick against THAT list
     * before the new open reset it to the picker. With the preselection in the
     * open itself, a second send from a token detail still lands on the form.
     */
    @Test
    fun `a second send from a token detail still lands on the form`() {
        val send = controller()
        send.open(account = me, display = usd)
        send.awaitView("the first send lists the holdings") { it.stage == SendStage.SelectToken && it.tokens.size == 3 }

        send.open(
            account = me,
            display = usd,
            params = SendOpenParams(preselected_symbol = "ETH", preselected_network = SendExecutor.network(42161)),
        )

        val view = send.awaitView("on the form with Arbitrum ETH") { it.stage == SendStage.EnterDetails && it.selected_token?.chain_id == 42161 }
        assertEquals("ETH", view.selected_token?.symbol)
    }

    /**
     * Issue #332, the phone's own order: Home → Scan opens Send on the picker
     * with the scanner over it, and the code arrives afterwards. The address
     * it read is the recipient on the picker, the form opens on it once a
     * token is chosen, and going back to choose another keeps it.
     */
    @Test
    fun `a code scanned from the home is the recipient on the picker and survives back`() {
        val send = controller()
        send.open(account = me, display = usd)
        send.awaitView("the picker lists the holdings") { it.stage == SendStage.SelectToken && it.tokens.size == 3 }
        send.openScanner()
        send.scanned("  $PAYEE ")

        val picking = send.awaitView("the picker, with the scanned recipient") {
            it.stage == SendStage.SelectToken && !it.show_scanner && it.recipient == PAYEE
        }
        assertNull(picking.selected_token)

        val pol = picking.tokens.first { it.symbol == "POL" }
        send.selectToken(app.getvela.wallet.feature.send.SendLive.tokenId(pol))
        send.awaitView("the form, for them") { it.stage == SendStage.EnterDetails && it.recipient == PAYEE }

        send.back()
        send.awaitView("back on the picker, still for them") {
            it.stage == SendStage.SelectToken && it.selected_token == null && it.recipient == PAYEE
        }
    }

    /**
     * Issue #312, the phone's order: a code that names a network
     * (`ethereum:<payee>@137`) offers only what the payer holds THERE — one
     * holding opens the form on it, never the balance's top coin on another
     * chain — and the token card (issue #326) goes back to that network's
     * picker with the payee kept.
     */
    @Test
    fun `a code naming a network offers only that network and the token card keeps it`() {
        val send = controller()
        send.open(account = me, display = usd)
        send.awaitView("the picker lists the holdings") { it.stage == SendStage.SelectToken && it.tokens.size == 3 }
        send.openScanner()
        send.scanned("ethereum:$PAYEE@137")

        val form = send.awaitView("the form, on the one Polygon holding") {
            it.stage == SendStage.EnterDetails && it.selected_token?.chain_id == 137
        }
        assertEquals("POL", form.selected_token?.symbol)
        assertEquals(PAYEE, form.recipient)
        assertEquals(137, form.request_chain_id)
        assertEquals(listOf(137), form.tokens.map { it.chain_id })
        assertEquals(true, form.can_change_token)

        send.changeToken()
        val picker = send.awaitView("that network's picker, for them") { it.stage == SendStage.SelectToken }
        assertEquals(listOf(137), picker.tokens.map { it.chain_id })
        assertEquals(PAYEE, picker.recipient)
        assertNull(picker.selected_token)
    }

    /**
     * Issue #467, the split's 从通讯录 pill: it opens the book for the split
     * as a whole (no row named), and the pick fills the first row with no
     * address yet — else a new row at the end. It used to land in the single
     * form's recipient, which a split hides and never pays. The rule is the
     * core's; these are the calls the pill and the book's rows make.
     */
    @Test
    fun `a pick from the split's contacts pill fills the first free row, else a new one`() {
        val send = controller()
        send.open(
            account = me,
            display = usd,
            params = SendOpenParams(preselected_symbol = "POL", preselected_network = SendExecutor.network(137)),
        )
        send.awaitView("on the form with POL") { it.stage == SendStage.EnterDetails && it.selected_token?.symbol == "POL" }
        send.setRecipient(PAYEE)
        send.awaitView("the payee typed") { it.recipient == PAYEE }
        send.enterSplit()
        val split = send.awaitView("the split, as [the payee, a blank]") { it.split_mode && it.recipients.size == 2 }
        assertEquals(PAYEE, split.recipients[0].address)
        assertEquals("", split.recipients[1].address)

        // The pill: the book for the split, no row named.
        send.openContactPicker()
        send.awaitView("the book is up over the split") { it.show_contact_picker && it.split_mode }
        send.pickedAddress(FRIEND)
        val first = send.awaitView("the pick took the blank row") { !it.show_contact_picker && it.recipients.getOrNull(1)?.address == FRIEND }
        assertEquals("no row is added while one is free", 2, first.recipients.size)
        assertEquals(split.recipients[0], first.recipients[0])
        assertEquals("the blank row is the one filled", split.recipients[1].id, first.recipients[1].id)
        assertEquals("the hidden single recipient is not where it went", PAYEE, first.recipient)
        assertEquals(app.getvela.wallet.feature.flows.FlowState.SD2, app.getvela.wallet.feature.send.SendLive.flowState(first, feeSheetOpen = false))

        // Every row taken: the next pick is a new row at the end.
        send.openContactPicker()
        send.awaitView("the book is up again") { it.show_contact_picker }
        send.pickedAddress(THIRD)
        val second = send.awaitView("a third row") { !it.show_contact_picker && it.recipients.size == 3 }
        assertEquals(first.recipients, second.recipients.take(2))
        assertEquals(THIRD, second.recipients[2].address)
        assertEquals("", second.recipients[2].amount)
        assertEquals(PAYEE, second.recipient)
    }

    /**
     * Spec 082 (T128): what a tracker entry means for the send on screen is
     * the core's one mapping (`sendReceiptOutcomeOf`) — this app's own `when`
     * over four statuses is gone. A lost reply the relay has not shown it
     * holds says nothing; once it does, the receipt is "submitted" again; the
     * relay never having it is "not sent", never the fee-refused words.
     */
    @Test
    fun `a tracker entry reaches the send receipt in the core's words`() {
        fun entry(status: TrackStatus, outcome: TrackOutcome, tx: String? = null) =
            TrackEntryView(user_op_hash = "0x" + "7a".repeat(32), chain_id = 100, status = status, tx_hash = tx, outcome = outcome)
        assertNull("in doubt: nothing yet", SendReceiptOutcomes.of(entry(TrackStatus.Pending, TrackOutcome.MaybeSent)))
        assertEquals(SendReceiptOutcome.Acknowledged, SendReceiptOutcomes.of(entry(TrackStatus.Pending, TrackOutcome.Landing)))
        assertEquals(SendReceiptOutcome.Failed(rejected = false, not_sent = true), SendReceiptOutcomes.of(entry(TrackStatus.NotSent, TrackOutcome.Final)))
        assertEquals(SendReceiptOutcome.Failed(rejected = true), SendReceiptOutcomes.of(entry(TrackStatus.Rejected, TrackOutcome.Final)))
        assertEquals(SendReceiptOutcome.Confirmed("0xtx"), SendReceiptOutcomes.of(entry(TrackStatus.Confirmed, TrackOutcome.Final, "0xtx")))
        assertEquals(SendReceiptOutcome.FeeHeld, SendReceiptOutcomes.of(entry(TrackStatus.FeeHeld, TrackOutcome.Landing)))
        assertNull("slow is not failed (invariant 5)", SendReceiptOutcomes.of(entry(TrackStatus.Unreachable, TrackOutcome.Unknown)))
    }

    private companion object {
        const val ME = "0x576a2cc9e6adc0c95989fa6aa104290aa940c73f"
        const val PAYEE = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
        const val FRIEND = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        const val THIRD = "0xdddddddddddddddddddddddddddddddddddddddd"
    }
}
