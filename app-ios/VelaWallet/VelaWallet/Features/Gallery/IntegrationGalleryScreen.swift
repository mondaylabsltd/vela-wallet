//
//  IntegrationGalleryScreen.swift
//  VelaWallet
//
//  Dev-only boards for PR 3's integration round (shell round 2), reached with
//  `VELA_PAGE=pr3b` and `VELA_STATE=<board>` — the states the round's notes
//  touch that no other gallery puts on screen. Every board is the PRODUCTION
//  view over fixture data, built by the production builders:
//
//  - `home-token-list` / `hero-plain` — the home when Tempo's token list could
//    not be loaded (note 4: the real `balance_dashboard` core's view — its
//    line, never "Can't reach Tempo"), and the same home with no line at all:
//    the hero keeps the line's room either way (note 26b).
//  - `list-token-list` / `list-network` — the list that line opens: Tempo's
//    row with NO "Fix", beside a network really out of reach, which keeps it.
//  - `split-history-shown|hidden` / `split-detail-shown|hidden` — the shared
//    privacy fixture's split (one send to two people): its row in History and
//    its detail, where each share is listed; hidden, the total and each share
//    read "•••• USDC" (notes 3, 11, 15).
//  - `wizard-no-p256` / `wizard-missing` — a refusal on the scan / auto-add
//    path (`phase: error` beside its check): the reason, and Chain Setup only
//    for missing contracts. `wizard-already` / `wizard-not-found` /
//    `wizard-no-rpc` / `wizard-unverified` — the other stops, each in the
//    core's sentence (notes 5, 10, 18).
//  - `checked-time` — "checked {{time}}" in a 12-hour clock at narrow widths:
//    the moment is one unbreakable unit (zh 「下午 2:32」 never splits), and its
//    word joiners draw as nothing (note 14).
//  - `home-waiting|committed`, `send-form-…`, `send-confirm-…`, `signing-…`,
//    `balance-detail-…`, `token-page-…` — the display currency on its way,
//    and the SAME frame once it commits (CNY): no fiat figure before, and
//    nothing moves when it lands (notes 9, 27).
//  - `flowsheet-long` — a failure sheet under a long message: as tall as its
//    content, its badge whole (note 26a).
//
//  Fixture data only: nothing here is read from a chain, scanned, signed or
//  sent — and, like every `VELA_PAGE` session, no camera is ever opened.
//

#if DEBUG

import SwiftUI
import VelaCore

