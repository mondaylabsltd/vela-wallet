package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.SendConfirmModel
import app.getvela.wallet.feature.flows.SendFormModel
import app.getvela.wallet.feature.flows.TokenDetailModel
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceCacheEntry
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.UnreachableNetwork
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * No fiat figure before the display currency commits — on EVERY surface.
 *
 * The core's rule (`rust/crates/vela-core/src/app/display_currency.rs`, its
 * module doc and `FIAT_SURFACES`): while `CurrencyView.committed` is false
 * the view is the USD/1 placeholder, which is not the person's currency, and
 * no fiat figure is drawn anywhere — not "$1,234" that becomes "¥8,876" a few
 * seconds later, and not a "$" on one screen while another waits. A token
 * amount ("0.5 ETH") is not fiat and is drawn as always. And withholding
 * never moves the layout: a withheld figure keeps its room.
 *
 * This shell cannot import the core's list, so the twelve names are pinned
 * here, one test each, in the core's order:
 *
 *     home_total, holdings, account_switcher, token_detail, assets,
 *     balance_detail, activity_row, activity_detail, send_coin_list,
 *     send_form, signing_sheet, settings_total
 *
 * Every figure goes through ONE helper (`WalletLive.Money.fiat` / `.parts`),
 * which answers `null` while the currency is not committed; each test below
 * builds its surface with the placeholder (a cold start with CNY stored) and
 * again committed, and checks: waiting, nothing drawn carries a currency
 * sign, the figure's digits or the word "null", and the figure's room is
 * kept; committed, the figure is there in the person's money.
 */
class FiatWithheldTest {
    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val strings: VelaStrings by lazy {
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    /** The core's `FIAT_SURFACES`, in its order. A surface added there needs a test here. */
    private val surfaces = listOf(
        "home_total", "holdings", "account_switcher", "token_detail", "assets", "balance_detail",
        "activity_row", "activity_detail", "send_coin_list", "send_form", "signing_sheet", "settings_total",
    )

    private val waiting = CurrencyView(code = "USD", rate = null, committed = false, pending = "CNY")
    private val committed = CurrencyView(code = "CNY", rate = 7.1, committed = true)
    private val chains = mapOf(1 to "Ethereum", 100 to "Gnosis", 137 to "Polygon")
    private val usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
    private val me = "0x" + "ab".repeat(20)

    /** 418.25 xDAI and 376.54 USDC at $1: $794.79 — at 7.1, ¥2,969.57 and ¥2,673.43, ¥5,643.01 in all. */
    private val view = BalanceView(
        address = me,
        display_total_usd = 794.79,
        tokens = listOf(
            BalanceToken(chain_id = 100, symbol = "xDAI", name = "xDAI", balance = "418.25", decimals = 18, price_usd = 1.0),
            BalanceToken(chain_id = 1, symbol = "USDC", name = "USDC", balance = "376.54", decimals = 6, token_address = usdc, price_usd = 1.0),
        ),
    )

    /** Not one drawn string holds a fiat figure: no currency sign, none of the figure's digit runs, and never "null". */
    private fun assertNoFiat(surface: String, drawn: List<String?>, vararg figures: String) {
        for (text in drawn.filterNotNull()) {
            assertFalse("$surface draws a dollar sign while the currency is on its way: \"$text\"", text.contains('$'))
            assertFalse("$surface draws a yen sign while the currency is on its way: \"$text\"", text.contains('¥'))
            assertFalse("$surface formatted a withheld figure as text: \"$text\"", text.contains("null"))
            assertFalse("$surface draws \"≈\" with nothing after it: \"$text\"", text.trimEnd().endsWith("≈"))
            for (figure in figures) assertFalse("$surface draws the figure $figure: \"$text\"", text.contains(figure))
        }
    }

    @Test
    fun `the pinned surfaces are the core's twelve`() {
        val core = File(root, "rust/crates/vela-core/src/app/display_currency.rs").readText()
        val block = core.substringAfter("pub const FIAT_SURFACES: [&str; 12] = [").substringBefore("];")
        val named = Regex("\"([a-z_]+)\"").findAll(block).map { it.groupValues[1] }.toList()
        assertEquals("the core's FIAT_SURFACES changed: add (or drop) the surface's test here", named, surfaces)
    }

    /** The one helper: `null` while the currency is not the person's, in either form. */
    @Test
    fun `the helper withholds every figure until the currency commits`() {
        val money = WalletLive.Money.of(waiting)
        assertFalse(money.settled)
        assertNull(money.fiat(794.79))
        assertNull(money.parts(794.79))
        assertEquals("the label may name the choice on its way", "CNY", money.label)
        val landed = WalletLive.Money.of(committed)
        assertEquals("CN¥5,643.01", landed.fiat(794.79))
        assertEquals("CN¥5,643" to "01", landed.parts(794.79)!!.let { it.integer to it.decimals })
        // Before the preference is even read there is nothing to name either.
        assertNull(WalletLive.Money.of(CurrencyView(code = "USD", rate = null, committed = false)).fiat(1.0))
    }

    // -- home_total ---------------------------------------------------------------

    @Test
    fun home_total() {
        val base = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)
        val wait = WalletLive.home(base, view, FeedView(), waiting, strings, chains).balance
        assertEquals("the total waits as a total still being read waits", BalanceStateKind.Loading, wait.state)
        assertNoFiat("home_total", listOf(wait.integer, wait.decimals, wait.liveText, wait.status?.text), "794", "5,643")
        val landed = WalletLive.home(base, view, FeedView(), committed, strings, chains).balance
        assertEquals(BalanceStateKind.Normal, landed.state)
        assertEquals("CN¥5,643" to "01", landed.integer to landed.decimals)
    }

