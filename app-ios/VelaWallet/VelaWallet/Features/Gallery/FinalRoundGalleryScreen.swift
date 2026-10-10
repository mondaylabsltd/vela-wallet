//
//  FinalRoundGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for PR 3's FINAL round (shell round 3), reached with
//  `VELA_PAGE=pr3c` and `VELA_STATE=<board>` — the states the final notes
//  touch. Every board is the PRODUCTION view over fixture data, built by the
//  production builders; where a core decides, the view is the real core's.
//
//  - `hero-checking` / `hero-live` / `hero-cant-reach` (F19) — a wallet that
//    held nothing last session, through its first round, as the real
//    `balance_dashboard` core writes it: "Checking…", then "Live · listening"
//    or "Can't reach 2 networks". One line, at one place.
//  - `hero-long` / `hero-fault` (F16) — a sentence longer than the line
//    (Tempo's token list; Vela's own fault): one line, cut with an ellipsis.
//    `list-token-list` is the list the first opens, titled by it in full;
//    `breakdown-fault` the breakdown the second opens, saying it at its top.
//  - `breakdown-status` (F21) — the breakdown's short status per network:
//    "Token list unavailable" beside "RPC unavailable".
//  - `breakdown-healthy` (F20) — every network answered: no "Networks still
//    updating" over an empty list.
//  - `wizard-…` (F4, F14, F22) — the RPC field and "Re-check with this RPC"
//    as the core rules them: `compatible` and `checked-unverified` (the
//    field, optional), `no-rpc` ("RPC URL", required), `unverified`, and the
//    refusals `no-p256` / `missing` (neither).
//  - `signing-out|send|swap|three|unverified|nothing|caution|danger` (F2) —
//    the signing sheet with the simulation's verdict in the place it keeps.
//  - `accounts-1` / `accounts-2` (F15) — "1 account · ", "2 accounts · ".
//  - `signout-one` / `signout-many` (F26) — the sign-out sheet, as tall as
//    what it holds.
//  - `batch-unknown` / `batch-known` (F8) — the batch importer before the
//    person's currency is known (no code said), and after.
//
//  Fixture data only: nothing here is read from a chain, scanned, signed or
//  sent — and, like every `VELA_PAGE` session, no camera is ever opened.
//

#if DEBUG

import SwiftUI
import VelaCore

