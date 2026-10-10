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
import app.getvela.wallet.feature.wallet.core.BalanceBoards
import app.getvela.wallet.feature.wallet.core.BalanceBoards.FirstRead
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedLine
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey

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
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun base() = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)

    /** The chain names a device would have, from its network list. */
    private val chains = mapOf(137 to "Polygon", 42161 to "Arbitrum")

    private fun home(
        view: BalanceView,
        feed: FeedView = FeedView(),
        currency: CurrencyView = CurrencyView(code = "USD", committed = true),
        chainFilter: Int? = null,
        now: Long = System.currentTimeMillis(),
    ) = WalletLive.home(base(), view, feed, currency, strings, chains, now = now, chainFilter = chainFilter)

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

    /**
     * The network filter (spec 048) narrows the home's holdings, as the web's
     * home does: a network picked on the assets page is no longer invisible on
     * the home while the send picker already obeys it. The hero total stays
     * the whole wallet's.
     */
    @Test
    fun `the home narrows its holdings to the network filter, not its total`() {
        val view = BalanceView(
            display_total_usd = 5.01,
            tokens = listOf(
                token("ETH", "0.002", price = 2500.0, chainId = 42161, name = "Ether"),
                token("USDT", "0.01", price = 1.0, chainId = 137, name = "USDT"),
            ),
        )
        val all = home(view)
        val arbitrum = home(view, chainFilter = 42161)

        assertEquals(listOf("Arbitrum", "Polygon"), all.assetRows.map { it.chain })
        assertEquals(listOf("Arbitrum"), arbitrum.assetRows.map { it.chain })
        assertEquals("the hero is the whole wallet's", all.balance, arbitrum.balance)
    }

    /** A network holding nothing, while others do, reads as empty — not loading, not blank. */
    @Test
    fun `a network filtered down to nothing is the empty state`() {
        val view = BalanceView(
            display_total_usd = 5.0,
            holdings_loading = true,
            tokens = listOf(token("ETH", "0.002", price = 2500.0, chainId = 42161, name = "Ether")),
        )
        val base = home(view, chainFilter = 8453)

        assertTrue(base.assetRows.isEmpty())
        assertEquals(SectionMode.Empty, base.assetsSection.mode)
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
        val model = home(firstRead(FirstRead.Answered))

        assertEquals(BalanceStateKind.ZeroLive, model.balance.state)
        assertEquals("$0", model.balance.integer)
    }

    /** An empty wallet around its first read, as the real `balance_dashboard` says it. */
    private fun firstRead(stage: FirstRead) = BalanceBoards.firstRead("0x" + "ab".repeat(20), 1.7e12, stage)

    /**
     * A first launch that could read nothing, with nothing cached. The core's
     * `display_total_usd` is 0.0 here, not null — and rendering it was the bug
     * spec 038 finding 15 names: a settled-looking "$0.00" over an unreadable
     * chain. The flag keeps that number off the hero; a skeleton and a reason
     * take its place, and the three zero-ish states stay distinct.
     */
    @Test
    fun `an unreachable first load shows no number`() {
        val model = home(BalanceView(display_total_usd = 0.0, unreachable = true))

        assertEquals(BalanceStateKind.Loading, model.balance.state)
        assertNull(model.balance.integer)
        val status = model.balance.status
        assertNotNull("an empty hero with no reason given is the bug", status)
        assertEquals(BalanceStatusKind.Warning, status!!.kind)
        assertTrue(status.text.isNotBlank())
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
        val unknown = home(BalanceView(balance_unknown = true))
        val settled = home(BalanceView())

        assertEquals(SectionMode.Loading, loading.assetsSection.mode)
        assertEquals("a balance nobody could read is not an empty wallet (087 F03)", SectionMode.Loading, unknown.assetsSection.mode)
        assertEquals(SectionMode.Empty, settled.assetsSection.mode)
        assertEquals(strings.t("assets.emptyTitle"), settled.assetsSection.empty?.title)
    }

    /**
     * Spec 078: a row is written on the ONE token-amount rule every shell and
     * every Send screen uses — the core's ladder, half up (6 places under 1,
     * 4 under 1000, 2 above) — so the balance here and the balance beside the
     * token on Send are the same number. It used to be cut at six places here
     * while other shells rounded.
     */
    @Test
    fun `a long balance is written on the one token-amount ladder`() {
        val view = BalanceView(
            display_total_usd = 1.0,
            tokens = listOf(token("POL", "0.1234569999", price = 1.0), token("XDAI", "12.3456789", price = 1.0)),
        )

        val rows = home(view).assetRows.associate { it.ticker to it.balance }
        assertEquals("0.123457 POL", rows["POL"])
        assertEquals("12.3457 XDAI", rows["XDAI"])
    }

    /**
     * Spec 078 round 2: every fiat figure rounds HALF UP to two places — the
     * hero, the rows. Both used to cut (the rows through the `BigDecimal(double)`
     * constructor, which printed 12.34 as 12.33), so the founder's home read
     * CN¥63.23 over rows that added up to 63.21.
     */
    @Test
    fun `the hero and the rows round money half up, never cut`() {
        val view = BalanceView(
            display_total_usd = 12.346,
            tokens = listOf(
                token("POL", "1", price = 12.346),
                token("XDAI", "1", price = 12.34, chainId = 100),
                token("ETH", "1", price = 0.004, chainId = 1),
            ),
        )
        val home = home(view)
        // Cut, both read 12.34.
        assertEquals("35", home.balance.decimals)
        val fiat = home.assetRows.associate { it.ticker to it.fiat }
        assertEquals(AssetFiatModel.Value("$12.35"), fiat["POL"])
        // 12.34 is 12.3399… in binary: rounded, never cut to 12.33.
        assertEquals(AssetFiatModel.Value("$12.34"), fiat["XDAI"])
        assertEquals(AssetFiatModel.Value("$0.00"), fiat["ETH"])
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

    /**
     * The device already knows a figure: it paints first, marked as updating,
     * and the live total replaces it. A skeleton over a known number was a
     * hero that blinked on every open (the web's `liveBalance`).
     */
    @Test
    fun `a cached total paints first, marked as refreshing`() {
        val model = home(BalanceView(display_total_usd = null, cached_total_usd = 12.34, refreshing = true))

        assertEquals(BalanceStateKind.Normal, model.balance.state)
        assertEquals("$12", model.balance.integer)
        assertEquals("34", model.balance.decimals)
        assertEquals(BalanceStatusKind.Refreshing, model.balance.status?.kind)
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.BALANCE_STALE), model.balance.status?.text)

        // Nothing live and nothing cached is still a skeleton, not a zero.
        assertEquals(BalanceStateKind.Loading, home(BalanceView(refreshing = true)).balance.state)
    }

    /**
     * PR 3 final note F19 — "live" and "Checking…" are the core's to say.
     *
     * A wallet that held nothing last session opens with a cached total of 0.
     * This file used to call that "live" (a zero total, not partial, no
     * tokens): the home drew "Live · listening for payments" over a wallet
     * nothing had read, then swapped it for "Can't reach 24 networks". The
     * real machine, through its first read: out, answered, some missing.
     */
    @Test
    fun `a cached zero says Checking until a round has ended, and live only when the core does`() {
        val checking = strings.t("componentsUi.funding.checking")
        val listening = strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.LIVE_INDICATOR)
        assertEquals("Checking…", checking)

        // The first read is out over last session's zero.
        val out = home(firstRead(FirstRead.Out)).balance
        assertEquals(checking, out.checkingText)
        assertNull("nothing has been read: not live", out.liveText)
        assertEquals(BalanceStateKind.Normal, out.state)
        assertEquals("the cached figure stands under it", "$0", out.integer)
        assertNull("…and it is not \"still updating\": nothing has been read at all", out.status)

        // It settled, and every network answered: live.
        val answered = home(firstRead(FirstRead.Answered)).balance
        assertNull("a round has ended: no longer checking", answered.checkingText)
        assertEquals(listening, answered.liveText)
        assertEquals(BalanceStateKind.ZeroLive, answered.state)
        assertNull(answered.status)

        // It settled with three networks missing: the line says so — never live.
        val missing = WalletLive.home(base(), firstRead(FirstRead.Missing), FeedView(), CurrencyView(code = "USD", committed = true), strings, BalanceBoards.FIRST_READ_CHAINS).balance
        assertNull(missing.checkingText)
        assertNull("three networks did not answer: not a listening wallet", missing.liveText)
        assertEquals(BalanceStateKind.Normal, missing.state)
        assertEquals(BalanceStatusKind.Warning, missing.status?.kind)
        assertEquals("Can't reach 3 networks right now", missing.status?.text)

        // One line at a time, whichever the core says.
        for (balance in listOf(out, answered, missing)) {
            assertEquals(balance.toString(), 1, listOfNotNull(balance.checkingText, balance.liveText, balance.status).size)
        }
    }

    /**
     * And this shell derives neither: a zero total with every flag clear is
     * NOT live unless the core says so (it is what a cached zero looks like),
     * and whatever the flags say, `live_key` is.
     */
    @Test
    fun `zero, live is the core's key and nothing else`() {
        for (view in listOf(
            BalanceView(display_total_usd = 0.0),
            BalanceView(display_total_usd = 0.0, balance_partial = true),
            BalanceView(display_total_usd = 0.0, balance_unknown = true),
            BalanceView(display_total_usd = null, cached_total_usd = 0.0),
        )) {
            val model = home(view)
            assertEquals(view.toString(), BalanceStateKind.Normal, model.balance.state)
            assertEquals("$0", model.balance.integer)
            assertNull(view.toString(), model.balance.liveText)
            assertNull(model.balance.checkingText)
        }
        val said = home(BalanceView(display_total_usd = 0.0, live_key = app.getvela.wallet.core.i18n.I18nKeys.Wallet.LIVE_INDICATOR)).balance
        assertEquals(BalanceStateKind.ZeroLive, said.state)
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.LIVE_INDICATOR), said.liveText)
    }

    /**
     * "Checking…" stands alone on the line: under a cached figure a first
     * read is not "still updating" (the line this shell wrote there), and it
     * is said under the skeleton too — while the display currency is on its
     * way, and with nothing cached at all.
     */
    @Test
    fun `Checking stands in place of anything else the line could say`() {
        val key = "componentsUi.funding.checking"
        val cached = home(BalanceView(display_total_usd = null, cached_total_usd = 12.34, checking_key = key)).balance
        assertEquals("$12", cached.integer)
        assertEquals(strings.t(key), cached.checkingText)
        assertNull("not \"Some balances are still updating.\"", cached.status)

        val nothing = home(BalanceView(checking_key = key)).balance
        assertEquals(BalanceStateKind.Loading, nothing.state)
        assertEquals(strings.t(key), nothing.checkingText)

        val waiting = home(
            BalanceView(display_total_usd = null, cached_total_usd = 12.34, checking_key = key),
            currency = CurrencyView(code = "USD", committed = false, pending = "CNY"),
        ).balance
        assertEquals(BalanceStateKind.Loading, waiting.state)
        assertEquals(strings.t(key), waiting.checkingText)

        // Hidden: the figure is masked, the line still says what is happening.
        val hidden = home(BalanceView(hidden = true, checking_key = key)).balance
        assertEquals(BalanceStateKind.Hidden, hidden.state)
        assertEquals(strings.t(key), hidden.checkingText)

        // A later refresh is not "checking": the core stops saying it, and the line is this shell's again.
        val later = home(BalanceView(display_total_usd = null, cached_total_usd = 12.34, refreshing = true)).balance
        assertNull(later.checkingText)
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.BALANCE_STALE), later.status?.text)
    }

    /**
     * The chain that is down is named — from `unreachable_networks`, which the
     * core already cut to failed MINUS rate-limited, and in the sentence the
     * core chose (spec 092: no "RPC" on the home). A chain that is only
     * rate-limited is not in it, so the hero never nags to swap an RPC that
     * will heal on its own; the balance just says it is updating.
     */
    @Test
    fun `a failing chain is named, a rate-limited one is not`() {
        fun down(vararg ids: Int) = ids.map { app.getvela.wallet.feature.wallet.core.UnreachableNetwork(it, "not_read", null, "assets.notReadYet") }
        val one = home(
            BalanceView(display_total_usd = 4.5, failed_chain_ids = listOf(137), unreachable_networks = down(137), unreachable_key = "assets.unreachableOne"),
        ).balance.status
        assertEquals(BalanceStatusKind.Warning, one?.kind)
        assertEquals("Can't reach Polygon right now", one?.text)

        val two = home(
            BalanceView(display_total_usd = 4.5, unreachable_networks = down(137, 42161), unreachable_key = "assets.unreachableMany"),
        ).balance.status
        assertEquals("Can't reach 2 networks right now", two?.text)

        // The integration's note 4: whichever sentence the core names is the
        // one filled — a network there for its TOKEN LIST says that, never
        // "Can't reach". The line used to switch on the two keys it knew, so
        // a third said nothing at all.
        val list = home(
            BalanceView(
                display_total_usd = 4.5,
                unreachable_networks = listOf(
                    app.getvela.wallet.feature.wallet.core.UnreachableNetwork(137, "held", 12.0, "assets.lastSeen", cause = "token_list", rpc_fixable = false),
                ),
                unreachable_key = "assets.tokenListUnreachable",
            ),
        ).balance.status
        assertEquals(BalanceStatusKind.Warning, list?.kind)
        assertEquals("Can't load Polygon's token list right now", list?.text)

        val limited = home(
            BalanceView(
                display_total_usd = 4.5,
                notice = app.getvela.wallet.feature.wallet.core.BalanceNotice.StillUpdating,
                failed_chain_ids = listOf(137),
                rate_limited_chain_ids = listOf(137),
            ),
        ).balance.status
        assertEquals(BalanceStatusKind.Refreshing, limited?.kind)

        val unpriced = home(BalanceView(display_total_usd = 4.5, notice = app.getvela.wallet.feature.wallet.core.BalanceNotice.Unpriced)).balance.status
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.Wallet.BALANCE_UNPRICED), unpriced?.text)
        assertNull(home(BalanceView(display_total_usd = 4.5)).balance.status)
    }

    /**
     * Issue 462: a refresh the person asked for is the control's to show — it
     * turns and says "Updating…" — and never a status line. When it was one,
     * every tap pushed "Some balances are still updating." in above the
     * control, and the control slid out from under the finger.
     */
    @Test
    fun `a refresh the person asked for adds no status line, the control says so`() {
        val now = 1_800_000_000_000L
        val idle = home(BalanceView(display_total_usd = 4.5, last_refreshed_at_ms = (now - 125_000L).toDouble()), now = now).balance
        assertNull(idle.status)
        assertEquals("Updated 2m", idle.refresh?.updated)
        assertEquals("Updating…", idle.refresh?.updating)
        assertFalse(idle.refresh!!.refreshing)

        val pulling = home(
            BalanceView(display_total_usd = 4.5, refreshing = true, last_refreshed_at_ms = (now - 125_000L).toDouble()),
            now = now,
        ).balance
        assertNull("the refresh draws no line above the control", pulling.status)
        assertTrue(pulling.refresh!!.refreshing)
        // The label beside it stays the last settle's until the round settles.
        assertEquals("Updated 2m", pulling.refresh?.updated)

        // Nothing read yet: the glyph alone, and no line either — a skeleton
        // with a refresh out is still a skeleton.
        val first = home(BalanceView(refreshing = true), now = now).balance
        assertEquals(BalanceStateKind.Loading, first.state)
        assertNull(first.status)
        assertNull(first.refresh?.updated)
        // The glyph alone still has a name, and it is not "Updating…": that
        // would be read over a control at rest (C7).
        assertEquals("Refresh balance", first.refresh?.idleLabel)
        assertNotEquals(first.refresh?.updating, first.refresh?.idleLabel)

        // Hidden figures keep the control: it reads, it shows no number.
        val hidden = home(BalanceView(display_total_usd = 4.5, hidden = true, last_refreshed_at_ms = now.toDouble()), now = now).balance
        assertEquals(BalanceStateKind.Hidden, hidden.state)
        assertEquals("Updated now", hidden.refresh?.updated)
    }

    /**
     * The label ages in the core's words: under 45 s is "now", then minutes,
     * then hours, then the weekday under a week, then the date.
     */
    @Test
    fun `the updated label ages in the cores relative words`() {
        // A Friday, 08:00 UTC.
        val now = 1_800_000_000_000L
        fun label(agoMs: Long) = WalletLive.refresh(BalanceView(last_refreshed_at_ms = (now - agoMs).toDouble()), strings, now).updated
        assertEquals("Updated now", label(0))
        assertEquals("Updated now", label(44_000))
        assertEquals("Updated 1m", label(45_000))
        assertEquals("Updated 2m", label(125_000))
        assertEquals("Updated 3m", label(150_000))
        assertEquals("Updated 59m", label(3_540_000))
        assertEquals("Updated 1h", label(3_600_000))
        assertEquals("Updated 3h", label(3 * 3_600_000L + 10 * 60_000L))
        // A clock behind the settle (read before it landed) is "now", never negative.
        assertEquals("Updated now", label(-5_000))
        // Under a week, the weekday — the shell's old port drew a date here.
        // Three days back is a Tuesday in every zone from UTC−8 to UTC+15.
        assertEquals("Updated Tue", label(3 * 86_400_000L))
        // Past a week, the date in the person's own preset.
        assertEquals("Updated " + Formats.current.date(now - 30 * 86_400_000L), label(30 * 86_400_000L))
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
        // The second line as the core words a transfer (spec 093): the person, by name or address.
        subtitle = listOf(
            counterparty?.let { if (received) FeedLine.From(it, alias) else FeedLine.To(it, alias) } ?: FeedLine.Network(137),
        ),
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

    /**
     * 087 F11: a dApp call that moved no coin of ours ("dApp 交易 · 处理中 ·
     * <site>") has no figure, and its row draws no amount cell — the empty
     * cell took half the row and printed a lone " ". A row with a figure keeps it.
     */
    @Test
    fun `a row with no figure draws no amount cell`() {
        val now = System.currentTimeMillis()
        val today = midnight(0, now)
        val call = item("d", received = false, value = null, symbol = "", dayStart = today)
            .copy(
                kind = app.getvela.wallet.feature.wallet.core.FeedTxKind.DappTx,
                site = "app.uniswap.org",
                subtitle = listOf(FeedLine.Site("app.uniswap.org"), FeedLine.Network(137)),
            )
        val feed = FeedView(
            rows = listOf(
                FeedRow.Header("day-$today", today.toDouble(), today / 1000.0),
                FeedRow.Item(call),
                FeedRow.Item(item("s", received = false, value = "1.5", dayStart = today)),
            ),
        )

        val rows = WalletLive.activity(feed, strings, now, chains).single().rows

        assertEquals("", rows[0].amount)
        assertEquals("", rows[0].unit)
        assertFalse(rows[0].hasFigure)
        assertEquals("the core's parts, worded and joined", "app.uniswap.org · Polygon", rows[0].subtitle)
        assertTrue(rows[1].hasFigure)
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

    /**
     * The 102 device run: a home read "$1,234 · USD" for a few seconds and
     * then jumped to "¥8,876 · CNY". While the display currency is not the
     * person's yet (`committed == false`, the core's USD placeholder) NO
     * figure in it is drawn — the total and each holding's worth wait, in
     * their loading state — and the label names the stored choice on its way
     * (`pending`), or nothing at all before one is known. Never "USD" for a
     * person who did not choose it.
     */
    @Test
    fun `no figure is drawn in a currency that is not the person's yet`() {
        val view = BalanceView(
            display_total_usd = 100.0,
            tokens = listOf(token("POL", "100", price = 1.0)),
        )

        // The stored choice (CNY) is being priced: its code, and no figure.
        val waiting = home(view, currency = CurrencyView(code = "USD", rate = null, committed = false, pending = "CNY"))
        assertEquals(BalanceStateKind.Loading, waiting.balance.state)
        assertNull(waiting.balance.integer)
        assertNull(waiting.balance.decimals)
        assertEquals("CNY", waiting.balance.currency)
        assertEquals(AssetFiatModel.Loading, waiting.assetRows.single().fiat)
        // What is held is not a figure in the display currency: it is shown.
        assertEquals("100 POL", waiting.assetRows.single().balance)

        // Before the preference is read there is no code to name either.
        val unread = home(view, currency = CurrencyView(code = "USD", rate = null, committed = false))
        assertEquals(BalanceStateKind.Loading, unread.balance.state)
        assertEquals("", unread.balance.currency)

        // Committed: the figure appears once, in the right money, under the same label.
        val settled = home(view, currency = CurrencyView(code = "CNY", rate = 7.1, committed = true))
        assertEquals(BalanceStateKind.Normal, settled.balance.state)
        assertEquals("CN¥710", settled.balance.integer)
        assertEquals("CNY", settled.balance.currency)
        assertEquals(AssetFiatModel.Value("CN¥710.00"), settled.assetRows.single().fiat)

        // Hidden stays hidden — the mask, and the same label rule.
        val hidden = home(view.copy(hidden = true), currency = CurrencyView(code = "USD", committed = false, pending = "CNY"))
        assertEquals(BalanceStateKind.Hidden, hidden.balance.state)
        assertEquals("CNY", hidden.balance.currency)
        assertEquals(AssetFiatModel.Masked, hidden.assetRows.single().fiat)
    }

    /**
     * A hidden amount keeps its unit — and the rule is the core's own
     * function (`maskedAmount`, the export of `privacy::masked_amount`), not
     * a copy of it here: the mask, then the unit; no unit, the mask alone,
     * never a trailing space. What this shell draws with it is checked where
     * it is drawn: a dApp row's "received" below, a detail's figures in
     * `PrivacyFixtureTest`.
     */
    @Test
    fun `a masked amount keeps its unit and never a trailing space`() {
        fun masked(unit: String): String = uniffi.vela_core_uniffi.maskedAmount(unit)
        assertEquals("•••• xDAI", masked("xDAI"))
        assertEquals("${WalletLive.MASK} USDC", masked("USDC"))
        assertEquals(WalletLive.MASK, masked(""))
        assertEquals(WalletLive.MASK, masked("  "))
        assertTrue(masked("ETH").none { it.isDigit() })

        // …and the row that draws one draws the core's: a swap's coin back, hidden.
        val swap = WalletFixtures.liveHiddenFeed()
        val row = WalletLive.activity(swap, strings).flatMap { it.rows }.single { it.id == "swap" }
        assertEquals(masked("xDAI"), row.received)
        assertEquals("•••• xDAI", row.received)
    }

    /** The label names the currency in every state — a total still being read does not borrow the board's "USD". */
    @Test
    fun `a total still loading already names the person's currency`() {
        val model = home(BalanceView(refreshing = true), currency = gbp())
        assertEquals(BalanceStateKind.Loading, model.balance.state)
        assertEquals("GBP", model.balance.currency)
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

    /** Spec 049: the founder stored `dot_comma` and the hero printed `CN¥3` `.63` — a mark that reads as a second thousands separator. */
    @Test
    fun `the hero and the rows follow the number preset`() {
        val saved = Formats.current
        Formats.current = Formats(NumberFormatKey.DotComma)
        try {
            val view = BalanceView(
                display_total_usd = 1234.5,
                tokens = listOf(token("POL", "10.25", price = 120.0)),
            )

            val model = home(view)

            assertEquals("$1.234", model.balance.integer)
            assertEquals("50", model.balance.decimals)
            assertEquals(",", model.balance.decimalMark)
            assertEquals(AssetFiatModel.Value("$1.230,00"), model.assetRows[0].fiat)
            // Token amounts take the mark and stay ungrouped (the web's `trimBalance`).
            assertTrue(model.assetRows[0].balance, model.assetRows[0].balance.startsWith("10,25 "))
        } finally {
            Formats.current = saved
        }
        assertEquals(".", home(BalanceView(display_total_usd = 1.0, tokens = listOf(token("POL", "1", price = 1.0)))).balance.decimalMark)
    }
}
