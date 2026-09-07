//
//  FlowsLive.swift
//  VelaWallet
//
//  The flow screens, wearing real state.
//
//  A sibling of `WalletFlowFixtures`, and — like `WalletLive` and `SettingsLive`
//  — a **partial** one. Spec 021 drew nine flows; two have machines behind them
//  in this cut:
//
//  - **T1 资产** — the same holdings the home lists, from `balance_dashboard`.
//  - **T3 添加代币** — `manage_tokens`: what was typed, what the chains
//    answered, and what is already saved.
//
//  Everything else this file does not touch stays the drawing, visibly.
//
//  ## The drawn add-token sheet and the machine do not have the same shape
//
//  The web panel searches every network and lists a card per chain; the drawing
//  has one network row, one result card and one CTA. So the card shown is the
//  **first** the core found (registry order), and the network row names the
//  chain it was found on rather than being a chooser. That is a recorded
//  deviation, not a redesign: the core's other found chains are still in its
//  view, and a drawing that lists them is what would let the shell show them.
//

import SwiftUI

enum FlowsLive {

    // MARK: - T1, the assets list

    /// The person's own holdings, in the drawn list.
    static func assets(
        _ balance: BalanceViewWire,
        currency: CurrencyViewWire?,
        on model: AssetsModel,
        loc: Loc
    ) -> AssetsModel {
        let rows = WalletLive.assetRows(balance, display: WalletLive.Display.from(currency))
        return AssetsModel(
            header: model.header,
            searchPlaceholder: model.searchPlaceholder,
            rows: rows,
            addByAddress: model.addByAddress,
            // The drawn guided-empty body, for a wallet that genuinely holds
            // nothing — and only once the core has ruled, never while the
            // fetch is still out (FR-008).
            empty: rows.isEmpty && !balance.balanceUnknown ? model.empty : nil
        )
    }

    // MARK: - T3, the add-token sheet

    /// The sheet, driven by `manage_tokens`.
    static func addToken(
        _ view: MtokViewWire,
        on model: AddTokenModel,
        loc: Loc
    ) -> AddTokenModel {
        let found = view.found.first
        var live = model
        live.network = found.map { card in
            AddTokenNetworkModel(
                mark: TokenMarkModel(ticker: card.symbol,
                                     badgeColor: SettingsLive.mark(chainId: card.chainId,
                                                                  name: card.networkName).color),
                name: card.networkName,
                pickLabel: model.network?.pickLabel ?? ""
            )
        } ?? model.network
        live.fieldValue = view.inputAddress
        // An error only once there is something to be wrong about: an empty
        // field is not an invalid address, it is an empty field.
        live.fieldError = view.inputAddress.isEmpty || view.addressValid
            ? nil : loc.t("addToken.invalidAddress")
        live.result = result(view, loc: loc)
        live.ctaDisabled = found == nil || found?.added == true || view.saving
        return live
    }

    private static func result(_ view: MtokViewWire, loc: Loc) -> AddTokenResult {
        if view.detecting { return .searching(loc.t("addToken.searchingNetworks")) }
        if view.notFound {
            return .notFound("\(loc.t("addToken.notFoundTitle")) — \(loc.t("addToken.notFoundMessage"))")
        }
        guard let card = view.found.first else { return .none }
        return .token(
            mark: TokenMarkModel(
                ticker: card.symbol,
                badgeColor: SettingsLive.mark(chainId: card.chainId,
                                              name: card.networkName).color
            ),
            name: card.name,
            detail: "\(card.symbol) · \(loc.t("tokenDetail.labelDecimals")) \(card.decimals)"
                + " · \(card.networkName)",
            // The core recomputes "added" against the CURRENT input, so this
            // chip cannot linger from an address somebody has since edited.
            chip: card.added
                ? StatusChipModel(text: loc.t("addToken.tokenAdded"), tone: .success)
                : nil
        )
    }

    /// The save error the core raises when the write itself failed.
    ///
    /// Drawn nowhere in T3 — the mock has no alert — so it rides in the result
    /// card's place rather than being swallowed: a person who taps 添加到钱包
    /// and sees nothing change has been told nothing.
    static func saveErrorText(_ view: MtokViewWire, loc: Loc) -> String? {
        view.saveError
            ? "\(loc.t("addToken.errorTitle")) — \(loc.t("addToken.errorSaveToken"))"
            : nil
    }
}
