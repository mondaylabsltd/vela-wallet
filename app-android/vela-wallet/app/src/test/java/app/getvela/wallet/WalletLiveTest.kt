package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.BalanceStatusKind
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The live wallet-home builders.
 *
 * Every case here is a way a money screen can lie, and each one was written
 * because the screen told that lie at least once. The most expensive was the
 * last: a real phone, two real holdings, and a confident "$0.00" above them.
 */
class WalletLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "public/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun base() = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)

    /** The chain names a device would have, from its network list. */
    private val chains = mapOf(137 to "Polygon", 42161 to "Arbitrum")

    private fun home(
        view: BalanceView,
        feed: FeedView = FeedView(),
        currency: CurrencyView = CurrencyView(code = "USD"),
    ) = WalletLive.home(base(), view, feed, currency, strings, chains)

    private fun token(
        symbol: String,
        balance: String,
        price: Double? = null,
        chainId: Int = 137,
        name: String = symbol,
    ) = BalanceToken(
        chain_id = chainId,
        symbol = symbol,
        // The TOKEN's name — "Ether", "USDT" — not the chain's. They were the
        // same string for as long as the shell was writing chain names into
        // this field, which is what hid the bug below.
        name = name,
        balance = balance,
        decimals = 18,
        price_usd = price,
    )

    /**
     * A row says which chain a holding is on.
     *
     * Reading that off `BalanceToken.name` looked correct for a whole phase,
     * because the shell was putting chain names in it. The moment real token
     * names arrived from the chain documents, every row read "USDT / USDT" and
     * "ETH / Ether" — the one thing the second line is for, gone.
     */
    @Test
    fun `an asset row names the chain, not the token again`() {
        val view = BalanceView(
            display_total_usd = 5.0,
            tokens = listOf(
                token("ETH", "0.002", price = 2500.0, chainId = 42161, name = "Ether"),
                token("USDT", "0.01", price = 1.0, chainId = 137, name = "USDT"),
            ),
        )

        val rows = home(view).assetRows

        assertEquals("Arbitrum", rows[0].chain)
        assertEquals("Polygon", rows[1].chain)
    }

    /** A chain this device has no row for still says something, never a blank. */
    @Test
    fun `an unknown chain falls back to the token's own name`() {
        val view = BalanceView(
            display_total_usd = 1.0,
            tokens = listOf(token("MON", "1", price = 1.0, chainId = 143, name = "Monad")),
        )

        assertEquals("Monad", home(view).assetRows.single().chain)
    }

    /**
     * The device bug, pinned.
     *
     * The core folds an unpriced holding in at zero, so a wallet whose every
     * coin is unpriced totals exactly 0.0 — indistinguishable, at the type
     * level, from a wallet that is genuinely empty. Rendering that as "$0.00"
     * told a person holding POL and ETH that they had nothing.
     */
    @Test
    fun `every holding unpriced shows no total at all, never a zero`() {
        val view = BalanceView(
            display_total_usd = 0.0,
            tokens = listOf(token("POL", "0.152784"), token("ETH", "0.002")),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Loading, model.balance.state)
        assertNull(model.balance.integer)
        assertNull(model.balance.decimals)
    }

    /**
     * And it says why.
     *
     * The H1 fixture carries no status line, so borrowing one with `?.copy`
     * produced a bare skeleton: an empty hero, two holdings under it, and no
     * explanation. The warning has to be built, not inherited.
     */
    @Test
    fun `the empty hero explains itself`() {
        val view = BalanceView(display_total_usd = 0.0, tokens = listOf(token("POL", "0.152784")))

        val status = home(view).balance.status

        assertNotNull("an empty hero with no reason given is the bug", status)
        assertEquals(BalanceStatusKind.Warning, status!!.kind)
        assertTrue(status.text.isNotBlank())
    }

    /** A zero that IS zero keeps its figure — the states must stay distinct. */
    @Test
    fun `a genuinely empty wallet shows zero`() {
        val model = home(BalanceView(display_total_usd = 0.0))

        assertEquals(BalanceStateKind.ZeroLive, model.balance.state)
        assertEquals("$0", model.balance.integer)
    }

    /** One priced holding is enough for a total; the rest show "—" in their rows. */
    @Test
    fun `a partly priced wallet totals what it could price`() {
        val view = BalanceView(
            display_total_usd = 4.5,
            tokens = listOf(token("POL", "10", price = 0.45), token("ETH", "0.002")),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Normal, model.balance.state)
        assertEquals("$4", model.balance.integer)
        assertEquals("50", model.balance.decimals)
        assertEquals(AssetFiatModel.Value("$4.50"), model.assetRows[0].fiat)
        assertEquals(AssetFiatModel.NoPrice("—"), model.assetRows[1].fiat)
    }

    /**
     * Nothing held is not the same as nothing read yet.
     *
     * Both leave the asset list empty; only one of them should say "no assets".
     */
    @Test
    fun `an empty list while loading is not an empty wallet`() {
        val loading = home(BalanceView(holdings_loading = true))
        val settled = home(BalanceView())

        assertEquals(SectionMode.Loading, loading.assetsSection.mode)
        assertEquals(SectionMode.Empty, settled.assetsSection.mode)
    }

    /** Amounts are truncated, never rounded: a rounded-up balance is money nobody has. */
    @Test
    fun `a long balance is cut, not rounded up`() {
        val view = BalanceView(
            display_total_usd = 1.0,
            tokens = listOf(token("POL", "0.1234569999", price = 1.0)),
        )

        assertEquals("0.123456 POL", home(view).assetRows[0].balance)
    }

    /** Hidden hides the figure and nothing else. */
    @Test
    fun `hidden keeps the holdings, drops the number`() {
        val view = BalanceView(
            display_total_usd = 12.0,
            hidden = true,
            tokens = listOf(token("POL", "10", price = 1.2)),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Hidden, model.balance.state)
        assertEquals(1, model.assetRows.size)
    }

    // -- the feed ------------------------------------------------------------

    private fun item(
        id: String,
        received: Boolean,
        value: String?,
        symbol: String = "USDC",
        dayStart: Long,
        alias: String? = null,
        counterparty: String? = "0x9F3c000000000000000000000000000000021aE0",
    ) = FeedItem(
        id = id,
        direction = if (received) FeedDirection.In else FeedDirection.Out,
        counterparty = counterparty,
        alias = alias,
        value = value,
        symbol = symbol,
        decimals = 6,
        chain_id = 137,
        timestamp = dayStart / 1000.0 + 3600,
        day_start_ms = dayStart.toDouble(),
    )

    private fun midnight(daysAgo: Int, now: Long): Long {
        val calendar = java.util.Calendar.getInstance().apply {
            timeInMillis = now
            set(java.util.Calendar.HOUR_OF_DAY, 0)
            set(java.util.Calendar.MINUTE, 0)
            set(java.util.Calendar.SECOND, 0)
            set(java.util.Calendar.MILLISECOND, 0)
        }
        calendar.add(java.util.Calendar.DAY_OF_YEAR, -daysAgo)
        return calendar.timeInMillis
    }

    /**
     * The core interleaves headers and items; this walks that order and never
     * re-sorts it. A shell that grouped by its own key would put a payment
     * under a day the core did not choose.
     */
    @Test
    fun `the feed keeps the core's grouping and order`() {
        val now = System.currentTimeMillis()
        val today = midnight(0, now)
        val yesterday = midnight(1, now)
        val feed = FeedView(
            rows = listOf(
                FeedRow.Header("day-$today", today.toDouble(), today / 1000.0),
                FeedRow.Item(item("a", received = false, value = "2", symbol = "POL", dayStart = today)),
                FeedRow.Item(item("b", received = true, value = "120", symbol = "USDT", dayStart = today)),
                FeedRow.Header("day-$yesterday", yesterday.toDouble(), yesterday / 1000.0),
                FeedRow.Item(item("c", received = true, value = "50", dayStart = yesterday)),
            ),
        )

        val groups = WalletLive.activity(feed, strings, now)

        assertEquals(2, groups.size)
        assertEquals(2, groups[0].rows.size)
        assertEquals(1, groups[1].rows.size)
        assertEquals(listOf("−2", "+120"), groups[0].rows.map { it.amount })
        assertEquals("POL", groups[0].rows[0].unit)
    }

    /** Today and yesterday are named; anything older shows its date. */
    @Test
    fun `day labels are relative only for the two days that deserve it`() {
        val now = System.currentTimeMillis()
        val old = midnight(9, now)
        val feed = FeedView(
            rows = listOf(
                FeedRow.Header("day-0", midnight(0, now).toDouble(), now / 1000.0),
                FeedRow.Item(item("a", received = true, value = "1", dayStart = midnight(0, now))),
                FeedRow.Header("day-1", midnight(1, now).toDouble(), now / 1000.0),
                FeedRow.Item(item("b", received = true, value = "1", dayStart = midnight(1, now))),
                FeedRow.Header("day-9", old.toDouble(), old / 1000.0),
                FeedRow.Item(item("c", received = true, value = "1", dayStart = old)),
            ),
        )

        val labels = WalletLive.activity(feed, strings, now).map { it.label }

        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.DAY_TODAY), labels[0])
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.DAY_YESTERDAY), labels[1])
        assertTrue("an older day shows a date, not a relative word", labels[2].isNotBlank())
        assertTrue(labels[2] != labels[0] && labels[2] != labels[1])
    }

    /** A resolved name wins over an address; an address is shortened, never raw. */
    @Test
    fun `a counterparty reads as a name when there is one`() {
        val now = System.currentTimeMillis()
        val today = midnight(0, now)
        val feed = FeedView(
            rows = listOf(
                FeedRow.Header("day-$today", today.toDouble(), today / 1000.0),
                FeedRow.Item(item("a", received = true, value = "50", dayStart = today, alias = "Alice")),
                FeedRow.Item(item("b", received = true, value = "50", dayStart = today)),
            ),
        )

        val rows = WalletLive.activity(feed, strings, now).single().rows

        assertTrue("a resolved name is used as-is", rows[0].subtitle.contains("Alice"))
        assertTrue("an address is shortened", rows[1].subtitle.contains("…"))
        assertTrue(
            "42 characters of hex in a list row tells nobody anything",
            rows[1].subtitle.length < 30,
        )
    }

    /** A header with nothing under it is not a day — it is a gap in the list. */
    @Test
    fun `an empty day is not rendered`() {
        val now = System.currentTimeMillis()
        val feed = FeedView(
            rows = listOf(FeedRow.Header("day-x", midnight(0, now).toDouble(), now / 1000.0)),
        )

        assertEquals(emptyList<Any>(), WalletLive.activity(feed, strings, now))
    }

    /** An empty feed says "empty", and does not keep the fixture's history. */
    @Test
    fun `an empty feed empties the section`() {
        val model = home(BalanceView(), FeedView())

        assertEquals(SectionMode.Empty, model.activitySection.mode)
        assertEquals(emptyList<Any>(), model.activityGroups)
    }

    // -- the display currency -------------------------------------------------

    private fun gbp(rate: Double? = 0.78) =
        CurrencyView(code = "GBP", rate = rate, committed = true)

    /** A settled choice with a rate converts the hero and every row with it. */
    @Test
    fun `a chosen currency converts the figures`() {
        val view = BalanceView(
            display_total_usd = 100.0,
            tokens = listOf(token("POL", "100", price = 1.0)),
        )

        val model = home(view, currency = gbp())

        assertEquals("£78", model.balance.integer)
        assertEquals("00", model.balance.decimals)
        assertEquals("GBP", model.balance.currency)
        assertEquals(AssetFiatModel.Value("£78.00"), model.assetRows.single().fiat)
    }

    /**
     * **The case that must not convert.**
     *
     * A chosen currency with no rate keeps the figure in dollars and says so.
     * Multiplying by a defaulted 1 would put a pound sign in front of a dollar
     * amount — the same number, relabelled, and wrong by whatever the rate is.
     */
    @Test
    fun `a currency nobody could price keeps showing dollars`() {
        val view = BalanceView(
            display_total_usd = 100.0,
            tokens = listOf(token("POL", "100", price = 1.0)),
        )

        val model = home(view, currency = gbp(rate = null))

        assertEquals("$100", model.balance.integer)
        assertEquals("USD", model.balance.currency)
        assertEquals(AssetFiatModel.Value("$100.00"), model.assetRows.single().fiat)
    }

    /** An uncommitted placeholder is not a choice, and does not convert either. */
    @Test
    fun `the USD placeholder does not convert`() {
        val view = BalanceView(display_total_usd = 100.0)

        val model = home(view, currency = CurrencyView(code = "GBP", rate = 0.78, committed = false))

        assertEquals("$100", model.balance.integer)
        assertEquals("USD", model.balance.currency)
    }

    /** A currency with no sign in the JVM's table reads as a code, never a wrong sign. */
    @Test
    fun `an unsigned currency shows its code`() {
        val view = BalanceView(display_total_usd = 100.0)

        val model = home(
            view,
            currency = CurrencyView(code = "CHF", rate = 0.80, committed = true),
        )

        assertEquals("CHF 80", model.balance.integer)
    }

    /** A nonsense rate is no rate: zero or infinity must never reach a figure. */
    @Test
    fun `a rate that is not a positive number is refused`() {
        val view = BalanceView(display_total_usd = 100.0)

        assertEquals("$100", home(view, currency = gbp(rate = 0.0)).balance.integer)
        assertEquals("$100", home(view, currency = gbp(rate = Double.NaN)).balance.integer)
        assertEquals(
            "$100",
            home(view, currency = gbp(rate = Double.POSITIVE_INFINITY)).balance.integer,
        )
    }
}
