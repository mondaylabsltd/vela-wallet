package app.getvela.wallet.feature.wallet

import app.getvela.wallet.core.format.tokenAmountText
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.RelativeTime
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
import app.getvela.wallet.feature.wallet.core.FeedAllowance
import app.getvela.wallet.feature.wallet.core.FeedDapp
import app.getvela.wallet.feature.wallet.core.FeedDappChange
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedItem
import app.getvela.wallet.feature.wallet.core.FeedLine
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.FeedTxKind
import app.getvela.wallet.feature.wallet.core.FeedTxStatus
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
 *
 * **Hidden is hidden everywhere** (`app::privacy`, one rule): while the
 * balance is hidden, every money figure these builders draw — the hero, a
 * holding's amount AND its worth, an activity row's figure (by the core's
 * `figure_maskable`) and a dApp row's "received", the account switcher's rows
 * and total — reads [MASK] (the hero [BALANCE_MASK]). The holdings mask on
 * `BalanceView.hidden`; everything drawn from the feed masks on the feed's
 * own `FeedView.hidden`. Send, the signing sheet and Receive keep their
 * figures on purpose and do not come through here.
 */
object WalletLive {

    /** A masked figure: the same four dots on every surface, every shell (`privacy::MASK`). */
    const val MASK = "••••"

    /** The hero's mask, one glyph wider (`privacy::BALANCE_MASK`). */
    const val BALANCE_MASK = "••••••"