struct IntegrationGalleryScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.colorScheme) private var scheme
    let loc: Loc
    let state: String

    var body: some View {
        switch state {
        case "hero-plain":
            WalletScreen(model: home(tokenList: true, line: false), loc: loc)
        case "list-token-list", "list-network":
            SettingsScreen(model: unreachableList(tokenList: state == "list-token-list"), loc: loc)
        case "split-history-shown", "split-history-hidden", "split-detail-shown", "split-detail-hidden":
            FlowHost(model: splitModel(detail: state.contains("detail"), hidden: state.hasSuffix("hidden")))
        case let board where board.hasPrefix("wizard-"):
            SettingsScreen(model: wizard(String(board.dropFirst("wizard-".count))), loc: loc)
        case "checked-time":
            checkedTimeBoard
        case "home-waiting", "home-committed":
            WalletScreen(model: currencyHome, loc: loc)
        case "send-form-waiting", "send-form-committed":
            FlowHost(model: sendFormModel, onRefreshFee: {})
        case "send-confirm-waiting", "send-confirm-committed":
            FlowHost(model: sendConfirmModel, onRefreshFee: {})
        case "signing-waiting", "signing-committed":
            SigningSheet(model: signingModel, onRefreshFee: {})
        case "balance-detail-waiting", "balance-detail-committed":
            SettingsScreen(model: balanceDetail, loc: loc)
        case "token-page-waiting", "token-page-committed":
            FlowHost(model: tokenPage)
        case "flowsheet-long":
            flowSheetBoard
        default:
            WalletScreen(model: home(tokenList: true, line: true), loc: loc)
        }
    }

    // MARK: - The display currency: on its way, then committed

    /// The currency this board draws in: the person's stored choice (CNY)
    /// with its rate still on its way — nothing committed — or committed.
    private var currency: CurrencyViewWire {
        state.hasSuffix("-waiting")
            ? CurrencyViewWire(code: "USD", rate: 1, committed: false, pending: "CNY")
            : CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
    }

    private var display: WalletLive.Display { .from(currency) }

    // MARK: - The home (notes 4, 26b)

    /// Tempo failed for want of its token list, or (the currency boards)
    /// Ethereum simply did not answer — as the real `balance_dashboard` core
    /// writes either round.
    private func balanceView(tokenList: Bool) -> BalanceViewWire? {
        tokenList
            ? BalanceCoreScene.view(failedChain: 4_217, internalFault: false, tokenListFault: true)
            : BalanceCoreScene.view(internalFault: false)
    }

    private func home(tokenList: Bool, line: Bool, currency: CurrencyViewWire? = nil) -> WalletHomeModel {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        guard let view = balanceView(tokenList: tokenList) else { return drawn }
        var model = WalletLive.apply(view, currency: currency, on: drawn, loc: loc)
        // The drawn home's own rows stay under the live hero and holdings.
        model.activityGroups = drawn.activityGroups
        model.activitySection = drawn.activitySection
        // The same home with no line under the figure: what moved the page
        // when the line arrived.
        if !line { model.balance.status = nil }
        return model
    }

    private var currencyHome: WalletHomeModel {
        home(tokenList: false, line: true, currency: currency)
    }

    private func unreachableList(tokenList: Bool) -> SettingsScreenModel {
        let base = SettingsFixtures.build(.sr6, loc: loc)
        let view = tokenList
            ? balanceView(tokenList: true)
            : BalanceCoreScene.view(failedChain: 4_217, internalFault: false)
        guard let view else { return base }
        return SettingsLive.withUnreachable(view, display: .usd, on: base, loc: loc)
    }

    // MARK: - The split (notes 3, 11, 15)

    /// The shared privacy fixture's feed, cut to three rows — received,
    /// sent, and the split (`rust/crates/vela-core/tests/fixtures/
    /// privacy-hidden.json`, as the core wrote it).
    private static let feedJson = #"""
        {"rows":[{"day_start_ms":1699920000000.0,"id":"day-1699920000000","timestamp":1699999940.0,"type":"header"},
        {"item":{"alias":null,"batch":null,"chain_id":1,"counterparty":"0xcccccccccccccccccccccccccccccccccccccccc",
        "counterparty_role":"recipient","day_start_ms":1699920000000.0,"decimals":6,"direction":"in",
        "figure_maskable":true,"id":"received","kind":"receive","priced":true,"site":null,"status":"confirmed",
        "subtitle":[{"address":"0xcccccccccccccccccccccccccccccccccccccccc","name":null,"type":"from"}],
        "symbol":"USDT","timestamp":1699999940.0,
        "tx_hash":"0xadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad","usd_value":289.5,
        "value":"289.5"},"type":"item"},{"day_start_ms":1699833600000.0,"id":"day-1699833600000",
        "timestamp":1699913600.0,"type":"header"},{"item":{"alias":"Bea","batch":null,"chain_id":1,
        "counterparty":"0xdddddddddddddddddddddddddddddddddddddddd","counterparty_role":"recipient",
        "day_start_ms":1699833600000.0,"decimals":6,"direction":"out","figure_maskable":true,"id":"sent",
        "kind":"send","priced":true,"site":null,"status":"confirmed",
        "subtitle":[{"address":"0xdddddddddddddddddddddddddddddddddddddddd","name":"Bea","type":"to"}],
        "symbol":"USDC","timestamp":1699913600.0,
        "tx_hash":"0xabababababababababababababababababababababababababababababababab","usd_value":163.25,
        "value":"163.25"},"type":"item"},{"item":{"alias":null,"batch":{"chain_id":1,"count":2,
        "from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ids":["split-a","split-b"],"kind":"split",
        "logo_urls":null,"status":"confirmed","symbol":"USDC","timestamp":1699913300.0,"to":null,"to_name":null,
        "total_usd":683.75,"transfers":[{"decimals":6,"logo_urls":null,"symbol":"USDC",
        "to":"0xdddddddddddddddddddddddddddddddddddddddd","to_name":"Bea","usd_value":214.5,"value":"214.5"},
        {"decimals":6,"logo_urls":null,"symbol":"USDC","to":"0xfafafafafafafafafafafafafafafafafafafafa",
        "to_name":null,"usd_value":469.25,"value":"469.25"}],
        "tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf",
        "user_op_hash":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef"},"chain_id":1,
        "counterparty":null,"counterparty_role":"recipient","day_start_ms":1699833600000.0,"decimals":6,
        "direction":"out","figure_maskable":true,
        "id":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef","kind":"send","priced":true,
        "site":null,"status":"confirmed","subtitle":[{"chain_id":1,"type":"network"}],"symbol":"USDC",
        "timestamp":1699913300.0,"tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf",
        "usd_value":683.75,"value":"683.75"},"type":"item"}],"home_rows":[{"day_start_ms":1699920000000.0,
        "id":"day-1699920000000","timestamp":1699999940.0,"type":"header"},{"item":{"alias":null,"batch":null,
        "chain_id":1,"counterparty":"0xcccccccccccccccccccccccccccccccccccccccc","counterparty_role":"recipient",
        "day_start_ms":1699920000000.0,"decimals":6,"direction":"in","figure_maskable":true,"id":"received",
        "kind":"receive","priced":true,"site":null,"status":"confirmed",
        "subtitle":[{"address":"0xcccccccccccccccccccccccccccccccccccccccc","name":null,"type":"from"}],
        "symbol":"USDT","timestamp":1699999940.0,
        "tx_hash":"0xadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad","usd_value":289.5,
        "value":"289.5"},"type":"item"},{"day_start_ms":1699833600000.0,"id":"day-1699833600000",
        "timestamp":1699913600.0,"type":"header"},{"item":{"alias":"Bea","batch":null,"chain_id":1,
        "counterparty":"0xdddddddddddddddddddddddddddddddddddddddd","counterparty_role":"recipient",
        "day_start_ms":1699833600000.0,"decimals":6,"direction":"out","figure_maskable":true,"id":"sent",
        "kind":"send","priced":true,"site":null,"status":"confirmed",
        "subtitle":[{"address":"0xdddddddddddddddddddddddddddddddddddddddd","name":"Bea","type":"to"}],
        "symbol":"USDC","timestamp":1699913600.0,
        "tx_hash":"0xabababababababababababababababababababababababababababababababab","usd_value":163.25,
        "value":"163.25"},"type":"item"},{"item":{"alias":null,"batch":{"chain_id":1,"count":2,
        "from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ids":["split-a","split-b"],"kind":"split",
        "logo_urls":null,"status":"confirmed","symbol":"USDC","timestamp":1699913300.0,"to":null,"to_name":null,
        "total_usd":683.75,"transfers":[{"decimals":6,"logo_urls":null,"symbol":"USDC",
        "to":"0xdddddddddddddddddddddddddddddddddddddddd","to_name":"Bea","usd_value":214.5,"value":"214.5"},
        {"decimals":6,"logo_urls":null,"symbol":"USDC","to":"0xfafafafafafafafafafafafafafafafafafafafa",
        "to_name":null,"usd_value":469.25,"value":"469.25"}],
        "tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf",
        "user_op_hash":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef"},"chain_id":1,
        "counterparty":null,"counterparty_role":"recipient","day_start_ms":1699833600000.0,"decimals":6,
        "direction":"out","figure_maskable":true,
        "id":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef","kind":"send","priced":true,
        "site":null,"status":"confirmed","subtitle":[{"chain_id":1,"type":"network"}],"symbol":"USDC",
        "timestamp":1699913300.0,"tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf",
        "usd_value":683.75,"value":"683.75"},"type":"item"}],"transactions":[{"chain_id":1,
        "day_start_ms":1699920000000.0,"decimals":6,"from":"0xcccccccccccccccccccccccccccccccccccccccc",
        "id":"received","kind":"receive","logo_urls":null,"status":"confirmed","symbol":"USDT",
        "timestamp":1699999940.0,"to":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","to_name":null,
        "tx_hash":"0xadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad","usd":"$289.50",
        "user_op_hash":"","value":"289.5"},{"chain_id":1,"day_start_ms":1699833600000.0,"decimals":6,
        "from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","id":"sent","kind":"send","logo_urls":null,
        "status":"confirmed","symbol":"USDC","timestamp":1699913600.0,
        "to":"0xdddddddddddddddddddddddddddddddddddddddd","to_name":"Bea",
        "tx_hash":"0xabababababababababababababababababababababababababababababababab","usd":"$163.25",
        "user_op_hash":"","value":"163.25"},{"chain_id":1,"day_start_ms":1699833600000.0,"decimals":6,
        "from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","id":"split-a","kind":"send","logo_urls":null,
        "status":"confirmed","symbol":"USDC","timestamp":1699913300.0,
        "to":"0xdddddddddddddddddddddddddddddddddddddddd","to_name":"Bea",
        "tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf","usd":"$214.50",
        "user_op_hash":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef","value":"214.5"},
        {"chain_id":1,"day_start_ms":1699833600000.0,"decimals":6,
        "from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","id":"split-b","kind":"send","logo_urls":null,
        "status":"confirmed","symbol":"USDC","timestamp":1699913300.0,
        "to":"0xfafafafafafafafafafafafafafafafafafafafa","to_name":null,
        "tx_hash":"0xafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf","usd":"$469.25",
        "user_op_hash":"0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef","value":"469.25"}],
        "new_item_id":null,"toast":null,"history_empty_key":"history.emptyTitle",
        "home_empty_key":"home.emptyNoActivity","contact_rows":[],"hidden":false}
        """#

    private static let splitId = "0x" + String(repeating: "ef", count: 32)

    private func splitModel(detail: Bool, hidden: Bool) -> FlowScreenModel {
        var model = WalletFlowFixtures.build(detail ? .a2 : .a1, loc: loc)
        guard let feed = try? CoreJSON.decoder.decode(FeedViewWire.self, from: Data(Self.feedJson.utf8))
        else { return model }
        if case .history(let drawn) = model.base {
            model.base = .history(FlowsLive.history(feed, on: drawn, loc: loc, hidden: hidden))
        }
        if case .txDetail(let drawn)? = model.sheet,
           let item = FlowsLive.items(feed).first(where: { $0.id == Self.splitId }) {
            model.sheet = .txDetail(FlowsLive.txDetail(
                item, record: nil, on: drawn, loc: loc, hidden: hidden
            ))
        }
        return model
    }

    // MARK: - The wizard's stops (notes 5, 10, 18)

    private static let refusedChain = 48_900

    private static var zircuit: NetChainInfoWire {
        NetChainInfoWire(
            chainId: refusedChain, name: "Zircuit", shortName: "zircuit", nativeName: "Ether",
            nativeSymbol: "ETH", nativeDecimals: 18, rpcUrl: "https://rpc.example", rpcUrls: [],
            explorerUrl: "https://explorer.example", logoUrl: "", isTestnet: false
        )
    }

    /// The check the scan / auto-add path now keeps beside its refusal, with
    /// the core's own ruling written out (`net_blocker`).
    private static func compat(noP256: Bool) -> NetCompatibilityWire {
        let names = [
            "Deterministic Deployment Proxy", "Safe Singleton Factory", "Multicall3",
            "EntryPoint v0.7", "Safe L2", "Safe Proxy Factory", "Safe 4337 Module",
            "Safe Module Setup", "WebAuthn Signer", "MultiSend",
        ]
        let missing: Set<String> = noP256 ? [] : ["Safe L2", "Safe 4337 Module", "Safe Module Setup"]
        return NetCompatibilityWire(
            chainId: refusedChain, compatible: false, multiKeyReady: false,
            contracts: names.map {
                NetContractStatusWire(name: $0, address: "0x", deployed: !missing.contains($0), multiKeyOnly: false)
            },
            p256Available: !noP256, bestRpcUrl: "https://rpc.example", bestRpcLatencyMs: 182, rpcFailure: nil,
            blocker: noP256 ? "no_p256" : "missing_contracts",
            hintKey: "settingsModals.addNetwork." + (noP256 ? "noP256Hint" : "incompatibleHint"),
            setupUrl: noP256 ? nil : "https://getvela.app/chain-setup?chain=\(refusedChain)"
        )
    }

    /// The wizard's view at each stop, as the core writes it: the error, the
    /// sentence it names for it (`error_key`), for a refusal the check
    /// itself raised the check beside it, and whether naming an RPC is a way
    /// on (`rpc_field` — none under a refusal, final note F22).
    private func wizard(_ stop: String) -> SettingsScreenModel {
        let k = I18nKeys.SettingsUi.self
        let chain = Self.refusedChain
        let view: NetWizardViewWire
        switch stop {
        case "no-p256", "missing":
            let refused = Self.compat(noP256: stop == "no-p256")
            view = NetWizardViewWire(
                phase: .error, query: "", customRpc: "", suggestions: [], chainInfo: Self.zircuit,
                compat: refused, error: .notCompatible(chainId: chain), errorKey: refused.hintKey, canAdd: false
            )
        case "unverified":
            view = NetWizardViewWire(
                phase: .error, query: "", customRpc: "", suggestions: [], chainInfo: Self.zircuit,
                compat: nil, error: .checkFailed(chainId: chain), errorKey: k.addUnableToVerify,
                // The core's rule: another endpoint may answer.
                rpcField: .optional, rpcFieldLabelKey: k.addCustomRpcTitle, canAdd: false
            )
        case "no-rpc":
            view = NetWizardViewWire(
                phase: .error, query: "", customRpc: "", suggestions: [], chainInfo: Self.zircuit,
                compat: nil, error: .noRpcEndpoint, errorKey: k.addNoRpcEndpoint,
                // … and here one typed is the only way on: "RPC URL".
                rpcField: .required, rpcFieldLabelKey: k.fieldRpcUrl, canAdd: false
            )
        case "already":
            view = NetWizardViewWire(
                phase: .error, query: "100", customRpc: "", suggestions: [], chainInfo: nil,
                compat: nil, error: .alreadyAdded(chainId: 100), errorKey: k.addAlreadyAdded, canAdd: false
            )
        default:
            view = NetWizardViewWire(
                phase: .error, query: "424242", customRpc: "", suggestions: [], chainInfo: nil,
                compat: nil, error: .notFound(chainId: 424_242), errorKey: k.addChainNotFound, canAdd: false
            )
        }
        var model = SettingsFixtures.build(view.chainInfo == nil ? .st10 : .st10c, loc: loc)
        model.addNetwork = SettingsLive.wizard(view, loc: loc, fallback: model.addNetwork)
        return model
    }

    // MARK: - "checked {{time}}" at narrow widths (note 14)

    /// 14:32 today and yesterday, read at 15:00: the clock time alone, and
    /// the date with it — in a 12-hour clock, whose day period is the part
    /// a CJK line used to break inside.
    private var checkedTimes: [String] {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        let today = calendar.startOfDay(for: Date())
        let now = today.addingTimeInterval(15 * 3_600)
        let offset = Int32(TimeZone.current.secondsFromGMT(for: now) / 60)
        return [today.addingTimeInterval(14 * 3_600 + 32 * 60), today.addingTimeInterval(-9 * 3_600 - 28 * 60)].map { at in
            signerIntegrityTime(
                checkedAtMs: UInt64(at.timeIntervalSince1970 * 1_000), nowMs: UInt64(now.timeIntervalSince1970 * 1_000),
                utcOffsetMinutes: offset, dateFormat: Formats.resolve(Formats.current.date).rawValue,
                timeFormat: TimeFormatKey.h12.rawValue, language: loc.resolvedLanguage
            )
        }
    }

    private var checkedTimeBoard: some View {
        let key = "componentsUi.signing.integrity.matches"
        let line = SignerIntegrityLine(state: .matches, version: "0ba8ee8c", checkedAtMs: 1, key: key, opens: true)
        let times = checkedTimes
        return ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                ForEach(Array(times.enumerated()), id: \.offset) { _, time in
                    // The moment by itself, as the core wrote it.
                    Text(verbatim: time)
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgBase)
                        .accessibilityIdentifier("checkedTime.moment")
                    ForEach(Self.narrowWidths, id: \.self) { width in
                        IntegrityLineView(line: line, text: loc.t(key, vars: ["version": line.version, "time": time]))
                            .frame(width: width, alignment: .leading)
                            .padding(Tokens.Space.s8)
                            .background(theme.bgRaised, in: RoundedRectangle(cornerRadius: Tokens.Radius.r8))
                    }
                }
            }
            .padding(Tokens.Layout.screenPaddingX)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(theme.bgBase.ignoresSafeArea())
    }

    /// Widths a card's line can come to on a small phone, a split view or at
    /// a large text size — each a different place for a line to want to break.
    private static let narrowWidths: [CGFloat] = [
        Tokens.Space.s48 * 2 + Tokens.Space.s24, Tokens.Space.s48 * 3, Tokens.Space.s48 * 3 + Tokens.Space.s24,
        Tokens.Space.s48 * 4, Tokens.Space.s48 * 4 + Tokens.Space.s32, Tokens.Space.s48 * 5 + Tokens.Space.s24,
    ]

    // MARK: - Send and the signing sheet (notes 9, 27)

    private static let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let recipient = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private static let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"

    /// A settled fee of 0.02 USDC on Base, and the relay's row that prices
    /// it — so the fee has a fiat half to withhold.
    private var fee: FeeViewWire? {
        let coin = #"{"type":"erc20","token":"\#(Self.usdc)","decimals":6,"amount":"20000","symbol":"USDC"}"#
        let json = """
        {"busy":false,"failed":null,"fee":{"chain_id":8453,"total_wei":"21000000000000",\
        "max_fee_per_gas":"1000000","network_fee_per_gas":"1000000","relayer_fee_per_gas":"0",\
        "bundler_gas_price":"1000000","in_band_gas_basis":"0","effective_gas_price":null,\
        "max_gas_price":null,"total_gas":"21000","deployed":true,"tier":"standard","quoted":true,\
        "fee_asset":\(coin),"fee_recipient":null},"stale":false,"fee_token":"\(Self.usdc)",\
        "options":[{"symbol":"USDC","contract":"\(Self.usdc)","decimals":6,"balance":"250000000",\
        "recipient":"0x0000000000000000000000000000000000000001","usd_balance":"250","usd_price":"1",\
        "amount":"20000","insufficient":false,"selected":true}],"confirm_fee_ready":true}
        """
        return try? CoreJSON.decoder.decode(FeeViewWire.self, from: Data(json.utf8))
    }

    private func sendView(stage: String) -> SendViewWire? {
        guard var object = try? CoreJSON.object(SendCore().view()) else { return nil }
        let token: [String: Any] = [
            "network": "base", "chain_id": 8_453, "symbol": "USDC", "balance": "250",
            "decimals": 6, "token_address": Self.usdc, "price_usd": 1.0, "logo_urls": [String](), "spam": false,
        ]
        // The send machine's own estimate: 0.02 USDC, the fee view's.
        let estimate: [String: Any] = [
            "chain_id": 8_453, "total_wei": "21000000000000", "max_fee_per_gas": "1000000",
            "total_gas": "21000", "deployed": true, "quoted": true, "tier": "standard",
            "fee_asset": ["type": "erc20", "token": Self.usdc, "decimals": 6, "amount": "20000", "symbol": "USDC"],
            "fee_recipient": NSNull(),
        ]
        let patch: [String: Any] = [
            "stage": stage, "selected_token": token, "recipient": Self.recipient,
            "amount": "25", "token_amount": "25", "confirm_amount": "25", "fee": estimate,
        ]
        for (key, value) in patch { object[key] = value }
        return try? CoreJSON.decode(SendViewWire.self, from: object)
    }

    private var speedInputs: SendLive.SpeedInputs? {
        let fee = self.fee
        return HandoffFeeFixtures.speedView.map { SendLive.SpeedInputs(view: $0, feeView: { _ in fee }) }
    }

    private var sendFormModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd2, loc: loc)
        if case .sendForm(let drawn) = model.base, let view = sendView(stage: "enter_details") {
            model.base = .sendForm(SendLive.form(
                view, fee: fee, display: display, on: drawn, loc: loc, speed: speedInputs
            ))
        }
        return model
    }

    private var sendConfirmModel: FlowScreenModel {
        var model = WalletFlowFixtures.build(.sd3, loc: loc)
        if case .sendConfirm(let drawn) = model.base, let view = sendView(stage: "confirm") {
            model.base = .sendConfirm(SendLive.confirm(
                view, from: (Self.address, "Everyday wallet"), display: display, on: drawn, loc: loc,
                fee: fee, speed: HandoffFeeFixtures.speedView
            ))
        }
        return model
    }

    private var signingModel: SigningModel {
        let tx: [String: Any] = [
            "from": Self.address, "to": Self.recipient, "value": "0x38d7ea4c68000", "data": "0x",
        ]
        let params = String(decoding: (try? JSONSerialization.data(withJSONObject: [tx])) ?? Data(), as: UTF8.self)
        let request = SigningController.Incoming(
            id: "pr3b", method: "eth_sendTransaction", paramsJson: params,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 8_453
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 8_453, blocked: nil
        )
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: SettingsLive.chainColor(8_453), nativeSymbol: "ETH",
            walletName: "Everyday wallet", walletAddress: Self.address
        )
        context.chainId = 8_453
        // The one thing these two boards differ in.
        context.display = display
        let fee = self.fee
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
            clear: .empty, guard: .empty, fee: fee, context: context,
            speed: HandoffFeeFixtures.speedView.map { SendLive.SpeedInputs(view: $0, feeView: { _ in fee }) },
            gate: SignConfirmStateWire(enabled: true, block: nil, key: nil)
        )
    }

    // MARK: - The balance detail sheet and the token page (notes 9, 27)

    private var balanceDetail: SettingsScreenModel {
        let base = SettingsFixtures.build(.sr3, loc: loc)
        guard let view = balanceView(tokenList: false) else { return base }
        return SettingsLive.withBalanceDetail(view, display: display, on: base, loc: loc)
    }

    private var tokenPage: FlowScreenModel {
        var model = WalletFlowFixtures.build(.t2, loc: loc)
        let held = BalanceTokenWire(
            chainId: 8_453, symbol: "ETH", name: "Ether", balance: "0.5", decimals: 18,
            tokenAddress: nil, priceUsd: 1_941.56, spam: false
        )
        if case .tokenDetail(let drawn)? = model.sheet {
            model.sheet = .tokenDetail(FlowsLive.tokenDetail(held, feed: nil, display: display, on: drawn, loc: loc))
        }
        return model
    }

    // MARK: - A failure sheet under a long message (note 26a)

    /// The longest thing a prompt carries: a refusal's own detail, three
    /// lines of it on a phone.
    private var flowSheetBoard: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(loc.t(I18nKeys.Create.keysTitle))
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgBase)
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Layout.screenPaddingX)
        .background(theme.bgBase.ignoresSafeArea())
        .sheet(isPresented: .constant(true)) {
            FlowSheet(
                loc: loc,
                kind: PromptKind(
                    type: "create_failed",
                    detail: "Register failed: the passkey index at p256-index-v2.getvela.app answered 503 "
                        + "(Service Unavailable) three times in a row, and the last attempt timed out after "
                        + "120 seconds. Nothing was written, and no passkey was lost."
                ),
                confirmable: false
            ) { _ in }
            .themed(scheme)
        }
    }
}

#endif
