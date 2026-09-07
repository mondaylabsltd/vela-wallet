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

    // MARK: - R1 / R2, the receive screens

    /// The network list, with the person's OWN address on every row.
    ///
    /// This is the screen where a fixture is not merely embarrassing: money
    /// sent to the drawn address is money gone. Until this landed, 收款 showed
    /// `WalletFixtures.identity` — somebody else's address entirely — beside a
    /// code drawn from a demo pattern.
    static func receiveList(
        _ address: String,
        on model: ReceiveListModel,
        loc: Loc
    ) -> ReceiveListModel {
        guard !address.isEmpty else { return model }
        let networks = ChainCatalog.chains
        var live = model
        live.subtitle = loc.t("receive.networksLine", vars: ["count": String(networks.count)])
        live.rows = networks.map { chain in
            NetworkRowModel(
                name: chain.displayName,
                code: chain.nativeSymbol,
                badgeColor: SettingsLive.mark(chainId: chain.chainId, name: chain.displayName).color,
                // One address, every network — which is what the subtitle above
                // promises and what a 4337 Safe at a deterministic address
                // actually delivers.
                addressDisplay: AddressText.short(address),
                copyLabel: model.rows.first?.copyLabel ?? "",
                qrLabel: model.rows.first?.qrLabel ?? ""
            )
        }
        return live
    }

    /// The code, and the account card above it.
    ///
    /// The QR encodes the **bare address**. That is the whole answer in address
    /// mode; the amount-carrying EIP-681 request is `payment_request`'s, and
    /// its mode toggle, amount field and acknowledge gate are drawn nowhere on
    /// this client yet — recorded in results.md rather than invented here.
    static func receiveQr(
        _ address: String,
        name: String,
        chain: ChainMeta?,
        on model: ReceiveQrModel,
        loc: Loc
    ) -> ReceiveQrModel {
        guard !address.isEmpty else { return model }
        var live = model
        // The chain the person actually tapped. Without it the sheet keeps the
        // fixture's first network and tells somebody who picked Gnosis to
        // receive Ethereum assets — the address is the same on both, but the
        // sentence would be a lie and the mark would back it up.
        if let chain {
            live.title = loc.t("receive.qrTitleNetwork", vars: ["network": chain.displayName])
            live.centre = TokenMarkModel(
                ticker: chain.nativeSymbol,
                badgeColor: SettingsLive.mark(chainId: chain.chainId,
                                              name: chain.displayName).color
            )
        }
        live.account = AddressCardModel(
            name: name.isEmpty ? model.account.name : name,
            identiconSeed: address,
            lines: AddressText.lines(address),
            copyLabel: model.account.copyLabel
        )
        // `nil` when the address cannot be encoded — the card then draws the
        // demo pattern, which is why `QrCode` never falls back to it silently:
        // the caller decides, and here an unencodable address is a bug worth
        // seeing rather than a picture worth showing.
        live.modules = QrCode.modules(address)
        return live
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