    // -- holdings -----------------------------------------------------------------

    @Test
    fun holdings() {
        val wait = WalletLive.assetRows(view, chains, waiting)
        assertTrue("each worth keeps its line", wait.all { it.fiat == AssetFiatModel.Loading })
        assertEquals("what is held is not fiat: drawn as always", listOf("418.25 xDAI", "376.54 USDC"), wait.map { it.balance })
        val landed = WalletLive.assetRows(view, chains, committed)
        assertEquals(listOf(AssetFiatModel.Value("CN¥2,969.57"), AssetFiatModel.Value("CN¥2,673.43")), landed.map { it.fiat })
    }

    // -- account_switcher ---------------------------------------------------------

    private val switcher = BalanceSwitcherView(balances = listOf(BalanceCacheEntry(me, 794.79)))

    @Test
    fun account_switcher() {
        val wait = WalletLive.accountSwitcher(listOf("Main" to me), 0, switcher, waiting, strings)
        assertNoFiat("account_switcher", listOf(wait.summary) + wait.rows.map { it.amount }, "794", "5,643")
        val count = strings.t(app.getvela.wallet.core.i18n.I18nKeys.SettingsUi.ACCOUNTS_COUNT, 1).trimEnd(' ', '·')
        assertEquals("the count alone, still one line", count, wait.summary)
        val landed = WalletLive.accountSwitcher(listOf("Main" to me), 0, switcher, committed, strings)
        assertEquals("CN¥5,643.01", landed.rows.single().amount)
        assertTrue(landed.summary, landed.summary.contains("CN¥5,643.01"))
    }

    // -- token_detail (the worth AND the price) -----------------------------------

    private fun tokenPage(currency: CurrencyView): TokenDetailModel {
        val fallback = (FlowFixtures.build(FlowState.T2, strings).sheet as FlowSheet.TokenDetail).model
        return FlowLive.tokenDetail(fallback, view, FeedView(), WalletLive.holdingId(1, usdc), chains, currency, strings)!!
    }

