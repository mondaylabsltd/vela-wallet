package app.getvela.wallet.feature.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import java.math.BigDecimal
import java.math.RoundingMode

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
        strings: VelaStrings,
        chainNames: Map<Int, String>,
    ): WalletHomeModel {
        val rows = view.tokens.map { token -> assetRow(token, chainNames) }
        return fallback.copy(
            balance = balance(fallback.balance, view, strings),
            assetsSection = fallback.assetsSection.copy(
                mode = when {
                    rows.isNotEmpty() -> SectionMode.Rows
                    view.holdings_loading -> SectionMode.Loading
                    else -> SectionMode.Empty
                },
            ),
            assetRows = rows,
            // live in 041 phase 5 — the feed needs `activity_feed`, so the
            // section keeps its fixture shape until then rather than claiming
            // this wallet has no history.
        )
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
    ): BalanceModel {
        if (view.hidden) return fallback.copy(state = BalanceStateKind.Hidden)

        val total = view.display_total_usd

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

        val rounded = BigDecimal(total).setScale(2, RoundingMode.DOWN)
        val whole = rounded.toBigInteger()
        val cents = rounded.subtract(BigDecimal(whole)).movePointRight(2).abs().toBigInteger()
        return fallback.copy(
            state = if (rounded.signum() == 0) BalanceStateKind.ZeroLive else BalanceStateKind.Normal,
            integer = "$" + groupThousands(whole.toString()),
            decimals = cents.toString().padStart(2, '0'),
            status = fallback.status?.takeIf { view.refreshing || view.balance_partial },
        )
    }

    private fun assetRow(token: BalanceToken, chainNames: Map<Int, String>): AssetRowModel = AssetRowModel(
        ticker = token.symbol,
        // The chain, falling back to the token's own name only when this
        // device has no row for the chain — never a blank line.
        chain = chainNames[token.chain_id] ?: token.name,
        badgeColor = badgeColour(token.chain_id),
        balance = "${trimAmount(token.balance)} ${token.symbol}",
        fiat = token.price_usd?.let { price ->
            val value = amountAsDouble(token.balance) * price
            AssetFiatModel.Value("$" + BigDecimal(value).setScale(2, RoundingMode.DOWN).toPlainString())
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
    private fun trimAmount(balance: String): String {
        val parsed = balance.toBigDecimalOrNull() ?: return balance
        return parsed.setScale(6, RoundingMode.DOWN).stripTrailingZeros().toPlainString()
    }

    private fun amountAsDouble(balance: String): Double = balance.toDoubleOrNull() ?: 0.0

    private fun groupThousands(digits: String): String =
        digits.reversed().chunked(3).joinToString(",").reversed()

    /** A stable colour per chain, so a token keeps its badge between launches. */
    private fun badgeColour(chainId: Int): Color = BADGES[chainId.mod(BADGES.size)]

    private val BADGES = listOf(
        Color(0xFF6C7BFF), Color(0xFF2E9E7E), Color(0xFFE0A03A), Color(0xFF8C8C8C),
        Color(0xFFCF5C7A), Color(0xFF4A9BD1), Color(0xFF9B6CD1), Color(0xFF3FA37A),
    )
}
