//
//  WalletLive.swift
//  VelaWallet
//
//  The person's own money, in the display models the drawn home screen already
//  consumes.
//
//  A **sibling** of `WalletFixtures`, and — like `SettingsLive` — a partial
//  one: `WalletHomeModel` carries the whole screen, and only the balance hero
//  and the asset rows have a machine behind them in this cut. The activity
//  section is spec 051 phase 3; the pill and the sheet wait on their own
//  machines. A field this file does not touch is visibly still a fixture.
//
//  ## `rate: null` is not `1`, again, and now on the figure that matters
//
//  `display_total_usd == nil` means **no total can be stated**. It is not zero.
//  Rendering it as `$0.00` would tell somebody their wallet is empty when the
//  truth is that nobody could price it.
//
//  ## A partial total is a floor, and has to say so
//
//  When some chains answered and some did not, the core sets `balance_partial`.
//  The figure is then "at least this much", and the drawn warning status is what
//  says so. Silently showing the sum as if it were complete is the same defect
//  as showing zero, one step subtler.
//

import SwiftUI

enum WalletLive {

    /// Swap the balance hero and the asset rows onto the drawn home.
    static func apply(
        _ view: BalanceViewWire,
        on model: WalletHomeModel,
        loc: Loc
    ) -> WalletHomeModel {
        var copy = model
        copy.balance = balance(view, fallback: model.balance)
        copy.assetRows = assetRows(view)
        return copy
    }

    // MARK: - The hero

    static func balance(_ view: BalanceViewWire, fallback: BalanceModel) -> BalanceModel {
        var model = BalanceModel(
            label: fallback.label,
            currency: fallback.currency,
            state: state(view),
            integer: nil,
            decimals: nil,
            liveText: nil,
            status: status(view, fallback: fallback),
            a11yHide: fallback.a11yHide,
            a11yShow: fallback.a11yShow
        )

        // Hidden and loading both draw without a figure, so there is nothing to
        // split — and building one anyway is how a concealed balance leaks into
        // an accessibility label.
        guard model.state == .normal || model.state == .zeroLive,
              let total = view.displayTotalUsd ?? view.cachedTotalUsd
        else { return model }

        let (integer, decimals) = split(total)
        model.integer = integer
        model.decimals = decimals
        return model
    }

    /// `nil` total is **not** zero — it is "no total can be stated", and the
    /// drawn loading treatment is what says that. `zeroLive` is for a wallet
    /// that genuinely holds nothing, which the core distinguishes.
    private static func state(_ view: BalanceViewWire) -> BalanceStateKind {
        if view.hidden { return .hidden }
        if view.balanceUnknown { return .loading }
        guard let total = view.displayTotalUsd ?? view.cachedTotalUsd else { return .loading }
        return total == 0 ? .zeroLive : .normal
    }

    /// The line under the figure. A partial total says so; a refresh over a
    /// cached figure says that instead.
    private static func status(
        _ view: BalanceViewWire,
        fallback: BalanceModel
    ) -> BalanceStatusModel? {
        if view.balancePartial || !view.failedChainIds.isEmpty {
            return BalanceStatusModel(kind: .warning,
                                      text: fallback.status?.text ?? "")
        }
        if view.refreshing || view.notice == .stillUpdating {
            return BalanceStatusModel(kind: .refreshing,
                                      text: fallback.status?.text ?? "")
        }
        return nil
    }

    /// `1234.56` → `("1,234", "56")`. The drawing splits the figure so the
    /// decimals can be subordinated, which is the design language's rule about
    /// money: the magnitude reads first.
    static func split(_ total: Double) -> (String, String) {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 2
        formatter.maximumFractionDigits = 2
        let text = formatter.string(from: NSNumber(value: total))
            ?? String(format: "%.2f", total)
        guard let separator = text.lastIndex(of: ".") else { return (text, "00") }
        return (String(text[..<separator]), String(text[text.index(after: separator)...]))
    }

    // MARK: - The assets

    /// One row per holding, priced or honestly unpriced.
    ///
    /// **`tokens` only.** `unpricedTokens` is not the complement of `tokens` —
    /// the core's own doc calls it "the detail sheet's 'couldn't be priced'
    /// list", a **subset** built for a different surface. Concatenating the two
    /// renders every unpriceable holding twice, which is what the first live
    /// run of this screen showed: one address, one chain, two identical xDAI
    /// rows.
    ///
    /// Invisible until the balance was real — with a fixture there was nothing
    /// to duplicate.
    static func assetRows(_ view: BalanceViewWire) -> [AssetRowModel] {
        view.tokens.map { token in
            AssetRowModel(
                ticker: token.symbol,
                chain: ChainCatalog.meta(token.chainId)?.displayName ?? "",
                badgeColor: chainColor(token.chainId),
                balance: token.balance,
                fiat: fiat(token, hidden: view.hidden),
                masked: view.hidden
            )
        }
    }

    private static func fiat(_ token: BalanceTokenWire, hidden: Bool) -> AssetFiatModel {
        if hidden { return .masked }
        guard let price = token.priceUsd, let amount = Double(token.balance) else {
            // Held, unpriceable. The drawn `noPrice` treatment says exactly
            // that rather than showing a `$0.00` nobody should read as a value.
            return .noPrice("")
        }
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = "USD"
        return .value(formatter.string(from: NSNumber(value: amount * price)) ?? "")
    }

    /// The same brand colours the settings list uses, and the same neutral for a
    /// chain nobody drew.
    private static func chainColor(_ chainId: Int) -> Color {
        SettingsLive.mark(chainId: chainId, name: "").color
    }
}