    @Test
    fun token_detail() {
        val wait = tokenPage(waiting)
        assertEquals("376.54 USDC", wait.balance)
        assertEquals("the worth: empty, its line kept", "" to true, wait.fiat to wait.fiatWithheld)
        val price = wait.facts.first()
        assertEquals("the price: empty, its room kept", "" to true, price.value to price.withheld)
        assertNoFiat("token_detail", listOf(wait.fiat) + wait.facts.flatMap { listOf(it.value) + it.lines }, "2,673", "7.10")

        val landed = tokenPage(committed)
        assertEquals("CN¥2,673.43" to false, landed.fiat to landed.fiatWithheld)
        assertEquals("1 USDC = CN¥7.10" to false, landed.facts.first().value to landed.facts.first().withheld)
        // Nothing else on the page changed: same rows, same labels, in the same order.
        assertEquals(wait.facts.map { it.label }, landed.facts.map { it.label })
        assertEquals(wait.facts.drop(1), landed.facts.drop(1))
        // The boards are this pair.
        val board = (FlowFixtures.build(FlowState.T2W, strings).sheet as FlowSheet.TokenDetail).model
        assertTrue(board.fiatWithheld && board.facts.first().withheld)
        assertFalse((FlowFixtures.build(FlowState.T2C, strings).sheet as FlowSheet.TokenDetail).model.fiatWithheld)
    }

    // -- assets -------------------------------------------------------------------

    @Test
    fun assets() {
        val fallback = (FlowFixtures.build(FlowState.T1, strings).base as FlowBase.Assets).model
        val wait = FlowLive.assets(fallback, view, chains, waiting)
        assertTrue(wait.rows.isNotEmpty() && wait.rows.all { it.fiat == AssetFiatModel.Loading })
        val landed = FlowLive.assets(fallback, view, chains, committed)
        assertTrue(landed.rows.all { (it.fiat as AssetFiatModel.Value).text.startsWith("CN¥") })
        assertEquals(wait.rows.map { it.balance }, landed.rows.map { it.balance })
    }

    // -- balance_detail (the sheet, its unpriced list, the unreachable list's "last seen") --

    @Test
    fun balance_detail() {
        val fallback = SettingsFixtures.buildState(SettingsScreenState.ST1, strings).balanceDetail
        val odd = BalanceToken(chain_id = 137, symbol = "ODD", name = "Odd", balance = "1.5", decimals = 18, token_address = "0xodd")
        val whole = view.copy(tokens = view.tokens + odd, unpriced_tokens = listOf(odd))

        val wait = SettingsLive.balanceDetail(fallback, whole, waiting, chains, strings)
        assertEquals("the total: not said, its line kept", "" to true, wait.summary to wait.summaryWithheld)
        assertTrue("each network's worth: its room kept", wait.done.isNotEmpty() && wait.done.all { it.amount == null && it.amountWithheld })
        // An unpriced holding's line is a TOKEN amount: never withheld.
        assertEquals("Polygon · 1.5", wait.unpriced.single().status)
        assertNoFiat("balance_detail", listOf(wait.summary) + (wait.pending + wait.done + wait.unpriced).flatMap { listOf(it.amount, it.status) }, "794", "418.25", "2,969")
        // (A chain whose only holding is unpriced settled too: it is listed, worth nothing counted.)

        val landed = SettingsLive.balanceDetail(fallback, whole, committed, chains, strings)
        assertEquals("Total CN¥5,643.01" to false, landed.summary to landed.summaryWithheld)
        assertEquals(listOf("CN¥2,969.57", "CN¥2,673.43", "CN¥0.00"), landed.done.map { it.amount })
        assertEquals("the same networks, in the same order", wait.done.map { it.name }, landed.done.map { it.name })

        // The unreachable list: "Last seen …" is a worth — its line is not
        // said while the currency is on its way; a line with no figure is.
        val down = BalanceView(
            unreachable_networks = listOf(
                UnreachableNetwork(1, "held", 4_500.0, "assets.lastSeen"),
                UnreachableNetwork(137, "not_read", null, "assets.notReadYet"),
            ),
            unreachable_key = "assets.unreachableMany",
        )
        val list = SettingsLive.unreachable(down, waiting, chains, strings)
        assertEquals("" to true, list.rows[0].line to list.rows[0].lineWithheld)
        assertEquals("Not read yet" to false, list.rows[1].line to list.rows[1].lineWithheld)
        assertNoFiat("balance_detail (unreachable)", list.rows.map { it.line } + list.title, "4,500", "31,950")
        assertEquals("Last seen CN¥31,950.00", SettingsLive.unreachable(down, committed, chains, strings).rows[0].line)
        // Hidden is not "withheld": the mask, said at once.
        assertEquals("Last seen ••••", SettingsLive.unreachable(down.copy(hidden = true), waiting, chains, strings).rows[0].line)

        // The boards: SR3B waits, SR3C is the same frame landed.
        val waitingBoard = SettingsFixtures.buildState(SettingsScreenState.SR3B, strings).balanceDetail
        val landedBoard = SettingsFixtures.buildState(SettingsScreenState.SR3C, strings).balanceDetail
        assertTrue(waitingBoard.summaryWithheld && waitingBoard.done.all { it.amountWithheld })
        assertEquals(waitingBoard.done.map { it.name } to waitingBoard.pending, landedBoard.done.map { it.name } to landedBoard.pending)
        assertTrue(landedBoard.summary, landedBoard.summary.contains("CN¥5,643.01"))
    }

