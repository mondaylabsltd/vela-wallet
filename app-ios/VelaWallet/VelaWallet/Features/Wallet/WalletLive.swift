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
//  ## The figure is converted, or it is labelled USD — never relabelled
//
//  `display_total_usd` is USD by construction; `display_currency` owns the
//  rate. When it has one the hero converts and wears the person's code; when it
//  does not, the hero shows the **USD figure under USD**. Relabelling the same
//  digits with a ¥ is the lie the whole `rate: null` rule exists to prevent —
//  it tells somebody 1,234.56 dollars is 1,234.56 yuan.
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

    /// Which currency the money on this screen is stated in, and what to
    /// multiply a USD figure by to get there.
    ///
    /// Built from the `display_currency` view, and it applies the same rule
    /// `SettingsLive.currencyRowValue` does — one rule, two surfaces.
    struct Display {
        let code: String
        let rate: Double

        static let usd = Display(code: "USD", rate: 1)

        /// The badge the hero's figure wears. Decoration, not identity — the
        /// code itself is already stated in the line above the figure, so a
        /// currency the catalog has never heard of shows the number bare rather
        /// than borrowing somebody else's `$`.
        var glyph: String { CurrencyCatalog.entry(code)?.glyph ?? "" }

        /// A currency without a rate, or one the person has not committed to,
        /// degrades to USD. `rate: nil` is **not** 1: the difference is
        /// whether the digits get relabelled.
        static func from(_ view: CurrencyViewWire?) -> Display {
            guard let view, view.committed, let rate = view.rate, rate > 0, rate.isFinite
            else { return .usd }
            return Display(code: view.code, rate: rate)
        }
    }

    /// Swap the balance hero and the asset rows onto the drawn home.
    static func apply(
        _ view: BalanceViewWire,
        currency: CurrencyViewWire? = nil,
        on model: WalletHomeModel,
        loc: Loc
    ) -> WalletHomeModel {
        var copy = model
        let display = Display.from(currency)
        copy.balance = balance(view, display: display, fallback: model.balance)
        copy.assetRows = assetRows(view, display: display)
        return copy
    }

    // MARK: - The hero

    static func balance(
        _ view: BalanceViewWire,
        display: Display = .usd,
        fallback: BalanceModel
    ) -> BalanceModel {
        var model = BalanceModel(
            label: fallback.label,
            currency: display.code,
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

        let (integer, decimals) = split(total * display.rate)
        model.integer = display.glyph + integer
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
    static func assetRows(_ view: BalanceViewWire, display: Display = .usd) -> [AssetRowModel] {
        view.tokens.map { token in
            AssetRowModel(
                ticker: token.symbol,
                chain: ChainCatalog.meta(token.chainId)?.displayName ?? "",
                badgeColor: chainColor(token.chainId),
                balance: token.balance,
                fiat: fiat(token, hidden: view.hidden, display: display),
                masked: view.hidden
            )
        }
    }

    private static func fiat(
        _ token: BalanceTokenWire,
        hidden: Bool,
        display: Display
    ) -> AssetFiatModel {
        if hidden { return .masked }
        guard let price = token.priceUsd, let amount = Double(token.balance) else {
            // Held, unpriceable. The drawn `noPrice` treatment says exactly
            // that rather than showing a `$0.00` nobody should read as a value.
            return .noPrice("")
        }
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = display.code
        let value = amount * price * display.rate
        return .value(formatter.string(from: NSNumber(value: value)) ?? "")
    }

    /// The same brand colours the settings list uses, and the same neutral for a
    /// chain nobody drew.
    private static func chainColor(_ chainId: Int) -> Color {
        SettingsLive.mark(chainId: chainId, name: "").color
    }
}
