package app.getvela.wallet.feature.wallet

import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.feature.wallet.core.BalanceSwitcherView
import app.getvela.wallet.feature.settings.AccountsSheetRowModel
import app.getvela.wallet.feature.settings.AccountsSheetModel
import app.getvela.wallet.feature.browser.ExploreLive
import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.feature.flows.TokenMarkModel
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.core.BalanceNotice
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import java.math.BigDecimal
import java.math.RoundingMode
import java.text.DateFormat
import java.util.Calendar
import java.util.Date

/**
 * The live wallet-home builders: `BalanceView` → the display models the drawn
 * components already consume.
 *
 * The sibling of [WalletFixtures]; the gallery keeps its canon, this renders
 * one person's holdings.
 *
 * **The distinctions this file exists to preserve.** A money screen has three
 * ways to lie, and the core has already decided the truth for each:
 *
 * - **Unknown is not zero.** `display_total_usd == null` means nothing could be
 *   totalled. It renders as a loading or unknown state, never as `$0`.
 * - **Unpriced is not worthless.** A holding with no `price_usd` shows its
 *   amount and says the price is unavailable; it is not counted and not shown
 *   as `$0.00`.
 * - **Busy is not broken.** A rate-limited chain is transient; the core keeps
 *   the two lists apart and so does this.
 */
object WalletLive {

    /**
     * The home screen, from the person's own holdings.
     *
     * [fallback] supplies everything that is content rather than data — section
     * titles, action labels, empty-state copy, the tab bar — already resolved
     * through the i18n engine by the fixture builder.
     *
     * [chainNames] is what each chain is called, from the `network_admin`
     * machine. A holding's row says which chain it is on, and that is NOT the
     * token's own name — `BalanceToken.name` is "Ether", "USDT", "Wrapped
     * Polygon Ecosystem Token". An earlier version read the chain off that
     * field and was right only because the shell had been writing chain names
     * into it; the moment real token names arrived, every row read "USDT USDT".
     */
    fun home(
        fallback: WalletHomeModel,
        view: BalanceView,
        feed: FeedView,
        currency: CurrencyView,
        strings: VelaStrings,
        chainNames: Map<Int, String>,
        now: Long = System.currentTimeMillis(),
    ): WalletHomeModel {
        val money = Money.of(currency)
        val rows = assetRows(view, chainNames, currency)
        val groups = activity(feed, strings, now)
        return fallback.copy(
            balance = balance(fallback.balance, view, strings, money, chainNames),
            activitySection = fallback.activitySection.copy(
                mode = if (groups.isEmpty()) SectionMode.Empty else SectionMode.Rows,
            ),
            activityGroups = groups,
            assetsSection = fallback.assetsSection.copy(
                mode = when {
                    rows.isNotEmpty() -> SectionMode.Rows
                    view.holdings_loading -> SectionMode.Loading
                    else -> SectionMode.Empty
                },
            ),
            assetRows = rows,
        )
    }

    /**
     * The feed, from the core's already-grouped rows.
     *
     * **The order and the grouping are not this function's.** `activity_feed`
     * emits headers and items already interleaved, precisely so a shell cannot
     * sort a header away from its day. This walks that list in order and never
     * re-sorts it.
     */
    fun activity(
        feed: FeedView,
        strings: VelaStrings,
        now: Long = System.currentTimeMillis(),
    ): List<ActivityGroupModel> {
        val groups = mutableListOf<ActivityGroupModel>()
        for (row in feed.rows) {
            when (row) {
                is FeedRow.Header -> groups += ActivityGroupModel(dayLabel(row, strings, now), emptyList())
                is FeedRow.Item -> {
                    val model = activityRow(row.item, strings)
                    val last = groups.lastOrNull()
                    if (last == null) {
                        // An item before any header cannot happen — but if the
                        // core ever emits one, it gets shown rather than
                        // dropped. A payment silently missing from a history is
                        // the worse failure.
                        groups += ActivityGroupModel("", listOf(model))
                    } else {
                        groups[groups.lastIndex] = last.copy(rows = last.rows + model)
                    }
                }
            }
        }
        return groups.filter { it.rows.isNotEmpty() }
    }