struct FinalRoundGalleryScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var scheme
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "hero-live", "hero-cant-reach", "hero-fault":
            WalletScreen(model: home, loc: loc)
        case "hero-long":
            IntegrationGalleryScreen(loc: loc, state: "home-token-list")
        case "list-token-list":
            IntegrationGalleryScreen(loc: loc, state: state)
        case "breakdown-fault", "breakdown-status", "breakdown-healthy":
            SettingsScreen(model: breakdown, loc: loc)
        case let board where board.hasPrefix("wizard-"):
            IntegrationGalleryScreen(loc: loc, state: board)
        case let board where board.hasPrefix("signing-"):
            SigningSheet(model: signing(String(board.dropFirst("signing-".count))), onRefreshFee: {})
        case "accounts-1", "accounts-2":
            SettingsScreen(model: accounts(state == "accounts-1" ? 1 : 2), loc: loc)
        case "signout-one", "signout-many":
            signOutBoard(many: state == "signout-many")
        case "batch-unknown", "batch-known":
            FlowHost(model: batch(unknown: state == "batch-unknown"))
        default:
            WalletScreen(model: home, loc: loc)
        }
    }

    // MARK: - The hero's one line (F19, F16)

    /// The balance view behind this board, from the real core.
    private var balanceView: BalanceViewWire? {
        switch state {
        case "hero-live": BalanceCoreScene.zeroWallet()?.settled
        case "hero-cant-reach": BalanceCoreScene.zeroWallet(missing: [137, 10])?.settled
        case "hero-fault", "breakdown-fault": BalanceCoreScene.view(internalFault: true)
        // Tempo for want of its token list, BNB Chain for want of its nodes.
        case "breakdown-status":
            BalanceCoreScene.view(failedChain: 4_217, internalFault: false, tokenListFault: true, alsoDown: [56])
        case "breakdown-healthy": BalanceCoreScene.view(internalFault: false, healthy: true)
        default: BalanceCoreScene.zeroWallet()?.checking
        }
    }

    private var home: WalletHomeModel {
        // A wallet waiting for its first deposit keeps the drawn empty home
        // under the live hero; one with holdings, the drawn full one.
        let zero = state != "hero-fault"
        let drawn = WalletFixtures.buildMobileState(zero ? .h2 : .h1, loc: loc)
        guard let view = balanceView else { return drawn }
        var model = WalletLive.apply(view, on: drawn, loc: loc)
        model.activityGroups = drawn.activityGroups
        model.activitySection = drawn.activitySection
        return model
    }

    // MARK: - The breakdown (F16, F20, F21)

    private var breakdown: SettingsScreenModel {
        let base = SettingsFixtures.build(.sr3, loc: loc)
        guard let view = balanceView else { return base }
        return SettingsLive.withBalanceDetail(view, display: .usd, on: base, loc: loc)
    }

    // MARK: - The signing sheet's verdict place (F2)

    private static let address = "0x88cca0eedbf2c4426110bbfc998f048689266894"

    private func simulation(_ verdict: String) -> (judgments: [[String: Any]]?, state: SigningController.Simulation) {
        let native: [String: Any] = ["type": "native", "delta": "-1500000000000000000"]
        let usdc: [String: Any] = [
            "type": "erc20_trusted", "token": "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
            "delta": "2500000", "symbol": "USDC", "decimals": 6, "in_trusted_set": true,
        ]
        let weth: [String: Any] = [
            "type": "erc20_trusted", "token": "0x4200000000000000000000000000000000000006",
            "delta": "-40000000000000000", "symbol": "WETH", "decimals": 18,
        ]
        let unknown: [String: Any] = [
            "type": "erc20_unverified", "token": "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a", "delta": "123456789",
        ]
        switch verdict {
        case "send": return ([native], .answered)
        case "swap": return ([native, usdc], .answered)
        case "three": return ([native, weth, usdc], .answered)
        case "unverified": return ([native, unknown], .answered)
        case "nothing": return ([], .answered)
        case "caution":
            return (nil, .notice(risk: "caution", key: "componentsUi.signing.simUnavailableWarning", reason: nil))
        case "danger":
            return (nil, .notice(risk: "danger", key: "componentsUi.signing.simWillFailReason",
                                 reason: "ERC20: transfer amount exceeds balance"))
        default: return (nil, .pending)
        }
    }

    /// A site's transaction on Base, through the production builder — which
    /// is what keeps the verdict's place — with `verdict` standing in it.
    private func signing(_ verdict: String) -> SigningModel {
        let tx: [String: Any] = [
            "from": Self.address, "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
            "value": "0x14d1120d7b160000", "data": "0x",
        ]
        let params = String(decoding: (try? JSONSerialization.data(withJSONObject: [tx])) ?? Data(), as: UTF8.self)
        let request = SigningController.Incoming(
            id: "pr3c", method: "eth_sendTransaction", paramsJson: params,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 8_453
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8_453, blocked: nil
        )
        let sim = simulation(verdict)
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: SettingsLive.chainColor(8_453), nativeSymbol: "ETH",
            walletName: "Everyday wallet", walletAddress: Self.address
        )
        context.chainId = 8_453
        context.sim = sim.judgments.flatMap {
            try? CoreJSON.decode(TrustSimViewWire.self, from: ["ready": true, "judgments": $0])
        }
        context.simulation = sim.state
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: plainSend(to: tx["to"] as? String, value: tx["value"] as? String),
            guard: .empty, fee: HandoffFeeFixtures.feeView, context: context,
            gate: SignConfirmStateWire(enabled: true, block: nil, key: nil)
        )
    }

    /// The core's own reading of the request.
    private func plainSend(to: String?, value: String?) -> ClearSigningViewWire {
        guard let result = try? ClearSigningCore().dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": to as Any? ?? NSNull(), "data": "0x", "value": value as Any? ?? NSNull(),
            "chain_id": 8_453, "locale": SigningController.defaultLocale,
        ])), let object = try? CoreJSON.object(result), let view = object["view"] as? [String: Any],
            let clear = try? CoreJSON.decode(ClearSigningViewWire.self, from: view)
        else { return .empty }
        return clear
    }

    // MARK: - The switcher's count (F15)

    private func accounts(_ count: Int) -> SettingsScreenModel {
        let base = SettingsFixtures.build(.st2, loc: loc)
        let all: [[String: Any]] = [
            ["index": 0, "account": ["name": "Everyday wallet", "address": Self.address]],
            ["index": 1, "account": ["name": "Savings", "address": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"]],
        ]
        guard let session = try? CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.address,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": Array(all.prefix(count)),
        ]) else { return base }
        let balances = [
            BalanceCacheEntryWire(address: Self.address, usd: 1_383.28),
            BalanceCacheEntryWire(address: "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", usd: 250),
        ]
        return SettingsLive.withAccounts(
            session: session, balances: Array(balances.prefix(count)), display: .usd, on: base, loc: loc
        )
    }

    // MARK: - The sign-out sheet (F26)

    private func signOutBoard(many: Bool) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t("settings.signOut.title"))
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgBase)
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Layout.screenPaddingX)
        .background(theme.bgBase.ignoresSafeArea())
        .sheet(isPresented: .constant(true)) {
            SignOutSheet(
                loc: loc, pendingUploadWarning: many, accountCount: many ? 6 : 1,
                onConfirm: {}, onDismiss: {}
            )
            .themed(scheme)
        }
    }

    // MARK: - The batch importer (F8)

    /// The real `batch_import` core, opened on USDC and handed one row — in
    /// dollars it was never told were the person's (`unknown`), or told the
    /// person's currency.
    private func batch(unknown: Bool) -> FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd2c, loc: loc)
        guard case .batchImport(let drawn)? = model.sheet,
              let wire = batchView(code: unknown ? "USD" : "CNY", rate: unknown ? 1 : 7.1),
              let send = sendView
        else { return model }
        model.sheet = .batchImport(SendLive.batchImport(
            wire, view: send, on: drawn, loc: loc, currencyUnknown: unknown
        ))
        return model
    }

    private func batchView(code: String, rate: Double) -> BatchViewWire? {
        let core = BatchImportCore()
        guard let opened = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "open",
            "token": ["symbol": "USDC", "decimals": 6, "balance": "5000", "price_usd": 1.0],
            "currency_code": code, "max_recipients": BatchStore.maxRecipients,
        ]))) else { return nil }
        // The rate it asks for, where the currency is not the dollar.
        for effect in opened["effects"] as? [[String: Any]] ?? [] {
            guard let operation = effect["operation"] as? [String: Any],
                  operation["type"] as? String == "fetch_usd_fiat_rate",
                  let id = (effect["id"] as? NSNumber)?.uint64Value
            else { continue }
            _ = try? core.resolveEffect(effectId: id, resultJson: CoreJSON.string([
                "type": "rate_resolved", "code": code, "rate": rate,
            ]))
        }
        guard let pasted = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "set_raw_text",
            "text": "0x1111111111111111111111111111111111111111,1200\n0x2222222222222222222222222222222222222222,800",
        ]))), let view = pasted["view"] as? [String: Any] else { return nil }
        return try? CoreJSON.decode(BatchViewWire.self, from: view)
    }

    private var sendView: SendViewWire? {
        guard var object = try? CoreJSON.object(SendCore().view()) else { return nil }
        object["selected_token"] = [
            "network": "base", "chain_id": 8_453, "symbol": "USDC", "balance": "5000",
            "decimals": 6, "token_address": "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
            "price_usd": 1.0, "logo_urls": [String](), "spam": false,
        ]
        return try? CoreJSON.decode(SendViewWire.self, from: object)
    }
}

#endif
