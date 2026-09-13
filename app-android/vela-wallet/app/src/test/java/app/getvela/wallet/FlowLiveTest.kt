package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.flows.HistoryMode
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey

/**
 * The receive screens.
 *
 * **This is the one screen in the app where a stale value is unrecoverable.**
 * Every other fixture that leaks shows somebody the wrong information; a
 * fixture address on a receive QR sends their money to a stranger, and no part
 * of the system can undo it. Every case here is about that.
 */
class FlowLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private val mine = "0x7687AbCdEf0123456789abcdefABCDEF0123D141"

    private fun qrFixture() =
        (FlowFixtures.build(FlowState.R2, strings).sheet as FlowSheet.ReceiveQr).model

    private fun listFixture() =
        (FlowFixtures.build(FlowState.R1, strings).base as FlowBase.Receive).model

    private fun row(chainId: Long, name: String, symbol: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
    )

    @Test
    fun `the QR card shows this person's address, not the drawn one`() {
        val drawn = qrFixture()
        // The fixture's address is somebody else's, and it is a real-looking
        // one — which is exactly why this cannot be left to a glance.
        assertTrue(drawn.account.lines.first.startsWith("0x"))

        val live = FlowLive.receiveQr(drawn, mine, "Me", PaymentRequestView())

        assertEquals(mine, live.account.lines.first + live.account.lines.second)
        assertEquals("Me", live.account.name)
    }

    /**
     * The identicon is drawn FROM the address.
     *
     * A stale seed beside a live address is a mismatch nobody can interpret —
     * and the face is precisely what people use to check at a glance that they
     * are looking at their own wallet.
     */
    @Test
    fun `the face is drawn from the address on screen`() {
        val live = FlowLive.receiveQr(qrFixture(), mine, "Me", PaymentRequestView())

        assertEquals(mine, live.account.identiconSeed)
    }

    /**
     * **No session, no address.**
     *
     * Not "keep the fixture until we have one". A blank card is a screen that
     * cannot be acted on; a fixture card is a screen that can.
     */
    @Test
    fun `an empty session shows no address at all`() {
        val live = FlowLive.receiveQr(qrFixture(), "", "", PaymentRequestView())

        assertEquals("" to "", live.account.lines)
        assertEquals(WalletFixtures.ADDRESS_FULL != "", true) // the fixture HAS one
        assertFalse(
            "a blank session must not fall back to the drawn address",
            live.account.lines.first.startsWith("0x"),
        )
    }

    /** The two lines are the whole address, split — never a shortened one. */
    @Test
    fun `the card carries the full address across its two lines`() {
        val live = FlowLive.receiveQr(qrFixture(), mine, "Me", PaymentRequestView())

        val rejoined = live.account.lines.first + live.account.lines.second
        assertEquals(42, rejoined.length)
        assertFalse("a copy button must not put an ellipsis on the clipboard", rejoined.contains("…"))
    }

    // -- the warning gate -----------------------------------------------------

    @Test
    fun `the QR stays covered until the gate has both loaded and been accepted`() {
        // While loading, a first visit would otherwise flash the code before
        // the warning about which networks this address is safe on.
        assertFalse(FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = true)))
        assertFalse(
            FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = false, acknowledged = false)),
        )
        assertTrue(
            FlowLive.receiveGateOpen(PaymentRequestView(gate_loading = false, acknowledged = true)),
        )
    }

    // -- the network list -----------------------------------------------------

    @Test
    fun `the network list is this device's, with this address on every row`() {
        val view = NetView(
            loaded = true,
            networks = listOf(row(1, "Ethereum", "ETH"), row(137, "Polygon", "POL")),
        )

        val live = FlowLive.receiveNetworks(listFixture(), view, mine, WalletLive::badge)

        assertEquals(listOf("Ethereum", "Polygon"), live.rows.map { it.name })
        assertEquals(listOf("ETH", "POL"), live.rows.map { it.code })
        // One address across every network — the thing the subtitle claims.
        assertEquals(1, live.rows.map { it.addressDisplay }.distinct().size)
        assertTrue(live.rows[0].addressDisplay.startsWith("0x7687"))
        assertTrue(live.rows[0].addressDisplay.endsWith("D141"))
    }

    @Test
    fun `the subtitle counts the networks this device has`() {
        val view = NetView(loaded = true, networks = listOf(row(1, "Ethereum", "ETH")))

        val live = FlowLive.receiveNetworks(listFixture(), view, mine, WalletLive::badge)

        // The fixture says "across all 8 networks" beside a live list; a count
        // that disagrees with the rows under it is the 040 bug in a new place.
        assertTrue("the count follows the list", live.subtitle.contains("1"))
        assertFalse(live.subtitle.contains("8"))
    }

    @Test
    fun `an unloaded network list leaves the screen alone`() {
        val drawn = listFixture()

        val live = FlowLive.receiveNetworks(drawn, NetView(), mine, WalletLive::badge)

        // Before storage answers there is nothing truer to show; blanking for a
        // frame is a flicker a person reads as a bug.
        assertEquals(drawn.rows, live.rows)
    }

    // -- the read screens behind "全部" (spec 041 phase 5b) -------------------

    private fun feedItem(
        id: String,
        received: Boolean,
        value: String,
        symbol: String = "POL",
        chainId: Int = 137,
        usd: Double = 0.0,
    ) = FeedItem(
        id = id,
        direction = if (received) FeedDirection.In else FeedDirection.Out,
        counterparty = "0x9F3c000000000000000000000000000000021aE0",
        value = value,
        symbol = symbol,
        decimals = 18,
        usd_value = usd,
        chain_id = chainId,
        timestamp = System.currentTimeMillis() / 1000.0,
        day_start_ms = midnightToday(),
    )

    private fun midnightToday(): Double = java.util.Calendar.getInstance().apply {
        set(java.util.Calendar.HOUR_OF_DAY, 0)
        set(java.util.Calendar.MINUTE, 0)
        set(java.util.Calendar.SECOND, 0)
        set(java.util.Calendar.MILLISECOND, 0)
    }.timeInMillis.toDouble()

    private fun feedOf(vararg items: FeedItem) = FeedView(
        rows = listOf(
            FeedRow.Header("day", midnightToday(), System.currentTimeMillis() / 1000.0),
        ) + items.map { FeedRow.Item(it) },
    )

    private fun historyFixture() =
        (FlowFixtures.build(FlowState.A1, strings).base as FlowBase.History).model

    private fun txFixture() =
        (FlowFixtures.build(FlowState.A2, strings).sheet as FlowSheet.TxDetail).model

    private fun assetsFixture() =
        (FlowFixtures.build(FlowState.T1, strings).base as FlowBase.Assets).model

    private fun tokenFixture() =
        (FlowFixtures.build(FlowState.T2, strings).sheet as FlowSheet.TokenDetail).model

    private fun token(symbol: String, balance: String, chainId: Int, price: Double? = null) =
        BalanceToken(
            chain_id = chainId,
            symbol = symbol,
            name = symbol,
            balance = balance,
            decimals = 18,
            price_usd = price,
        )

    private val chainNames = mapOf(137 to "Polygon", 42161 to "Arbitrum")

    /** "全部" opened a fixture: a person tapped past their own payments. */
    @Test
    fun `the history screen lists this device's own transactions`() {
        val drawn = historyFixture()
        assertTrue("the fixture has history of its own", drawn.groups.isNotEmpty())

        val live = FlowLive.history(drawn, feedOf(feedItem("a", false, "2")), strings)

        assertEquals(1, live.groups.sumOf { it.rows.size })
        assertEquals("−2", live.groups.single().rows.single().amount)
    }

    @Test
    fun `an empty history says so rather than borrowing one`() {
        val live = FlowLive.history(historyFixture(), FeedView(), strings)

        assertEquals(emptyList<Any>(), live.groups)
        assertEquals(HistoryMode.Empty, live.mode)
    }

    /**
     * **A detail with no target shows nothing.**
     *
     * Every row opened the same screen before the id existed, so tapping one
     * payment showed another. A screen about the wrong payment and one about
     * the right payment look equally authoritative; only one is wrong.
     */
    @Test
    fun `a transaction detail with no matching id renders nothing`() {
        val feed = feedOf(feedItem("a", false, "2"))

        assertNull(FlowLive.txDetail(txFixture(), feed, id = null, strings = strings))
        assertNull(FlowLive.txDetail(txFixture(), feed, id = "not-in-the-feed", strings = strings))
    }

    @Test
    fun `a transaction detail shows the transaction that was tapped`() {
        val feed = feedOf(
            feedItem("a", received = false, value = "2", usd = 0.5),
            feedItem("b", received = true, value = "120", symbol = "USDT", usd = 120.0),
        )

        val detail = FlowLive.txDetail(txFixture(), feed, id = "b", strings = strings)!!

        assertTrue(detail.amount.startsWith("+120"))
        assertTrue(detail.amount.contains("USDT"))
        assertEquals(true, detail.positive)
        assertEquals("≈ $120.00", detail.fiat)
    }

    /**
     * Spec 043 phase 4, device-found: the notification's deep link opened the
     * sheet with the fixture's "received USDT" title and facts around a live
     * amount. Every line is the tapped item's now.
     */
    @Test
    fun `a transaction detail carries its own title, status and facts`() {
        val feed = feedOf(feedItem("sent", received = false, value = "0.001", symbol = "XDAI", chainId = 100))

        val detail = FlowLive.txDetail(txFixture(), feed, id = "sent", strings = strings, chainNames = mapOf(100 to "Gnosis"))!!

        assertEquals(strings.t(I18nKeys.Flows.TX_LABEL_SENT, mapOf("symbol" to "XDAI")), detail.title)
        assertEquals(strings.t(I18nKeys.Flows.STATUS_PENDING), detail.status.text)
        assertEquals(
            listOf(I18nKeys.Flows.DETAIL_TO, I18nKeys.Flows.DETAIL_CHAIN, I18nKeys.Flows.DETAIL_DATE, I18nKeys.Flows.DETAIL_HASH).map { strings.t(it) },
            detail.facts.map { it.label },
        )
        assertEquals("Gnosis", detail.facts[1].value)
        assertTrue("the counterparty is the item's, shortened", detail.facts[0].value.startsWith("0x9F3c"))
        assertTrue("no chain hash yet: the row's own id", detail.facts[3].value.startsWith("sent"))
        assertTrue(detail.facts[2].value.startsWith(strings.t(I18nKeys.Flows.DAY_TODAY)))
    }

    /** An unpriced payment shows no fiat line, not a confident zero. */
    @Test
    fun `a transaction nothing could price shows no fiat figure`() {
        val feed = feedOf(feedItem("a", received = true, value = "5", usd = 0.0))

        val detail = FlowLive.txDetail(txFixture(), feed, id = "a", strings = strings)!!

        assertEquals("", detail.fiat)
    }

    // -- assets ---------------------------------------------------------------

    @Test
    fun `the assets screen lists this device's own holdings`() {
        val view = BalanceView(
            display_total_usd = 5.0,
            tokens = listOf(
                token("POL", "0.152784", 137, price = 0.097),
                token("ETH", "0.002", 42161, price = 2500.0),
            ),
        )

        val live = FlowLive.assets(assetsFixture(), view, chainNames, CurrencyView(code = "USD"))

        assertEquals(listOf("POL", "ETH"), live.rows.map { it.ticker })
        assertEquals(listOf("Polygon", "Arbitrum"), live.rows.map { it.chain })
        // A wallet that holds something is not an empty wallet.
        assertNull(live.empty)
    }

    @Test
    fun `an empty assets screen keeps its guided empty state`() {
        val live = FlowLive.assets(
            assetsFixture(),
            BalanceView(),
            chainNames,
            CurrencyView(code = "USD"),
        )

        assertEquals(emptyList<Any>(), live.rows)
    }

    @Test
    fun `a token detail with no matching id renders nothing`() {
        val view = BalanceView(tokens = listOf(token("POL", "1", 137, price = 1.0)))

        assertNull(detailFor(view, FeedView(), id = null))
        assertNull(detailFor(view, FeedView(), id = "999:native"))
    }

    private fun detailFor(view: BalanceView, feed: FeedView, id: String?) = FlowLive.tokenDetail(
        fallback = tokenFixture(),
        view = view,
        feed = feed,
        id = id,
        chainNames = chainNames,
        currency = CurrencyView(code = "USD"),
        strings = strings,
    )

    @Test
    fun `a token detail shows the holding that was tapped, and only its own history`() {
        val view = BalanceView(
            tokens = listOf(
                token("POL", "0.152784", 137, price = 0.097),
                token("ETH", "0.002", 42161, price = 2500.0),
            ),
        )
        val feed = feedOf(
            feedItem("a", received = false, value = "2", symbol = "POL", chainId = 137),
            feedItem("b", received = true, value = "1", symbol = "ETH", chainId = 42161),
        )

        val detail = detailFor(view, feed, id = "137:native")!!

        assertEquals("POL", detail.symbol)
        assertEquals("Polygon", detail.chain)
        assertTrue(detail.balance.startsWith("0.152784"))
        // Somebody else's ETH transfer must not appear under POL.
        assertEquals(1, detail.rows.size)
        assertEquals("POL", detail.rows.single().unit)
    }

    /** An unpriced holding shows "—", never a zero that calls it worthless. */
    @Test
    fun `an unpriced token detail shows no figure rather than zero`() {
        val view = BalanceView(tokens = listOf(token("MON", "1", 143)))

        val detail = detailFor(view, FeedView(), id = "143:native")!!

        assertEquals("—", detail.fiat)
    }

    /** Spec 049: the detail's amount takes the preset's mark; its fiat line is the display currency, never a bare `$`. */
    @Test
    fun `a transaction detail follows the number preset and the display currency`() {
        val saved = Formats.current
        Formats.current = Formats(NumberFormatKey.DotComma)
        try {
            val feed = feedOf(feedItem("sent", received = false, value = "0.001", symbol = "XDAI", chainId = 100, usd = 1.0))
            val money = WalletLive.Money.of(CurrencyView(code = "GBP", rate = 0.78, committed = true))
            val detail = FlowLive.txDetail(txFixture(), feed, id = "sent", strings = strings, chainNames = mapOf(100 to "Gnosis"), money = money)!!
            assertEquals("−0,001 XDAI", detail.amount)
            assertEquals("≈ £0,78", detail.fiat)
            val dollars = FlowLive.txDetail(txFixture(), feed, id = "sent", strings = strings, chainNames = mapOf(100 to "Gnosis"))!!
            assertEquals("≈ $1,00", dollars.fiat)
        } finally {
            Formats.current = saved
        }
    }
}