    /**
     * "Today", "Yesterday", or the date.
     *
     * The comparison is between LOCAL midnights: the core stamped
     * `day_start_ms` with this device's own midnight, so comparing it against
     * today's gives whole days apart without any timezone arithmetic here.
     */
    private fun dayLabel(header: FeedRow.Header, strings: VelaStrings, now: Long): String {
        val today = Calendar.getInstance().apply {
            timeInMillis = now
            set(Calendar.HOUR_OF_DAY, 0)
            set(Calendar.MINUTE, 0)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
        }.timeInMillis
        val days = ((today - header.day_start_ms.toLong()) / DAY_MS)
        return when (days) {
            0L -> strings.t(I18nKeys.Wallet.DAY_TODAY)
            1L -> strings.t(I18nKeys.Wallet.DAY_YESTERDAY)
            else -> Formats.current.date(header.day_start_ms.toLong())
        }
    }

    private fun activityRow(item: FeedItem, strings: VelaStrings): ActivityRowModel {
        val received = item.direction == FeedDirection.In
        val batch = item.batch
        return ActivityRowModel(
            id = item.id,
            kind = when {
                // A dApp transaction is a send whose counterparty is a
                // contract; the core does not label it, because what to call it
                // is display vocabulary.
                item.batch != null -> if (received) ActivityKind.Received else ActivityKind.Sent
                received -> ActivityKind.Received
                else -> ActivityKind.Sent
            },
            title = strings.t(
                if (received) I18nKeys.Wallet.LABEL_RECEIVED else I18nKeys.Wallet.LABEL_SENT,
            ),
            subtitle = counterparty(item, strings),
            amount = signedAmount(item, received),
            unit = item.symbol.ifBlank { batch?.symbol.orEmpty() },
            positive = received,
            masked = false,
            badgeColor = badgeColour(item.chain_id),
            badgeLogoUrl = Marks.chainLogoUrl(item.chain_id),
        )
    }

    /**
     * The counterparty line: a resolved name, or a shortened address.
     *
     * `alias` is the core's overlay — a resolved identity, falling back to the
     * name captured when the payment was sent. An address with neither is
     * shortened rather than shown in full; the whole 42 characters in a list
     * row is unreadable and tells nobody anything more.
     */
    private fun counterparty(item: FeedItem, strings: VelaStrings): String {
        val received = item.direction == FeedDirection.In
        val name = item.alias ?: item.counterparty?.let(::shortenAddress) ?: return ""
        return strings.t(
            if (received) I18nKeys.Wallet.FROM_NAME else I18nKeys.Wallet.TO_NAME,
            mapOf("name" to name),
        )
    }

    private fun shortenAddress(address: String): String =
        if (address.length <= 12) address else address.take(6) + "…" + address.takeLast(4)

    /**
     * The amount, signed.
     *
     * Truncated, never rounded — the same rule as an asset row. A batch of
     * mixed tokens has no single amount, so the core sends none and the row
     * says how many assets moved instead.
     */
    private fun signedAmount(item: FeedItem, received: Boolean): String {
        val raw = item.value ?: return item.batch?.count?.toString() ?: ""
        val parsed = raw.toBigDecimalOrNull() ?: return raw
        val trimmed = Formats.current.plain(parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString())
        return if (received) "+$trimmed" else "−$trimmed"
    }

    /**
     * Every holding as a row.
     *
     * Shared with the assets screen and the token detail, so a holding reads
     * the same wherever it appears — and so the chain label, the truncation and
     * the currency are decided once.
     */
    fun assetRows(
        view: BalanceView,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
    ): List<AssetRowModel> {
        val money = Money.of(currency)
        return view.tokens.map { token -> assetRow(token, chainNames, money) }
    }

    /**
     * The hero figure.
     *
     * Four states, and choosing between them is the only judgement here — the
     * amounts themselves are the core's.
     */
    private fun balance(
        fallback: BalanceModel,
        view: BalanceView,
        strings: VelaStrings,
        money: Money,
        chainNames: Map<Int, String>,
    ): BalanceModel {
        val live = balanceVisible(fallback, view, strings, money, chainNames)
        // Spec 048 (device-found): hidden used to return the FIXTURE with a hidden
        // state — "$1,383 · USD" under the eye. Hidden is the live label with
        // the figures masked.
        // The currency is the person's own even while hidden: with the total withheld the
        // visible builder falls back to the drawn "USD".
        return if (view.hidden) live.copy(state = BalanceStateKind.Hidden, integer = "••••", decimals = null, currency = money.code) else live
    }