    // -- activity_row -------------------------------------------------------------

    /** A row draws its token amount and its coin — this shell puts no fiat on a row, waiting or not. */
    @Test
    fun activity_row() {
        val feed = WalletFixtures.liveHiddenFeed(hidden = false)
        val base = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)
        for (currency in listOf(waiting, committed)) {
            val home = WalletLive.home(base, view, feed, currency, strings, chains)
            val rows = home.activityGroups.flatMap { it.rows } + WalletLive.activity(feed, strings, chainNames = chains).flatMap { it.rows }
            assertTrue(rows.isNotEmpty())
            assertNoFiat("activity_row", rows.flatMap { listOf(it.amount, it.received, it.subtitle) })
            // The token amounts are there either way.
            assertTrue(rows.any { it.amount == "+289.5" && it.unit == "USDT" })
        }
    }

    // -- activity_detail (a transfer's and a dApp's) ------------------------------

    @Test
    fun activity_detail() {
        val feed = WalletFixtures.liveHiddenFeed(hidden = false)
        val fallback = (FlowFixtures.build(FlowState.A2, strings).sheet as FlowSheet.TxDetail).model
        fun detail(id: String, currency: CurrencyView) =
            FlowLive.txDetail(fallback, feed, id, strings, chains, money = WalletLive.Money.of(currency))!!

        for (id in listOf("received", "sent", WalletFixtures.SPLIT_ROW, "swap")) {
            val wait = detail(id, waiting)
            assertEquals("$id: the worth is empty, its line kept", "" to true, wait.fiat to wait.fiatWithheld)
            assertTrue("$id: the token amount is drawn as always", wait.amount.isNotBlank())
            assertNoFiat("activity_detail ($id)", listOf(wait.fiat, wait.received) + wait.facts.flatMap { listOf(it.value) + it.lines })
            val landed = detail(id, committed)
            assertTrue("$id: ${landed.fiat}", landed.fiat.startsWith("≈ CN¥") && !landed.fiatWithheld)
            assertEquals("$id: nothing else changed", wait.copy(fiat = "", fiatWithheld = false), landed.copy(fiat = "", fiatWithheld = false))
        }
        // A record the core knows no price for has no worth line at all: nothing to withhold.
        val signature = detail("signature", waiting)
        assertEquals("" to false, signature.fiat to signature.fiatWithheld)
        // Hidden is the mask, at once — never "withheld".
        val hidden = FlowLive.txDetail(fallback, WalletFixtures.liveHiddenFeed(), "received", strings, chains, money = WalletLive.Money.of(waiting))!!
        assertEquals(WalletLive.MASK to false, hidden.fiat to hidden.fiatWithheld)
    }

    // -- send_coin_list -----------------------------------------------------------

    private fun sendContext(currency: CurrencyView) = SendLive.Context(strings, chains, emptyMap(), WalletLive.Money.of(currency), "Main", me)

    private val sendTokens = view.tokens.map { token ->
        SendToken(
            network = "chain-${token.chain_id}", chain_id = token.chain_id, symbol = token.symbol, balance = token.balance,
            decimals = token.decimals, token_address = token.token_address, price_usd = token.price_usd,
        )
    }

    @Test
    fun send_coin_list() {
        val fallback = (FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick).model
        val wait = SendLive.pick(fallback, SendView(tokens = sendTokens), sendContext(waiting))
        assertTrue(wait.rows.isNotEmpty() && wait.rows.all { it.fiat == AssetFiatModel.Loading })
        assertEquals(listOf("418.25 xDAI", "376.54 USDC"), wait.rows.map { it.balance })
        val landed = SendLive.pick(fallback, SendView(tokens = sendTokens), sendContext(committed))
        assertTrue(landed.rows.all { (it.fiat as AssetFiatModel.Value).text.startsWith("CN¥") })
    }

    // -- send_form (the "≈" line, the fee's worth, a split's total, the confirm) ---

    private fun form(state: FlowState) = (FlowFixtures.build(state, strings).base as FlowBase.SendForm).model
    private fun confirm(state: FlowState) = (FlowFixtures.build(state, strings).base as FlowBase.SendConfirm).model

    private fun drawn(form: SendFormModel) = listOf(form.amount?.fiat, form.fee.value, form.summary?.value, form.warning, form.speed?.value) +
        form.speed?.options.orEmpty().map { it.value }

    private fun drawn(confirm: SendConfirmModel) = listOf(confirm.amount, confirm.subline) +
        confirm.facts.flatMap { listOf(it.value, it.note) + it.lines } + confirm.breakdown.map { it.value }

    @Test
    fun send_form() {
        // The form: 120 USDT typed, a fee of 0.000123 ETH priced at $2,560.
        val wait = form(FlowState.SD2N)
        val amount = wait.amount!!
        assertEquals("the \"≈\" line: empty, its room kept", "" to true, amount.fiat to amount.fiatWithheld)
        assertEquals("the figure typed is a token amount: drawn", "120", amount.value)
        assertFalse("⇄ would type in the placeholder's dollars: not while waiting", amount.denomEnabled)
        assertTrue("…and it keeps its place", amount.denomShown)
        assertEquals("the fee in its coin, no worth beside it", "0.000123 ETH", wait.fee.value)
        assertNoFiat("send_form", drawn(wait), "852", "2.24")

        val landed = form(FlowState.SD2O)
        assertEquals("≈ CN¥852.00" to false, landed.amount!!.fiat to landed.amount!!.fiatWithheld)
        assertTrue(landed.amount!!.denomEnabled)
        assertEquals("0.000123 ETH · ≈CN¥2.24", landed.fee.value)
        // Nothing else on the form differs between the two frames.
        assertEquals(
            wait.copy(amount = null, fee = wait.fee.copy(value = "")),
            landed.copy(amount = null, fee = landed.fee.copy(value = "")),
        )

        // The confirm: the worth under the figure, and the fee's.
        val waitConfirm = confirm(FlowState.SD3J)
        assertEquals("" to true, waitConfirm.subline to waitConfirm.sublineWithheld)
        assertNoFiat("send_form (confirm)", drawn(waitConfirm), "852", "2.24")
        val landedConfirm = confirm(FlowState.SD3K)
        assertEquals("≈ CN¥852.00" to false, landedConfirm.subline to landedConfirm.sublineWithheld)
        assertTrue(landedConfirm.facts.joinToString { it.value }, landedConfirm.facts.any { it.value.endsWith("0.000123 ETH · ≈CN¥2.24") })
        assertTrue(waitConfirm.facts.joinToString { it.value }, waitConfirm.facts.any { it.value.endsWith("0.000123 ETH") })
        assertEquals(waitConfirm.facts.map { it.label }, landedConfirm.facts.map { it.label })

        // A split's total rides one line with its worth: the token total alone while waiting.
        val split = SendView(
            selected_token = sendTokens[1], split_mode = true, confirm_amount = "300",
            recipients = listOf(
                app.getvela.wallet.feature.send.core.SendRecipientDraft(id = "a", address = "0x" + "11".repeat(20), amount = "100"),
                app.getvela.wallet.feature.send.core.SendRecipientDraft(id = "b", address = "0x" + "22".repeat(20), amount = "200"),
            ),
        )
        val waitTotal = SendLive.splitSummary(split, "USDC", sendContext(waiting)).value
        assertEquals("300 USDC", waitTotal)
        assertEquals("300 USDC · ≈ CN¥2,130.00", SendLive.splitSummary(split, "USDC", sendContext(committed)).value)
    }

    // -- signing_sheet (the fee's worth) ------------------------------------------

    @Test
    fun signing_sheet() {
        val wait = SigningFixtures.build(SigningScreenState.CS59, strings).fee as FeeModel.OnChain
        assertEquals("the fee in its coin, no worth beside it", "~0.000123 ETH", wait.value)
        assertNoFiat("signing_sheet", listOf(wait.value, wait.warning, wait.speed?.value) + wait.options.map { it.toString() }, "2.24", "0.31")
        val landed = SigningFixtures.build(SigningScreenState.CS60, strings).fee as FeeModel.OnChain
        assertEquals("~0.000123 ETH · ≈CN¥2.24", landed.value)
        // The one formatter every fee line shares: the send's, the sheet's, the hand-off card's.
        val parts = SendLive.FeeParts(coin = "0.000123 ETH", mark = WalletLive.mark(1, "ETH", null), units = 0.000123, contract = null)
        assertEquals("0.000123 ETH", SendLive.feeLine(parts, 2560.0, WalletLive.Money.of(waiting)))
        assertEquals("0.000123 ETH · ≈CN¥2.24", SendLive.feeLine(parts, 2560.0, WalletLive.Money.of(committed)))
    }

    // -- settings_total -----------------------------------------------------------

    /** Settings' accounts sheet is the home's switcher, built by the one builder. */
    @Test
    fun settings_total() {
        val base = SettingsFixtures.buildState(SettingsScreenState.ST2, strings)
        val wait = SettingsLive.withAccounts(base, listOf("Main" to me), 0, switcher, waiting, strings).accountsSheet
        assertNoFiat("settings_total", listOf(wait.summary) + wait.rows.map { it.amount }, "794", "5,643")
        assertEquals("", wait.rows.single().amount)
        val landed = SettingsLive.withAccounts(base, listOf("Main" to me), 0, switcher, committed, strings).accountsSheet
        assertEquals("CN¥5,643.01", landed.rows.single().amount)
    }

    // -- beyond the twelve: the dApp panel's network rows -------------------------

    @Test
    fun `the network picker says nothing rather than a placeholder figure`() {
        val nets = listOf(1L, 100L).map { NetNetworkRow(id = it.toString(), chain_id = it, display_name = chains[it.toInt()].orEmpty(), native_symbol = "ETH") }
        val wait = ExploreLive.networkOptions(nets, chains, siteChain = 100, balances = view, fiat = WalletLive.Money.of(waiting)::fiat)
        assertTrue(wait.all { it.amount == null })
        val landed = ExploreLive.networkOptions(nets, chains, siteChain = 100, balances = view, fiat = WalletLive.Money.of(committed)::fiat)
        assertEquals(listOf("CN¥2,673.43", "CN¥2,969.57"), landed.map { it.amount })
    }
}
