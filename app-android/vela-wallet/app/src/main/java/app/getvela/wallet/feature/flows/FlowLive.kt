package app.getvela.wallet.feature.flows

import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedDirection
import app.getvela.wallet.feature.wallet.core.FeedRow
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.PaymentRequestView

/**
 * The live flow builders: the drawn flow screens, showing this device's facts.
 *
 * The sibling of [FlowFixtures], the way `WalletLive` is of `WalletFixtures`.
 *
 * **The receive screen is the one place in this app where a stale value is
 * unrecoverable.** Every other fixture that leaks shows somebody the wrong
 * information; a fixture ADDRESS on a receive QR sends their money to a
 * stranger, permanently. So the address is replaced unconditionally here — not
 * "if we have one", not "unless it is blank" — and an empty session renders no
 * address at all rather than the one that was drawn.
 */
object FlowLive {

    /**
     * The QR sheet, for this person.
     *
     * [qrValue] is the core's: in address mode it is the bare recipient, and in
     * request mode the built `ethereum:` URI. The shell does not assemble it,
     * because what a payment request SAYS is the thing a scanner acts on.
     */
    fun receiveQr(
        fallback: ReceiveQrModel,
        address: String,
        name: String,
        request: PaymentRequestView,
    ): ReceiveQrModel = fallback.copy(
        account = fallback.account.copy(
            name = name,
            // The identicon is drawn FROM the address, so a stale seed is a
            // stale face beside a live address — the exact mismatch somebody
            // uses to check they are looking at their own wallet.
            identiconSeed = address,
            lines = addressLines(address),
        ),
    )

    /**
     * Whether this sheet may be shown at all.
     *
     * The core's gate: while it is loading, the QR stays covered so a first
     * visit never flashes a code before the warning about which networks this
     * address is safe on.
     */
    fun receiveGateOpen(request: PaymentRequestView): Boolean =
        !request.gate_loading && request.acknowledged

    /**
     * R1's rows: the networks this device actually has, each showing THIS
     * address.
     *
     * The fixture drew eight rows with a fixture address on every one. The
     * count is the person's own now, and so is the address — the same one, on
     * every network, which is the whole point the subtitle makes.
     */
    fun receiveNetworks(
        fallback: ReceiveListModel,
        view: NetView,
        address: String,
        badge: (Long) -> androidx.compose.ui.graphics.Color,
    ): ReceiveListModel {
        if (!view.loaded) return fallback
        val shown = shortAddress(address)
        // Copy on a row means the same address every time; the fixture's own
        // labels carry the wording.
        val template = fallback.rows.firstOrNull()
        return fallback.copy(
            subtitle = fallback.subtitle.replaceFirst(
                Regex("\\d+"),
                view.networks.size.toString(),
            ),
            rows = view.networks.map { row ->
                NetworkRowModel(
                    name = row.display_name,
                    code = row.native_symbol,
                    badgeColor = badge(row.chain_id),
                    addressDisplay = shown,
                    copyLabel = template?.copyLabel.orEmpty(),
                    qrLabel = template?.qrLabel.orEmpty(),
                )
            },
        )
    }

    /** `0x1234…abcd` — a row has no space for forty-two characters. */
    internal fun shortAddress(address: String): String =
        if (address.length <= 12) address else address.take(6) + "…" + address.takeLast(4)


    // -- the read screens behind "全部" ---------------------------------------

    /**
     * A1 — the whole history, not the first few.
     *
     * The home screen's Activity section shows a handful and offers "全部".
     * That link opened a fixture: a person tapped past their own payments into
     * somebody else's. Same rows, same grouping, same core ordering as the home
     * feed — this only removes the cut-off.
     */
    fun history(
        fallback: HistoryModel,
        feed: FeedView,
        strings: VelaStrings,
        now: Long = System.currentTimeMillis(),
    ): HistoryModel {
        val groups = WalletLive.activity(feed, strings, now)
        return fallback.copy(
            mode = if (groups.isEmpty()) HistoryMode.Empty else HistoryMode.Rows,
            groups = groups,
        )
    }