    private fun balanceVisible(
        fallback: BalanceModel,
        view: BalanceView,
        strings: VelaStrings,
        money: Money,
        chainNames: Map<Int, String>,
    ): BalanceModel {

        // The core withholds the display total while a fetch is out; the
        // last-known cached total paints first and live replaces it
        // (max(live, cached) is the core's rule — this only chooses what to
        // show meanwhile, as the web's `liveBalance` does). A skeleton over a
        // figure the device already knows was a hero that blinked on every open.
        val total = view.display_total_usd ?: view.cached_total_usd

        // **Unreachable is not zero.** A first launch that could read nothing,
        // with nothing cached: the core's figure here is 0.0 — `total` is not
        // null — and rendering it was spec 038 finding 15, a settled-looking
        // "$0.00" over an unreadable chain. The flag exists to keep that number
        // off the hero. A skeleton and a reason, the same reason the web and
        // desktop heroes give.
        if (view.unreachable) {
            return fallback.copy(
                state = BalanceStateKind.Loading,
                integer = null,
                decimals = null,
                status = BalanceStatusModel(
                    kind = BalanceStatusKind.Warning,
                    text = strings.t(I18nKeys.Wallet.BALANCE_UNREACHABLE),
                ),
            )
        }

        // **A total of zero is only honest when it is one.**
        //
        // The core values an unpriced holding at nothing (`price_usd` folded in
        // as 0), so a wallet whose every coin is unpriced totals exactly $0.00 —
        // and the first device run rendered that as a confident "$0.00" over two
        // real holdings. The drawn design pairs a total with an "some holdings
        // could not be priced" warning, which assumes SOME of them were; when
        // none were, there is no total to show and the skeleton is the truthful
        // shape. Phase 4c removes this state by giving prices a source.
        val nothingPriceable = view.tokens.isNotEmpty() && view.tokens.all { it.price_usd == null }
        if (nothingPriceable) {
            return fallback.copy(
                state = BalanceStateKind.Loading,
                integer = null,
                decimals = null,
                // Built here rather than borrowed from the fixture: the H1
                // state carries no status at all, so a `?.copy` produced a
                // bare skeleton with no explanation — a person staring at an
                // empty hero with two holdings underneath and no reason given.
                // Seen on the device before it was fixed.
                status = BalanceStatusModel(
                    kind = BalanceStatusKind.Warning,
                    text = strings.t(I18nKeys.Wallet.BALANCE_UNPRICED),
                ),
            )
        }

        if (total == null) {
            // Unknown. Either still counting, or nothing could be priced —
            // both render as "not a number yet" rather than as zero, and the
            // core's own notice says which.
            return fallback.copy(
                state = BalanceStateKind.Loading,
                integer = null,
                decimals = null,
                status = fallback.status?.takeIf { view.refreshing },
            )
        }

        // `valueOf`, not the constructor: `BigDecimal(12.34)` is the binary
        // double's exact expansion, 12.3399…, and cutting THAT printed $12.33.
        val rounded = BigDecimal.valueOf(money.convert(total)).setScale(2, RoundingMode.DOWN)
        val whole = rounded.toBigInteger()
        val cents = rounded.subtract(BigDecimal(whole)).movePointRight(2).abs().toBigInteger()
        // A zero is "live" only once EVERY chain has answered: a zero with a
        // chain unread (partial) or unknown is not a listening wallet, it is an
        // unknown one — and a cached zero is not live at all.
        val zeroLive = rounded.signum() == 0 && view.display_total_usd != null &&
            !view.balance_unknown && !view.balance_partial && view.tokens.isEmpty()
        return fallback.copy(
            state = if (zeroLive) BalanceStateKind.ZeroLive else BalanceStateKind.Normal,
            integer = money.symbol + Formats.current.groupDigits(whole.toString()),
            decimals = cents.toString().padStart(2, '0'),
            decimalMark = Formats.current.decimalMark(),
            // The label beside the figure names the currency it is in.
            currency = money.code,
            liveText = if (zeroLive) strings.t(I18nKeys.Wallet.LIVE_INDICATOR) else null,
            status = balanceStatus(view, strings, chainNames),
        )
    }

