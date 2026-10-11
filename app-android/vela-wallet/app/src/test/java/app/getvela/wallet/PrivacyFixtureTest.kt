package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.ContactsScreenState
import app.getvela.wallet.feature.contacts.core.Contact
import app.getvela.wallet.feature.contacts.core.ContactsView
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.TxDetailModel
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.ActivityRowModel
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.AssetRowModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Balance privacy, replayed from the core's own fixture
 * (`rust/crates/vela-core/tests/fixtures/privacy-hidden.json`, written by
 * `tests/app_privacy.rs`): a real balance and feed, shown and hidden, driven
 * through `balance_dashboard` and `activity_feed`. Every shell replays it
 * through every surface builder it has, so "hidden" means the same thing on
 * all four — the core's rule (`app::privacy`), never a shell's own.
 *
 * 1. Shown: every forbidden digit run appears on some surface — the fixture
 *    really reaches the builders, so (2) is not passing on an empty screen.
 * 2. Hidden: no masked surface's output holds one, and every masked figure
 *    reads the mask (the hero the wider one).
 * 3. Hidden: Send and the signing sheet keep their figures on purpose — you
 *    cannot choose, or consent to, an amount you cannot see. (Receive's
 *    "arrived" list is not drawn on Android.)
 * 4. A row's own figure masks exactly when the core says it is money
 *    (`figure_maskable`).
 * 5. The fixture's SPLIT (one operation, two recipients): its row draws the
 *    total and its detail lists each person's share. Shown, all three
 *    figures are on screen; hidden, none is — on the row, in the detail, in
 *    History or on the home — and each masked figure keeps its unit.
 */
class PrivacyFixtureTest {
    private val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
    private val fixture = JSONObject(File(root, "rust/crates/vela-core/tests/fixtures/privacy-hidden.json").readText())