    /**
     * A2 — one transaction, the one that was tapped.
     *
     * [id] comes from the row. With no id, or an id the feed no longer holds,
     * the fixture is NOT shown: a detail screen about the wrong payment is
     * worse than one that admits it has nothing, because both look equally
     * authoritative and only one of them is wrong.
     */
    fun txDetail(
        fallback: TxDetailModel,
        feed: FeedView,
        id: String?,
        strings: VelaStrings,
    ): TxDetailModel? {
        val item = feed.rows
            .filterIsInstance<FeedRow.Item>()
            .map { it.item }
            .firstOrNull { it.id == id }
            ?: return null

        val received = item.direction == FeedDirection.In
        val amount = item.value?.toBigDecimalOrNull()
            ?.stripTrailingZeros()
            ?.toPlainString()
            .orEmpty()
        return fallback.copy(
            amount = "${if (received) "+" else "\u2212"}$amount ${item.symbol}".trim(),
            positive = received,
            // The fiat line is the core's own `usd_value`, which is 0 when
            // nothing could price it — and a confident "$0.00" on a detail
            // screen is the same lie the hero told once.
            fiat = if (item.usd_value > 0) {
                "$" + java.math.BigDecimal(item.usd_value)
                    .setScale(2, java.math.RoundingMode.DOWN)
                    .toPlainString()
            } else {
                ""
            },
        )
    }

    /**
     * T1 — every holding.
     *
     * The same rows the home screen shows, minus its cut-off. `chainNames`
     * comes from the network machine for the same reason it does there: a
     * row's second line is the chain, and `BalanceToken.name` is the token's.
     */
    fun assets(
        fallback: AssetsModel,
        view: BalanceView,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
    ): AssetsModel {
        val rows = WalletLive.assetRows(view, chainNames, currency)
        return fallback.copy(
            rows = rows,
            // The guided-empty body replaces the list; it must not sit under
            // one. A wallet that holds something is not an empty wallet.
            empty = if (rows.isEmpty()) fallback.empty else null,
        )
    }

    /**
     * T2 — one holding, the one that was tapped.
     *
     * Same rule as the transaction detail: no id, or an id nothing matches,
     * shows nothing rather than a fixture token. The transactions listed under
     * it are the ones on THIS token's chain and symbol.
     */
    fun tokenDetail(
        fallback: TokenDetailModel,
        view: BalanceView,
        feed: FeedView,
        id: String?,
        chainNames: Map<Int, String>,
        currency: CurrencyView,
        strings: VelaStrings,
        now: Long = System.currentTimeMillis(),
    ): TokenDetailModel? {
        val token = view.tokens
            .firstOrNull { WalletLive.holdingId(it.chain_id, it.token_address) == id }
            ?: return null

        val row = WalletLive.assetRows(
            BalanceView(tokens = listOf(token)),
            chainNames,
            currency,
        ).single()

        val theirs = FeedView(
            rows = feed.rows.filter { entry ->
                entry !is FeedRow.Item ||
                    (entry.item.chain_id == token.chain_id && entry.item.symbol == token.symbol)
            },
        )
        return fallback.copy(
            mark = TokenMarkModel(token.symbol, row.badgeColor),
            symbol = token.symbol,
            chain = row.chain,
            balance = row.balance,
            fiat = when (val fiat = row.fiat) {
                is AssetFiatModel.Value -> fiat.text
                // "—", not "$0.00": an unpriced holding has no fiat figure, and
                // a zero here would say it is worthless.
                is AssetFiatModel.NoPrice -> fiat.text
                else -> ""
            },
            rows = WalletLive.activity(theirs, strings, now).flatMap { it.rows },
        )
    }

    /** The address as the card draws it: two lines, split halfway. */
    internal fun addressLines(address: String): Pair<String, String> {
        if (address.isEmpty()) return "" to ""
        val half = (address.length + 1) / 2
        return address.take(half) to address.drop(half)
    }
}