    /**
     * The one line under the hero, most actionable first (the web's
     * `liveBalance`). `banner_chain_ids` is already failed MINUS rate-limited —
     * the core's exclusion: a rate limit heals on its own, so the balance
     * quietly stays on cache with no "fix your RPC" nag — so a chain named here
     * really is unreachable and the person can fix its RPC. Then the refresh
     * (or the cached figure standing in for the live one), then the core's
     * notice.
     */
    internal fun balanceStatus(view: BalanceView, strings: VelaStrings, chainNames: Map<Int, String>): BalanceStatusModel? {
        val banner = view.banner_chain_ids
        val onCache = view.display_total_usd == null && view.cached_total_usd != null
        return when {
            banner.size == 1 -> BalanceStatusModel(
                BalanceStatusKind.Warning,
                strings.t(I18nKeys.Wallet.RPC_UNAVAILABLE_SINGLE, mapOf("name" to (chainNames[banner[0]] ?: banner[0].toString()))),
            )
            banner.size > 1 -> BalanceStatusModel(
                BalanceStatusKind.Warning,
                strings.t(I18nKeys.SettingsUi.RPC_UNAVAILABLE_MULTIPLE, mapOf("count" to banner.size.toString())),
            )
            view.refreshing || onCache || view.notice == BalanceNotice.StillUpdating ->
                BalanceStatusModel(BalanceStatusKind.Refreshing, strings.t(I18nKeys.Wallet.BALANCE_STALE))
            view.notice == BalanceNotice.Unpriced ->
                BalanceStatusModel(BalanceStatusKind.Warning, strings.t(I18nKeys.Wallet.BALANCE_UNPRICED))
            else -> null
        }
    }

    /**
     * A holding's identity: the chain it is on and the contract, or `native`.
     *
     * The same shape `receive_watch` uses for its baseline keys, so the two
     * cannot disagree about which holding is which.
     */
    fun holdingId(chainId: Int, contract: String?): String =
        "$chainId:${contract?.lowercase() ?: "native"}"

    private fun assetRow(
        token: BalanceToken,
        chainNames: Map<Int, String>,
        money: Money,
    ): AssetRowModel = AssetRowModel(
        id = holdingId(token.chain_id, token.token_address),
        ticker = token.symbol,
        // The chain, falling back to the token's own name only when this
        // device has no row for the chain — never a blank line.
        chain = chainNames[token.chain_id] ?: token.name,
        badgeColor = badgeColour(token.chain_id),
        logoUrls = Marks.tokenMark(token.chain_id, token.symbol, token.token_address).logoUrls,
        badgeLogoUrl = Marks.tokenMark(token.chain_id, token.symbol, token.token_address).badgeLogoUrl,
        badgeHidden = Marks.tokenMark(token.chain_id, token.symbol, token.token_address).badgeHidden,
        balance = "${trimAmount(token.balance)} ${token.symbol}",
        fiat = token.price_usd?.let { price ->
            val value = money.convert(amountAsDouble(token.balance) * price)
            AssetFiatModel.Value(money.symbol + Formats.current.fixed2(value))
        } ?: AssetFiatModel.NoPrice("—"),
        masked = false,
    )

    /**
     * A holding's amount, shortened for a row.
     *
     * Six decimals is where a phone row stops being readable; the full figure
     * lives on the asset's own screen. Truncated rather than rounded, because a
     * rounded-up balance is a number the person does not have.
     */
    internal fun trimAmount(balance: String): String {
        val parsed = balance.toBigDecimalOrNull() ?: return balance
        // The decimal mark is the preset's; the grouping stays off (spec 049, the web's `trimBalance`).
        return Formats.current.plain(parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString())
    }

    private fun amountAsDouble(balance: String): Double = balance.toDoubleOrNull() ?: 0.0

    private fun groupThousands(digits: String): String =
        digits.reversed().chunked(3).joinToString(",").reversed()

    /**
     * A stable colour per chain, so a token keeps its badge between launches —
     * and so a network is the same colour wherever it appears, including on the
     * receive screen.
     */
    fun badge(chainId: Long): Color = badgeColour(chainId.toInt())