    private val strings: VelaStrings by lazy {
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val forbidden: List<String> = fixture.getJSONArray("forbidden").let { a -> (0 until a.length()).map(a::getString) }
    private val mask = fixture.getString("mask")
    private val balanceMask = fixture.getString("balance_mask")

    private fun balance(half: String): BalanceView =
        Wire.json.decodeFromString(BalanceView.serializer(), fixture.getJSONObject(half).getJSONObject("balance").toString())

    private fun feed(half: String): FeedView =
        Wire.json.decodeFromString(FeedView.serializer(), fixture.getJSONObject(half).getJSONObject("feed").toString())

    private val usd = CurrencyView(code = "USD", committed = true)
    private val chains = mapOf(1 to "Ethereum", 100 to "Gnosis", 56 to "BNB Chain")
    private val other = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

    /** The split's feed row id (its operation hash), its total and its two shares, as the fixture carries them. */
    private val splitId = "0x" + "ef".repeat(32)
    private val splitTotal = "683"
    private val splitShares = listOf("214", "469")
    private val bea = "0xdddddddddddddddddddddddddddddddddddddddd"

    // -- the surfaces, each as the figures it draws ------------------------------

    private fun rowFigures(row: ActivityRowModel): List<String> = listOfNotNull(row.amount, row.received)

    private fun assetFigures(row: AssetRowModel): List<String> = listOf(
        row.balance,
        when (val fiat = row.fiat) {
            is AssetFiatModel.Value -> fiat.text
            is AssetFiatModel.NoPrice -> fiat.text
            AssetFiatModel.Masked -> mask
            // No figure at all: neither a digit nor the mask.
            AssetFiatModel.Loading, AssetFiatModel.None -> ""
        },
    )

    private fun detailFigures(detail: TxDetailModel): List<String> =
        listOfNotNull(detail.amount, detail.fiat, detail.received) + detail.facts.flatMap { listOf(it.value) + it.lines }

    private fun itemIds(feed: FeedView): List<String> = feed.rows.filterIsInstance<FeedRow.Item>().map { it.item.id }

    /** Every masked surface's figures, by the surface's name in the fixture's `masked_surfaces`. */
    private fun maskedSurfaces(view: BalanceView, feed: FeedView): Map<String, List<String>> {
        val home = WalletLive.home(WalletFixtures.buildMobileState(WalletScreenState.H1, strings), view, feed, usd, strings, chains)
        val assets = FlowLive.assets((FlowFixtures.build(FlowState.T1, strings).base as FlowBase.Assets).model, view, chains, usd)
        val tokenFallback = (FlowFixtures.build(FlowState.T2, strings).sheet as FlowSheet.TokenDetail).model
        val tokenDetails = view.tokens.map { token ->
            FlowLive.tokenDetail(tokenFallback, view, feed, WalletLive.holdingId(token.chain_id, token.token_address), chains, usd, strings)!!
        }
        val history = FlowLive.history((FlowFixtures.build(FlowState.A1, strings).base as FlowBase.History).model, feed, strings, chainNames = chains)
        val txFallback = (FlowFixtures.build(FlowState.A2, strings).sheet as FlowSheet.TxDetail).model
        val details = itemIds(feed).associateWith { id ->
            FlowLive.txDetail(txFallback, feed, id, strings, chains, money = WalletLive.Money.of(usd))!!
        }
        val dappIds = feed.rows.filterIsInstance<FeedRow.Item>().filter { it.item.dapp != null }.map { it.item.id }.toSet()
        val contact = ContactsLive.detail(
            ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!,
            Contact(address = bea, name = "Bea"),
            ContactsView(loaded = true, contacts = listOf(Contact(address = bea, name = "Bea"))),
            feed = feed,
            strings = strings,
            chainNames = chains,
        )
        val switcher = WalletLive.accountSwitcher(
            listOf("Main" to view.address.orEmpty(), "Other" to other),
            0,
            view.switcher,
            usd,
            strings,
        )
        val settings = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        val detail = SettingsLive.balanceDetail(settings.balanceDetail, view, usd, chains, strings)
        val unreachable = SettingsLive.unreachable(view, usd, chains, strings)
        val nets = listOf(1L, 100L, 56L).map { NetNetworkRow(id = it.toString(), chain_id = it, display_name = chains[it.toInt()].orEmpty(), native_symbol = "ETH") }
        val picker = ExploreLive.networkOptions(nets, chains, siteChain = 100, balances = view, fiat = { WalletLive.Money.of(usd).fiat(it) })
        return mapOf(
            "home_total" to listOfNotNull(home.balance.integer, home.balance.decimals, home.balance.status?.text),
            "holdings" to home.assetRows.flatMap(::assetFigures),
            "assets" to assets.rows.flatMap(::assetFigures),
            "token_detail" to tokenDetails.flatMap { listOf(it.balance, it.fiat) + it.rows.flatMap(::rowFigures) },
            "home_activity" to home.activityGroups.flatMap { it.rows }.flatMap(::rowFigures),
            "history" to history.groups.flatMap { it.rows }.flatMap(::rowFigures),
            "transfer_detail" to details.filterKeys { it !in dappIds }.values.flatMap(::detailFigures),
            "dapp_detail" to details.filterKeys { it in dappIds }.values.flatMap(::detailFigures),
            "contact_activity" to contact.activity.rows.flatMap(::rowFigures),
            "account_switcher" to listOf(switcher.summary) + switcher.rows.map { it.amount },
            "balance_detail" to listOf(detail.summary) + (detail.pending + detail.done + detail.unpriced).flatMap { listOfNotNull(it.amount, it.status) } +
                unreachable.rows.map { it.line },
            "network_picker" to picker.mapNotNull { it.amount },
            // The core withholds the toast itself; Android draws whatever it sends.
            "receipt_toast" to listOfNotNull(feed.toast?.value),
        )
    }

    /** What Send and the signing sheet draw from the same money — visible on purpose. */
    private fun visibleSurfaces(view: BalanceView, feed: FeedView): Map<String, List<String>> {
        val sendView = SendView(
            tokens = view.tokens.map { token ->
                SendToken(
                    network = "chain-${token.chain_id}", chain_id = token.chain_id, symbol = token.symbol, balance = token.balance,
                    decimals = token.decimals, token_address = token.token_address, price_usd = token.price_usd,
                )
            },
        )
        val ctx = SendLive.Context(strings, chains, emptyMap(), WalletLive.Money.of(usd), "Main", view.address.orEmpty())
        val pick = SendLive.pick((FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick).model, sendView, ctx)
        // The swap's balance changes, as the sheet's own simulation would show them.
        val swap = feed.rows.filterIsInstance<FeedRow.Item>().first { it.item.id == "swap" }.item.dapp!!
        val judged = swap.changes.map { change ->
            val units = java.math.BigDecimal(change.value!!).movePointRight(change.decimals ?: 0).toBigInteger()
            val delta = (if (change.direction == app.getvela.wallet.feature.wallet.core.FeedDirection.In) units else units.negate()).toString()
            TrustSimJudgment.Erc20Trusted(token = "0x" + change.symbol.lowercase().padEnd(40, '0').take(40), delta = delta, symbol = change.symbol, decimals = change.decimals ?: 0)
        }
        val signing = SigningLive.simBlocks(
            SigningController.SimOutcome.Ready(judged),
            SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Main", view.address.orEmpty()),
        ).filterIsInstance<SigningBlock.Balances>().flatMap { block -> block.rows.map { it.delta } }
        return mapOf(
            "send" to pick.rows.flatMap(::assetFigures),
            "signing_sheet" to signing,
        )
    }

    private fun holdsForbidden(text: String): String? = forbidden.firstOrNull { text.contains(it) }

    // -- (1) shown: the fixture reaches every builder -----------------------------

    @Test
    fun `shown, every forbidden figure appears on some surface`() {
        val surfaces = maskedSurfaces(balance("shown"), feed("shown"))
        val all = surfaces.values.flatten()
        for (run in forbidden) {
            assertTrue("shown: \"$run\" is on no surface — the fixture did not reach a builder:\n$surfaces", all.any { it.contains(run) })
        }
        // Every surface the fixture names is one this shell built.
        val named = fixture.getJSONArray("masked_surfaces").let { a -> (0 until a.length()).map(a::getString) }.toSet()
        assertEquals(named, surfaces.keys)
    }

    // -- (2) hidden: no masked surface holds a figure -----------------------------

    @Test
    fun `hidden, no masked surface holds a figure, and every masked figure reads the mask`() {
        val view = balance("hidden")
        val feed = feed("hidden")
        assertTrue("the fixture's hidden half is hidden", view.hidden && feed.hidden && view.switcher.hidden)
        for ((surface, figures) in maskedSurfaces(view, feed)) {
            for (figure in figures) {
                assertNull("$surface leaks \"$figure\" while hidden", holdsForbidden(figure))
            }
        }
        val home = WalletLive.home(WalletFixtures.buildMobileState(WalletScreenState.H1, strings), view, feed, usd, strings, chains)
        assertEquals(BalanceStateKind.Hidden, home.balance.state)
        assertEquals("the hero's mask is the wider one", balanceMask, home.balance.integer)
        assertTrue("holdings: amount AND worth", home.assetRows.all { it.balance == mask && it.fiat == AssetFiatModel.Masked && it.masked })
        val switcher = WalletLive.accountSwitcher(listOf("Main" to view.address.orEmpty(), "Other" to other), 0, view.switcher, usd, strings)
        assertTrue("the switcher's rows", switcher.rows.all { it.amount == mask })
        assertTrue("the switcher's total: ${switcher.summary}", switcher.summary.contains(mask))
        val tokenFallback = (FlowFixtures.build(FlowState.T2, strings).sheet as FlowSheet.TokenDetail).model
        view.tokens.forEach { token ->
            val detail = FlowLive.tokenDetail(tokenFallback, view, feed, WalletLive.holdingId(token.chain_id, token.token_address), chains, usd, strings)!!
            assertEquals(mask, detail.balance)
            assertEquals(mask, detail.fiat)
        }
        val txFallback = (FlowFixtures.build(FlowState.A2, strings).sheet as FlowSheet.TxDetail).model
        // Item 12: a masked figure keeps its unit — how much is hidden, what
        // kind of money is not ("•••• xDAI", as iOS reads). A transfer's
        // detail used to drop it.
        val units = feed.rows.filterIsInstance<FeedRow.Item>().associate { it.item.id to it.item.symbol }
        for (id in listOf("received", "sent", "swap", splitId)) {
            val detail = FlowLive.txDetail(txFallback, feed, id, strings, chains, money = WalletLive.Money.of(usd))!!
            assertTrue("$id has a unit to keep", units.getValue(id).isNotBlank())
            assertEquals("$id: the amount", "$mask ${units.getValue(id)}", detail.amount)
            assertEquals("$id: the worth", mask, detail.fiat)
        }
        // A capped allowance is money: its figure masks, its coin stays.
        assertEquals("$mask USDC", FlowLive.txDetail(txFallback, feed, "permit", strings, chains)!!.amount)
        assertEquals("what came back masks", "$mask xDAI", FlowLive.txDetail(txFallback, feed, "swap", strings, chains)!!.received)
        assertNull("the core withholds the toast", feed.toast)
        assertNotNull("…which it sends while shown", feed("shown").toast)
    }

    // -- (5) the split: its row and its detail, shown and hidden ------------------

    private fun splitRow(feed: FeedView, rows: List<FeedRow> = feed.rows): ActivityRowModel =
        WalletLive.activity(feed.copy(rows = rows), strings, chainNames = chains).flatMap { it.rows }.single { it.id == splitId }

    private fun splitDetail(feed: FeedView): TxDetailModel {
        val fallback = (FlowFixtures.build(FlowState.A2, strings).sheet as FlowSheet.TxDetail).model
        return FlowLive.txDetail(fallback, feed, splitId, strings, chains, money = WalletLive.Money.of(usd))!!
    }

    /** The row of a detail that lists the split's people: its value says how many, its lines who got what. */
    private fun shares(detail: TxDetailModel) = detail.facts.single { it.lines.size == splitShares.size }

    @Test
    fun `shown, the split's row draws its total and its detail lists each share`() {
        val feed = feed("shown")
        val item = feed.rows.filterIsInstance<FeedRow.Item>().single { it.item.id == splitId }.item
        assertEquals("the fixture's row is a split of two", 2, item.batch?.transfers?.size)

        val row = splitRow(feed)
        assertTrue("the row's total: ${row.amount}", row.amount.contains(splitTotal))
        assertEquals("USDC", row.unit)
        assertTrue("the home draws it too: it is in the core's cut", splitRow(feed, feed.home_rows).amount.contains(splitTotal))

        val detail = splitDetail(feed)
        assertEquals("\u2212683.75 USDC", detail.amount)
        val people = shares(detail)
        assertEquals("2 recipients", people.value)
        assertEquals(listOf("Bea · 214.5 USDC", "0xfafa…fafa · 469.25 USDC"), people.lines)
        // …so each of the three runs is on a surface this shell draws.
        val drawn = (rowFigures(row) + detailFigures(detail)).joinToString(" ")
        for (run in splitShares + splitTotal) assertTrue("shown: \"$run\" in $drawn", drawn.contains(run))
    }

    @Test
    fun `hidden, the split leaks neither its total nor a share, and each mask keeps its unit`() {
        val feed = feed("hidden")
        // The row, in History and on the home: the mask, its coin drawn beside it as always.
        for ((where, rows) in listOf("history" to feed.rows, "home" to feed.home_rows)) {
            val row = splitRow(feed, rows)
            assertTrue("$where: masked", row.masked)
            assertEquals("$where: the figure", mask, row.amount)
            assertEquals("$where: the unit stays", "USDC", row.unit)
        }
        // The detail: the total and EVERY share are the mask with the coin —
        // who was paid stays, how much each got does not.
        val detail = splitDetail(feed)
        assertEquals("$mask USDC", detail.amount)
        assertEquals("the worth", mask, detail.fiat)
        val people = shares(detail)
        assertEquals("2 recipients", people.value)
        assertEquals(listOf("Bea · $mask USDC", "0xfafa…fafa · $mask USDC"), people.lines)
        assertEquals("the unit rule is the core's", uniffi.vela_core_uniffi.maskedAmount("USDC"), detail.amount)
        for (figure in rowFigures(splitRow(feed)) + detailFigures(detail)) {
            assertNull("the split leaks \"$figure\" while hidden", holdsForbidden(figure))
        }
        // The contact's page (Bea was paid by it) draws no share either.
        val contact = ContactsLive.detail(
            ContactsFixtures.buildMobileState(ContactsScreenState.C2, strings).detail!!,
            Contact(address = bea, name = "Bea"),
            ContactsView(loaded = true, contacts = listOf(Contact(address = bea, name = "Bea"))),
            feed = feed,
            strings = strings,
            chainNames = chains,
        )
        for (figure in contact.activity.rows.flatMap(::rowFigures)) assertNull("the contact page leaks \"$figure\"", holdsForbidden(figure))
    }

    // -- (3) hidden: Send and the signing sheet keep their figures ----------------

    @Test
    fun `hidden, Send and the signing sheet still show their figures`() {
        val visible = visibleSurfaces(balance("hidden"), feed("hidden"))
        val send = visible.getValue("send").joinToString(" ")
        assertTrue("Send's picker: $send", send.contains("418") && send.contains("376"))
        val signing = visible.getValue("signing_sheet").joinToString(" ")
        assertTrue("the signing sheet's balance changes: $signing", signing.contains("237") && signing.contains("128"))
        val named = fixture.getJSONArray("visible_surfaces").let { a -> (0 until a.length()).map(a::getString) }.toSet()
        assertTrue("every visible surface Android draws is checked", visible.keys.all { it in named })
    }

    // -- (4) a row masks iff the core says its figure is money --------------------

    @Test
    fun `a row's figure masks exactly when the core says it is money`() {
        val maskable = fixture.getJSONObject("figure_maskable")
        val ids = maskable.keys().asSequence().toSet()
        val hidden = feed("hidden")
        val rows = WalletLive.activity(hidden, strings, chainNames = chains).flatMap { it.rows }.associateBy { it.id }
        assertEquals(ids, rows.keys)
        for (id in ids) {
            val row = rows.getValue(id)
            assertEquals("$id: masked", maskable.getBoolean(id), row.masked)
            if (maskable.getBoolean(id)) {
                assertEquals("$id reads the mask", mask, row.amount)
            } else {
                assertTrue("$id keeps what it draws — never dots that claim a figure: ${row.amount}", row.amount != mask)
            }
        }
        assertEquals("an unlimited allowance stays: a risk to see", strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.UNLIMITED), rows.getValue("permit-unlimited").amount)
        // The core's own decision, not the shell's, reaches the row.
        val shown = WalletLive.activity(feed("shown"), strings, chainNames = chains).flatMap { it.rows }
        assertTrue("shown, nothing masks", shown.none { it.masked })
    }
}