    /**
     * A masked amount that is written WITH its unit: the dots, then the coin
     * — "•••• xDAI". The core's rule (`privacy::masked_amount`): the unit
     * stays, because it says what kind of money without saying how much, and
     * the secret is the number. A row that draws its unit apart from the
     * figure has always kept it; a transfer's detail dropped it ("••••")
     * while iOS kept it. A figure with no unit of its own — a holding under
     * its ticker, a worth in the display currency — is [MASK] alone, never a
     * trailing space.
     */
    fun masked(unit: String): String = unit.trim().let { if (it.isEmpty()) MASK else "$MASK $it" }

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
        /**
         * The network filter the assets page, history and send picker share
         * (spec 048). The home narrows its holdings to it as the web's does
         * (`liveSections`); the feed arrives already narrowed by its machine,
         * and the hero total stays the whole wallet's.
         */
        chainFilter: Int? = null,
    ): WalletHomeModel {
        val money = Money.of(currency)
        val rows = assetRows(view, chainNames, currency)
            .filter { chainFilter == null || it.id.startsWith("$chainFilter:") }
        // Issue #469: the home draws the core's short list (`home_rows`, the
        // newest three); History keeps every row. The cut is the core's — the
        // shared helper below also builds History, so it never caps.
        val groups = activity(feed.copy(rows = feed.home_rows), strings, now, chainNames)
        return fallback.copy(
            balance = balance(fallback.balance, view, strings, money, chainNames).copy(refresh = refresh(view, strings, now)),
            activitySection = fallback.activitySection.copy(
                mode = if (groups.isEmpty()) SectionMode.Empty else SectionMode.Rows,
                // Spec 082 RG5: which empty line — "no activity yet" or "none
                // on this network" — is the core's, by the chain filter.
                empty = fallback.activitySection.empty?.copy(title = strings.t(feed.home_empty_key)),
            ),
            activityGroups = groups,
            assetsSection = fallback.assetsSection.copy(
                mode = when {
                    rows.isNotEmpty() -> SectionMode.Rows
                    // A network filtered down to nothing reads as the empty
                    // state, not as a list still loading or a blank one.
                    chainFilter != null && view.tokens.isNotEmpty() -> SectionMode.Empty
                    // "Nothing here" is a claim: not while the first read is
                    // out, and not while the balance cannot be read at all —
                    // the web's `assetsMode`, the desktop's and the iPhone's
                    // (087 F03).
                    // Nor while nothing could be read at all — every chain
                    // failed, or the fetch threw, with nothing cached
                    // (`unreachable`): "Deposit your first asset" under the
                    // reason would be a claim nobody made (PR 2 integration).
                    view.holdings_loading || view.balance_unknown || view.unreachable -> SectionMode.Loading
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
        /** What each network is called, for the core's `network` parts of a row's second line (spec 093). */
        chainNames: Map<Int, String> = emptyMap(),
    ): List<ActivityGroupModel> {
        val groups = mutableListOf<ActivityGroupModel>()
        for (row in feed.rows) {
            when (row) {
                is FeedRow.Header -> groups += ActivityGroupModel(dayLabel(row.day_start_ms, strings, now), emptyList())
                is FeedRow.Item -> {
                    val model = activityRow(row.item, strings, chainNames, now, feed.hidden)
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
     * Spec 093: rows with no headers — a contact's page draws the core's
     * `contact_rows` with the very builder Activity uses, so a dApp's payment
     * to that person reads as its verb there too. Their second line carries
     * the day instead.
     */
    fun rows(
        items: List<FeedItem>,
        strings: VelaStrings,
        chainNames: Map<Int, String> = emptyMap(),
        now: Long = System.currentTimeMillis(),
        /** The feed's `hidden` — a contact's page masks as Activity does. */
        hidden: Boolean = false,
    ): List<ActivityRowModel> = items.map { activityRow(it, strings, chainNames, now, hidden) }

    /**
     * "Today", "Yesterday", or the date.
     *
     * The comparison is between LOCAL midnights: the core stamped
     * `day_start_ms` with this device's own midnight, so comparing it against
     * today's gives whole days apart without any timezone arithmetic here.
     */
    private fun dayLabel(dayStartMs: Double, strings: VelaStrings, now: Long): String {
        val today = Calendar.getInstance().apply {
            timeInMillis = now
            set(Calendar.HOUR_OF_DAY, 0)
            set(Calendar.MINUTE, 0)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
        }.timeInMillis
        val days = ((today - dayStartMs.toLong()) / DAY_MS)
        return when (days) {
            0L -> strings.t(I18nKeys.Wallet.DAY_TODAY)
            1L -> strings.t(I18nKeys.Wallet.DAY_YESTERDAY)
            else -> Formats.current.date(dayStartMs.toLong())
        }
    }

    private fun activityRow(item: FeedItem, strings: VelaStrings, chainNames: Map<Int, String>, now: Long, hidden: Boolean): ActivityRowModel {
        val received = item.direction == FeedDirection.In
        val batch = item.batch
        // What the row is and where its record stands are the core's (spec
        // 082 RG1): `kind` and `status`, never a guess from a hash. Spec 093:
        // a row the core describes as a dApp's (a transaction or a signature)
        // is a dApp row.
        val dapp = item.dapp
        val dappRow = dapp != null || item.kind in DAPP_KINDS
        // The right column: the money as before; else a grant's allowance.
        val allowance = dapp?.allowance?.takeIf { item.value == null }
        // Privacy (`app::privacy`): the row's own figure masks exactly when the
        // core says it is money — never an unlimited allowance (a risk to see)
        // nor a row with no figure (dots there would claim one).
        val masked = hidden && item.figure_maskable
        return ActivityRowModel(
            id = item.id,
            kind = when {
                dappRow -> ActivityKind.Dapp
                item.kind == FeedTxKind.Receive || received -> ActivityKind.Received
                else -> ActivityKind.Sent
            },
            titlePlace = dapp?.let { dappTitleParts(it, strings) },
            title = when {
                dapp != null -> dappTitle(dapp, strings)
                // A dApp payload this build could not read: the plain word.
                dappRow -> strings.t(I18nKeys.Wallet.LABEL_DAPP_TX)
                received -> strings.t(I18nKeys.Wallet.LABEL_RECEIVED)
                else -> strings.t(I18nKeys.Wallet.LABEL_SENT)
            },
            subtitle = subtitle(item.subtitle, strings, chainNames, now),
            amount = when {
                masked -> MASK
                allowance != null -> allowanceFigure(allowance, strings)
                // 083 F1: the simulation's figure, not one the wallet can vouch for.
                dapp?.estimated == true && item.value != null -> "≈ " + signedAmount(item, received)
                else -> signedAmount(item, received)
            },
            unit = allowance?.symbol ?: item.symbol.ifBlank { batch?.symbol.orEmpty() },
            positive = received,
            masked = masked,
            badgeColor = badgeColour(item.chain_id),
            badgeLogoUrl = Marks.chainLogoUrl(item.chain_id),
            danger = allowance?.unlimited == true,
            // What came back masks whenever the balance is hidden, whatever
            // the row's own figure is (`privacy`).
            received = dapp?.received?.let { change ->
                if (hidden) masked(change.symbol) else listOf(changeFigure(change), change.symbol).filter { it.isNotBlank() }.joinToString(" ")
            },
        )
    }

    /**
     * Spec 093: a dApp row's title, in the core's words — the headline verb
     * (`componentsUi.signing.<intent_term>`, else the recorded text) "on" the
     * place the core named (`history.dappRowTitle`), or the verb alone.
     */
    fun dappTitle(dapp: FeedDapp, strings: VelaStrings): String {
        val verb = dappVerb(dapp, strings)
        return dapp.place?.takeIf { it.isNotBlank() }
            ?.let { place -> strings.t(I18nKeys.Wallet.DAPP_ROW_TITLE, mapOf("intent" to verb, "place" to place)) }
            ?: verb
    }

    private fun dappVerb(dapp: FeedDapp, strings: VelaStrings): String = dapp.intent_term
        ?.let { term -> (I18nKeys.Wallet.SIGNING_TERM_PREFIX + term).let { key -> strings.t(key).takeIf { it.isNotBlank() && it != key } } }
        ?: dapp.intent?.takeIf { it.isNotBlank() }
        ?: strings.t(I18nKeys.Wallet.LABEL_DAPP_TX)

    /**
     * [dappTitle] split around its place, as the locale orders them — `null`
     * when the title names no place, or the template does not name it once.
     */
    fun dappTitleParts(dapp: FeedDapp, strings: VelaStrings): TitlePlace? {
        val place = dapp.place?.takeIf { it.isNotBlank() } ?: return null
        val mark = "\uFFFC"
        val pieces = strings.t(I18nKeys.Wallet.DAPP_ROW_TITLE, mapOf("intent" to dappVerb(dapp, strings), "place" to mark))
            .split(mark)
        if (pieces.size != 2) return null
        return TitlePlace(
            lead = pieces[0].trim(),
            place = place,
            trail = pieces[1].trim(),
            gapBefore = pieces[0].lastOrNull()?.isWhitespace() == true,
            gapAfter = pieces[1].firstOrNull()?.isWhitespace() == true,
        )
    }

    /**
     * Spec 093: the row's second line, part by part as the core ordered them,
     * joined " · ". Status in the detail sheet's words (087 F04 "Unknown" too);
     * a person by name, else a short address; a site verbatim; a network by
     * its name on this device; a day as the date headers say it.
     */
    fun subtitle(lines: List<FeedLine>, strings: VelaStrings, chainNames: Map<Int, String>, now: Long = System.currentTimeMillis()): String =
        lines.mapNotNull { line ->
            when (line) {
                is FeedLine.Status -> when (line.status) {
                    FeedTxStatus.Confirmed -> null
                    FeedTxStatus.Pending -> strings.t(I18nKeys.Wallet.ROW_PENDING)
                    FeedTxStatus.Failed -> strings.t(I18nKeys.Wallet.ROW_FAILED)
                    FeedTxStatus.Unknown -> strings.t(I18nKeys.Wallet.ROW_UNKNOWN)
                }
                is FeedLine.To -> strings.t(I18nKeys.Wallet.TO_NAME, mapOf("name" to (line.name ?: shortenAddress(line.address))))
                is FeedLine.From -> strings.t(I18nKeys.Wallet.FROM_NAME, mapOf("name" to (line.name ?: shortenAddress(line.address))))
                is FeedLine.Site -> line.site
                is FeedLine.Network -> chainNames[line.chain_id] ?: line.chain_id.toString()
                is FeedLine.Day -> dayLabel(line.day_start_ms, strings, now)
            }?.takeIf { it.isNotBlank() }
        }.joinToString(" · ")

    /**
     * Spec 093: an allowance where a figure would be — "Unlimited" (drawn in
     * the danger tone by the caller), else the cap. The symbol is the unit.
     */
    fun allowanceFigure(allowance: FeedAllowance, strings: VelaStrings): String = when {
        allowance.unlimited -> strings.t(I18nKeys.Wallet.UNLIMITED)
        else -> allowance.value?.let(::amountText).orEmpty()
    }

    /**
     * One balance change as a figure (083 F1): "≈" on everything the wallet
     * cannot vouch for to the unit; a token the sheet could not verify keeps
     * its direction and never a number.
     */
    fun changeFigure(change: FeedDappChange): String {
        val sign = if (change.direction == FeedDirection.In) "+" else "−"
        val value = change.value?.takeIf { change.verified } ?: return sign
        return (if (change.exact) "" else "≈ ") + sign + amountText(value)
    }

    /** `0x1234…abcd`. */
    fun shortenAddress(address: String): String =
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
        val trimmed = amountText(raw)
        return if (received) "+$trimmed" else "−$trimmed"
    }

    /** A decimal amount on the row's ladder: six places at most, cut, never rounded, in the person's number format. */
    private fun amountText(raw: String): String {
        val parsed = raw.toBigDecimalOrNull() ?: return raw
        return Formats.current.plain(parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString())
    }

    /** A dApp's record kinds (spec 093): its transactions and its signatures. */
    private val DAPP_KINDS = setOf(FeedTxKind.DappTx, FeedTxKind.SignMessage, FeedTxKind.SignTypedData)

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
        return view.tokens.map { token -> assetRow(token, chainNames, money, view.hidden) }
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
            // The label names the currency in EVERY state — it used to keep
            // the drawn board's "USD" while the figure loaded, then change.
            .copy(currency = money.label)
        // Spec 048 (device-found): hidden used to return the FIXTURE with a hidden
        // state — "$1,383 · USD" under the eye. Hidden is the live label with
        // the figures masked.
        // The hero's mask is the wider one (`privacy::BALANCE_MASK`), as the
        // gallery's hidden state always drew it.
        if (view.hidden) return live.copy(state = BalanceStateKind.Hidden, integer = BALANCE_MASK, decimals = null)
        // The display currency is not the person's yet (the core's rule on
        // `CurrencyView.committed`): no figure. The total waits as a total
        // still being read waits, and appears once, in the right money — it
        // read "$1,234" for a few seconds and then jumped to "¥8,876".
        if (!money.settled && (live.state == BalanceStateKind.Normal || live.state == BalanceStateKind.ZeroLive)) {
            return live.copy(state = BalanceStateKind.Loading, integer = null, decimals = null, liveText = null)
        }
        return live
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
        // with nothing cached: rendering a figure here was spec 038 finding 15,
        // a settled-looking "$0.00" over an unreadable chain. The core's figure
        // is null in this state now (PR 2 polish; it was 0.0), and the flag
        // still decides first: a skeleton and a reason, the same reason the
        // web and desktop heroes give.
        if (view.unreachable) {
            return fallback.copy(
                state = BalanceStateKind.Loading,
                integer = null,
                decimals = null,
                status = BalanceStatusModel(
                    kind = BalanceStatusKind.Warning,
                    // A read that failed inside Vela (PR 2 note 11) is said as
                    // that — never "the request never arrived".
                    text = strings.t(view.internal_key ?: I18nKeys.Wallet.BALANCE_UNREACHABLE),
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
                // A refresh the person asked for is the control's to show
                // (issue 462), never a line pushed in above it.
                status = null,
            )
        }

        // The exact binary value rounded HALF UP — `Formats.fixed2`'s rule and
        // the web's `toFixed(2)`, so the hero and its rows agree with each
        // other and with the web. It used to CUT: `BigDecimal(12.34)` is
        // 12.3399…, and cutting that printed $12.33.
        val rounded = BigDecimal(money.convert(total)).setScale(2, RoundingMode.HALF_UP)
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
     * `liveBalance`): the networks the wallet cannot reach (spec 092 — every
     * one, held or not; a rate limit heals on its own and is never listed),
     * said without "RPC" — the line opens their list. Then the cached figure
     * standing in for the live one, then the core's notice.
     *
     * A refresh the person asked for (`view.refreshing`) is NOT a reason for
     * this line (issue 462): the control under it turns and says "Updating…"
     * instead. When it was one, every tap inserted "Some balances are still
     * updating." above the control and pushed it out from under the finger.
     */
    internal fun balanceStatus(view: BalanceView, strings: VelaStrings, chainNames: Map<Int, String>): BalanceStatusModel? {
        val onCache = view.display_total_usd == null && view.cached_total_usd != null
        val unreachable = unreachableLine(view, strings, chainNames)
        return when {
            // PR 2 note 11 (issue 483): a read that never left the app is
            // Vela's own fault — the core's sentence for it, where the
            // unreachable line goes and in place of any "Can't reach …".
            view.internal_key != null -> BalanceStatusModel(BalanceStatusKind.Warning, strings.t(view.internal_key))
            unreachable != null -> BalanceStatusModel(BalanceStatusKind.Warning, unreachable)
            onCache || view.notice == BalanceNotice.StillUpdating ->
                BalanceStatusModel(BalanceStatusKind.Refreshing, strings.t(I18nKeys.Wallet.BALANCE_STALE))
            view.notice == BalanceNotice.Unpriced ->
                BalanceStatusModel(BalanceStatusKind.Warning, strings.t(I18nKeys.Wallet.BALANCE_UNPRICED))
            else -> null
        }
    }

    /**
     * The hero's refresh control (issue 462): when the figure was last read —
     * "Updated 2m", the core's `last_refreshed_at_ms` in the core's relative
     * words — and whether a refresh the person asked for is out. [now] is the
     * caller's clock; the screen re-reads it at least every 30 s, so the label
     * ages while it is on screen.
     */
    fun refresh(view: BalanceView, strings: VelaStrings, now: Long = System.currentTimeMillis()): BalanceRefreshModel =
        BalanceRefreshModel(
            updated = view.last_refreshed_at_ms?.let { at ->
                strings.t(I18nKeys.Wallet.LAST_UPDATED, mapOf("ago" to RelativeTime.ago(at, now, strings)))
            },
            updating = strings.t(I18nKeys.Wallet.UPDATING),
            idleLabel = strings.t(I18nKeys.Wallet.REFRESH_BALANCE),
            refreshing = view.refreshing,
        )

    /**
     * The line over the networks the wallet cannot reach (spec 092) — the
     * hero's status line and the title of the list it opens. The core chooses
     * the sentence (`unreachable_key`: one network named, several counted);
     * this only fills it. `null` when every network answered.
     */
    fun unreachableLine(view: BalanceView, strings: VelaStrings, chainNames: Map<Int, String>): String? {
        val first = view.unreachable_networks.firstOrNull() ?: return null
        // Whichever sentence the core chose, filled the one way: `{{name}}` is
        // the first network's name ("Can't reach Polygon", "Can't load
        // Tempo's token list" — its RPC is fine, the list that names what to
        // read there is what could not be loaded), `{{n}}` how many there
        // are. It switched on the two keys it knew, so a third said nothing.
        val key = view.unreachable_key ?: return null
        return strings.t(
            key,
            mapOf(
                "name" to (chainNames[first.chain_id] ?: first.chain_id.toString()),
                "n" to view.unreachable_networks.size.toString(),
            ),
        )
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
        /** Balance privacy: the amount AND its worth draw the mask (`privacy`). */
        hidden: Boolean,
    ): AssetRowModel {
        val mark = mark(token.chain_id, token.symbol, token.token_address)
        return AssetRowModel(
            id = holdingId(token.chain_id, token.token_address),
            ticker = token.symbol,
            // The chain, falling back to the token's own name only when this
            // device has no row for the chain — never a blank line.
            chain = chainNames[token.chain_id] ?: token.name,
            badgeColor = mark.badgeColor,
            logoUrls = mark.logoUrls,
            badgeLogoUrl = mark.badgeLogoUrl,
            badgeHidden = mark.badgeHidden,
            // The ONE token-amount rule (spec 078): Send's picker, token card,
            // confirm and receipt call the same function on the same holding.
            balance = if (hidden) MASK else "${tokenAmountText(token.balance)} ${token.symbol}",
            fiat = when {
                hidden -> AssetFiatModel.Masked
                // A holding's worth is a figure in the display currency: it
                // waits with the total while that is not the person's yet.
                !money.settled -> AssetFiatModel.Loading
                else -> token.price_usd?.let { price ->
                    val value = money.convert(amountAsDouble(token.balance) * price)
                    AssetFiatModel.Value(money.symbol + Formats.current.fixed2(value))
                } ?: AssetFiatModel.NoPrice("—")
            },
            masked = hidden,
        )
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
     *
     * While the balance is hidden (`switcher.hidden`) the core withholds every
     * figure, and each row and the total draw [MASK] — never an overlay of the
     * total the hero is hiding.
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
        val hidden = switcher.hidden
        val count = strings.t(I18nKeys.SettingsUi.ACCOUNTS_COUNT, mapOf("count" to accounts.size.toString()))
        return AccountsSheetModel(
            title = strings.t(I18nKeys.SettingsUi.ACCOUNTS_TITLE),
            summary = when {
                hidden -> count + strings.t(I18nKeys.SettingsUi.ACCOUNTS_TOTAL, mapOf("amount" to MASK))
                // No total in a currency that is not the person's yet.
                known && money.settled -> count + strings.t(I18nKeys.SettingsUi.ACCOUNTS_TOTAL, mapOf("amount" to money.fiat(total)))
                else -> count.trimEnd(' ', '·')
            },
            rows = accounts.mapIndexed { i, (name, address) ->
                val short = ExploreLive.shortAddress(address)
                val usd = switcher.balances.firstOrNull { it.address.equals(address, ignoreCase = true) }?.usd
                AccountsSheetRowModel(
                    name = name.ifBlank { short },
                    addressDisplay = short,
                    addressFull = address,
                    amount = if (hidden) MASK else usd?.takeIf { money.settled }?.let { money.fiat(it) } ?: "",
                    selected = i == activeIndex,
                )
            },
            primary = strings.t(I18nKeys.SettingsUi.ACCOUNT_CREATE),
            secondary = strings.t(I18nKeys.SettingsUi.ACCOUNT_SIGN_IN),
            // Taking ONE wallet off this device (2026-09-23). The words are
            // resolved here because the sheet resolves none of its own.
            remove = strings.t(I18nKeys.SettingsUi.ACCOUNT_REMOVE),
            removeBody = strings.t(I18nKeys.SettingsUi.ACCOUNT_REMOVE_BODY),
            removeCancel = strings.t(I18nKeys.Settings.SIGN_OUT_CANCEL),
        )
    }

    /**
     * Every COIN's mark on the phone: the core's answer (`Marks.tokenMark`:
     * the coin's home-chain logo for a native coin, the asset entry for a
     * token, no badge where it would repeat the coin) in this shell's colours.
     * [tokenAddress] is `null` for the chain's own coin.
     */
    fun mark(chainId: Int, symbol: String, tokenAddress: String?, logoUrls: List<String> = emptyList()): TokenMarkModel =
        markModel(symbol, chainId, Marks.tokenMark(chainId, symbol, tokenAddress, logoUrls))

    /**
     * Every NETWORK's mark (the kind rule): a network row or fact, a notice
     * that locks a chain, the QR centre. The chain's own logo over its coin's
     * letters, never a badge, and never the coin's home chain — ETH sent on
     * Base is Base's logo here.
     */
    fun chainMark(chainId: Int, nativeSymbol: String): TokenMarkModel =
        markModel(nativeSymbol, chainId, Marks.chainMark(chainId, nativeSymbol))

    /**
     * The core's mark in this shell's model. The badge is drawn exactly when
     * the core names its chain; its dot is that chain's colour.
     * [ticker] stays what the caller named (the QR centre letters it in
     * full); the circle draws its first three letters, as the core's glyph.
     */
    private fun markModel(ticker: String, chainId: Int, mark: uniffi.vela_core_uniffi.MarkView): TokenMarkModel = TokenMarkModel(
        ticker = ticker,
        badgeColor = badgeColour(mark.badgeChainId?.toInt() ?: chainId),
        logoUrls = mark.logoUrls,
        badgeLogoUrl = mark.badgeLogoUrl,
        badgeHidden = mark.badgeChainId == null,
    )

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
        /**
         * The display currency is the person's (`CurrencyView.committed`).
         * `false`: the core is still on its USD placeholder — and a surface
         * that draws a figure in it draws the WRONG currency for a few
         * seconds, then jumps. The home's total and its holdings' worth wait
         * instead (their loading state), and appear once, in the right money.
         */
        val settled: Boolean = true,
        /**
         * What a label that names its currency apart from the figure says:
         * the committed code; while waiting, the stored choice on its way
         * (`CurrencyView.pending`); nothing at all before either is known.
         */
        val label: String = code,
    ) {
        fun convert(usd: Double): Double = rate?.let { usd * it } ?: usd

        /** A fiat figure in this money, drawn with the person's number format (spec 047 D2). */
        fun fiat(usd: Double): String = symbol + Formats.current.fixed2(convert(usd))

        companion object {
            /** Dollars, unconverted — the default before a currency is known. */
            fun dollars(): Money = Money("USD", "$", null)

            fun of(view: CurrencyView): Money {
                // No settled choice yet: the placeholder's dollars, marked as
                // not the person's — figures drawn in the display currency wait.
                if (!view.committed) return Money("USD", "$", null, settled = false, label = view.pending.orEmpty())
                val rate = view.rate?.takeIf { it.isFinite() && it > 0.0 }
                // A settled choice nothing could price means dollars — and saying so.
                if (rate == null) return Money("USD", "$", null)
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