    /**
     * Spec 047: the home's account switcher — every account on this device
     * with the total the balance machine keeps for it (`switcher.balances`,
     * filled after `SwitcherOpened`), the active one ticked. A total not yet
     * known is blank rather than a zero the person does not have.
     */
    fun accountSwitcher(
        accounts: List<Pair<String, String>>,
        activeIndex: Int,
        switcher: BalanceSwitcherView,
        currency: CurrencyView,
        strings: VelaStrings,
    ): AccountsSheetModel {
        val money = Money.of(currency)
        // "2 accounts · Total $5.65": the corpus's count line ends in the
        // separator so the total follows it, the way the web's sheet reads.
        val total = switcher.balances.sumOf { it.usd }
        val known = switcher.balances.isNotEmpty()
        val count = strings.t(I18nKeys.SettingsUi.ACCOUNTS_COUNT, mapOf("count" to accounts.size.toString()))
        return AccountsSheetModel(
            title = strings.t(I18nKeys.SettingsUi.ACCOUNTS_TITLE),
            summary = if (known) count + strings.t(I18nKeys.SettingsUi.ACCOUNTS_TOTAL, mapOf("amount" to money.fiat(total))) else count.trimEnd(' ', '·'),
            rows = accounts.mapIndexed { i, (name, address) ->
                val short = ExploreLive.shortAddress(address)
                val usd = switcher.balances.firstOrNull { it.address.equals(address, ignoreCase = true) }?.usd
                AccountsSheetRowModel(
                    name = name.ifBlank { short },
                    addressDisplay = short,
                    addressFull = address,
                    amount = usd?.let { money.fiat(it) } ?: "",
                    selected = i == activeIndex,
                )
            },
            primary = strings.t(I18nKeys.SettingsUi.ACCOUNT_CREATE),
            secondary = strings.t(I18nKeys.SettingsUi.ACCOUNT_SIGN_IN),
        )
    }

    /**
     * Spec 047: every token mark on the phone — the drawn colour plus the
     * web's logo rules (`tokenMarkFor`): the coin's chain logo for a native
     * coin, the asset entry for a token, the badge hidden when it would repeat.
     */
    fun mark(chainId: Int, symbol: String, tokenAddress: String?, logoUrls: List<String> = emptyList()): TokenMarkModel {
        val m = Marks.tokenMark(chainId, symbol, tokenAddress, logoUrls)
        return TokenMarkModel(symbol, badgeColour(chainId), m.logoUrls, m.badgeLogoUrl, m.badgeHidden)
    }

    private fun badgeColour(chainId: Int): Color = BADGES[chainId.mod(BADGES.size)]

    /**
     * The display currency, as the money on this screen is written.
     *
     * **A null rate is not 1.** The core resolves the rate and leaves it null
     * when nothing could price the currency; converting anyway would tell
     * somebody in Tokyo that their ¥150,000 is $150,000. So an unpriced
     * currency keeps the figure in dollars and says USD — which is what the
     * hero showed for the whole of phases 4 and 5, correctly, before there was
     * a rate to use.
     */
    class Money private constructor(
        val code: String,
        val symbol: String,
        private val rate: Double?,
    ) {
        fun convert(usd: Double): Double = rate?.let { usd * it } ?: usd

        /** A fiat figure in this money, drawn with the person's number format (spec 047 D2). */
        fun fiat(usd: Double): String = symbol + Formats.current.fixed2(convert(usd))

        companion object {
            /** Dollars, unconverted — the default before a currency is known. */
            fun dollars(): Money = Money("USD", "$", null)

            fun of(view: CurrencyView): Money {
                val rate = view.rate?.takeIf { it.isFinite() && it > 0.0 }
                // No rate, or no settled choice, means dollars — and saying so.
                if (rate == null || !view.committed) return Money("USD", "$", null)
                return Money(view.code, symbolFor(view.code), rate)
            }

            /**
             * The currency's own sign, from the JVM's ISO-4217 table rather
             * than a table of this app's own. An unknown code falls back to the
             * code itself, which reads as "CHF 12.00" — plain, and never the
             * wrong sign in front of a number.
             */
            private fun symbolFor(code: String): String = runCatching {
                java.util.Currency.getInstance(code).getSymbol(java.util.Locale.US)
            }.getOrNull()?.takeIf { it != code } ?: "$code "
        }
    }

    private const val DAY_MS = 24L * 60 * 60 * 1000

    private val BADGES = listOf(
        Color(0xFF6C7BFF), Color(0xFF2E9E7E), Color(0xFFE0A03A), Color(0xFF8C8C8C),
        Color(0xFFCF5C7A), Color(0xFF4A9BD1), Color(0xFF9B6CD1), Color(0xFF3FA37A),
    )
}
