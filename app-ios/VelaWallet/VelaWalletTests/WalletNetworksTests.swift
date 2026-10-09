//
//  WalletNetworksTests.swift
//  VelaWalletTests
//
//  The receive list, the chain filters and a holding's chain name come from
//  the core's network list — the built-ins and the person's own — as they do
//  on Android, the web and the desktop. They came from the 24-chain
//  `ChainCatalog`, so a network somebody added had no receive row, no filter
//  row, and a blank name beside its tokens.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct WalletNetworksTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// A network the person added: chain 7777, "Vela Testnet", coin VTN.
    private static let customChainId = 7777

    private func row(_ chainId: Int, _ name: String, _ coin: String, custom: Bool) -> NetNetworkRowWire {
        NetNetworkRowWire(
            id: custom ? "custom-\(chainId)" : name.lowercased(), chainId: chainId,
            displayName: name, nativeSymbol: coin, isCustom: custom,
            rpcUrl: "https://rpc.example/\(chainId)", explorerUrl: "", bundlerUrl: "",
            rpcHealth: nil, explorerHealth: nil, rpcChainMismatch: nil, rpcSaveDeferred: false
        )
    }

    /// The core's list: every built-in, in the core's order, then the
    /// person's own.
    private var networks: WalletNetworks {
        WalletNetworks(ChainCatalog.chains.map { row($0.chainId, $0.displayName, $0.nativeSymbol, custom: false) }
            + [row(Self.customChainId, "Vela Testnet", "VTN", custom: true)])
    }

    private func holding(_ symbol: String, chainId: Int) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chainId, symbol: symbol, name: symbol, balance: "1",
                         decimals: 18, tokenAddress: nil, priceUsd: 1, spam: false)
    }

    private func balance(_ tokens: [BalanceTokenWire]) -> BalanceViewWire {
        BalanceViewWire(
            address: golden,
            displayTotalUsd: 1, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: tokens, unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    private func item(id: String, chainId: Int, symbol: String = "VTN") -> FeedItemWire {
        let now = Date().timeIntervalSince1970
        return FeedItemWire(
            id: id, direction: .in, counterparty: "0x9F3cA71b8021aE9F3cA71b8021aE9F3cA71b8021",
            alias: nil, value: "1", symbol: symbol, decimals: 18, usdValue: 0, chainId: chainId,
            timestamp: now, dayStartMs: TxRecords.dayStartMs(now), txHash: "0xabcdef0123456789",
            batch: nil, status: .confirmed, priced: false
        )
    }

    private func feed(_ items: [FeedItemWire]) -> FeedViewWire {
        FeedViewWire(rows: items.map { .item($0) }, transactions: [], newItemId: nil, toast: nil)
    }

    private var baseList: ReceiveListModel {
        guard case .receive(let model) = WalletFlowFixtures.build(.r1, loc: loc).base
        else { fatalError("the R1 fixture lost its receive list") }
        return model
    }

    private var drawnAssets: AssetsModel {
        guard case .assets(let model) = WalletFlowFixtures.build(.t1, loc: loc).base
        else { fatalError("T1 does not draw the assets list") }
        return model
    }

    private var baseTxDetail: TxDetailModel {
        guard case .txDetail(let model)? = WalletFlowFixtures.build(.a2, loc: loc).sheet
        else { fatalError("the A2 fixture lost its transaction sheet") }
        return model
    }

    // MARK: - The list itself

    /// Before the networks machine has read its stores, the catalogue stands
    /// in; once it has, the core's rows are the list, in its order, and a
    /// built-in is the catalogue's own entry.
    @Test func theListIsTheCoresOnceItHasOne() {
        #expect(WalletNetworks(nil).chains.map(\.chainId) == ChainCatalog.chains.map(\.chainId))
        #expect(WalletNetworks([]).chains.map(\.chainId) == ChainCatalog.chains.map(\.chainId))
        let list = networks
        #expect(list.chains.count == ChainCatalog.chains.count + 1)
        #expect(list.chains.last?.chainId == Self.customChainId)
        #expect(list.meta(Self.customChainId)?.displayName == "Vela Testnet")
        #expect(list.meta(Self.customChainId)?.nativeSymbol == "VTN")
        #expect(list.meta(100)?.apiNetworkId == ChainCatalog.meta(100)?.apiNetworkId)
        // A built-in stays nameable from a list that has not loaded.
        #expect(WalletNetworks.builtin.meta(8453)?.displayName == "Base")
    }

    // MARK: - Receive

    /// Every network the wallet has gets a receive row — the person's own
    /// with its own name, coin and logo — and the subtitle counts them all.
    @Test func theReceiveListHasTheNetworksThePersonAdded() {
        let live = FlowsLive.receiveList(golden, on: baseList, loc: loc, networks: networks)
        #expect(live.rows.count == ChainCatalog.chains.count + 1)
        #expect(live.subtitle.contains(String(ChainCatalog.chains.count + 1)))
        let own = live.rows.last
        #expect(own?.name == "Vela Testnet")
        #expect(own?.code == "VTN")
        #expect(own?.logoURLs == [Marks.chainLogoURL(Self.customChainId)].compactMap { $0 })
        #expect(own?.addressDisplay == AddressText.short(golden))
    }

    // MARK: - The filters

    /// The assets filter offers a network the person added when they hold
    /// something there, and the pill names it.
    @Test func theAssetsFilterOffersTheNetworkThePersonAdded() {
        let held = balance([holding("ETH", chainId: 1), holding("VTN", chainId: Self.customChainId)])
        let sheet = FlowsLive.assetChainSheet(held, selected: Self.customChainId, loc: loc,
                                              networks: networks)
        #expect(sheet.rows.map(\.chainId) == [nil, 1, Self.customChainId])
        #expect(sheet.rows.last?.name == "Vela Testnet")
        #expect(sheet.rows.last?.selected == true)
        #expect(sheet.rows.last?.logoUrl == Marks.chainLogoURL(Self.customChainId))

        let narrowed = FlowsLive.assets(held, currency: nil, selected: Self.customChainId,
                                        on: drawnAssets, loc: loc, networks: networks)
        #expect(narrowed.rows.map(\.ticker) == ["VTN"])
        #expect(narrowed.header.pill?.label == "Vela Testnet")
    }

    /// The history filter does too.
    @Test func theHistoryFilterOffersTheNetworkThePersonAdded() {
        let view = feed([item(id: "a", chainId: 1, symbol: "ETH"), item(id: "b", chainId: Self.customChainId)])
        let sheet = FlowsLive.chainSheet(view, selected: nil, loc: loc, networks: networks)
        #expect(sheet.rows.map(\.chainId) == [nil, 1, Self.customChainId])
        #expect(sheet.rows.last?.name == "Vela Testnet")
        // The catalogue alone never offered it.
        let old = FlowsLive.chainSheet(view, selected: nil, loc: loc)
        #expect(!old.rows.contains { $0.chainId == Self.customChainId })
    }

    // MARK: - Names

    /// A holding on a network the person added names its network; the
    /// catalogue left the line blank.
    @Test func aHoldingNamesTheNetworkThePersonAdded() {
        let held = balance([holding("VTN", chainId: Self.customChainId)])
        #expect(WalletLive.assetRows(held, networks: networks).first?.chain == "Vela Testnet")
        #expect(WalletLive.assetRows(held).first?.chain == "", "the catalogue alone had no name")
    }

    /// An activity row on a network the person added names it — on the
    /// home, the history, a token's page and a contact's page alike; the
    /// catalogue said "7777".
    @Test func anActivityRowNamesTheNetworkThePersonAdded() {
        let lines: [FeedLineWire] = [.network(chainId: Self.customChainId)]
        #expect(WalletLive.subtitleText(lines, loc: loc, networks: networks) == "Vela Testnet")
        #expect(WalletLive.subtitleText(lines, loc: loc) == String(Self.customChainId),
                "the catalogue alone named it by its id")
        var own = item(id: "a", chainId: Self.customChainId)
        own.subtitle = lines
        let view = FeedViewWire(rows: [.header(id: "today", dayStartMs: own.dayStartMs, timestamp: own.timestamp),
                                       .item(own)],
                                transactions: [], newItemId: nil, toast: nil)
        let home = WalletLive.apply(balance([]), feed: view,
                                    on: WalletFixtures.buildMobileState(.h1, loc: loc),
                                    loc: loc, networks: networks)
        #expect(home.activityGroups.first?.rows.first?.subtitle == "Vela Testnet")
        guard case .history(let drawn) = WalletFlowFixtures.build(.a1, loc: loc).base else {
            Issue.record("A1 does not draw the history")
            return
        }
        let history = FlowsLive.history(view, on: drawn, loc: loc, hidden: false, networks: networks)
        #expect(history.groups.first?.rows.first?.subtitle == "Vela Testnet")
    }

    /// The hero's "can't reach" line names a network the person added by its
    /// name, and so does the list it opens.
    @Test func anUnreachableNetworkThePersonAddedIsNamed() {
        var view = balance([holding("VTN", chainId: Self.customChainId)])
        view.unreachableNetworks = [UnreachableNetworkWire(
            chainId: Self.customChainId, lastKnown: "not_read", lastSeenUsd: nil,
            lineKey: I18nKeys.SettingsUi.notReadYet
        )]
        view.unreachableKey = I18nKeys.SettingsUi.unreachableOne
        let line = WalletLive.unreachableLine(view, loc: loc, networks: networks)
        #expect(line == loc.t(I18nKeys.SettingsUi.unreachableOne, vars: ["name": "Vela Testnet"]))
        let home = WalletLive.apply(view, on: WalletFixtures.buildMobileState(.h1, loc: loc),
                                    loc: loc, networks: networks)
        #expect(home.balance.status?.text == line)
        #expect(WalletLive.unreachableLine(view, loc: loc)?.contains("Vela Testnet") == false,
                "the catalogue alone could not name it")
        let sheet = SettingsLive.withUnreachable(view, display: .usd, on: SettingsFixtures.build(.sr3, loc: loc),
                                                 loc: loc, networks: networks)
        #expect(sheet.unreachable.title == line)
        #expect(sheet.unreachable.rows.first?.name == "Vela Testnet")
    }

    /// One network, one name, in every place a flow that starts at the hero's
    /// line reaches: the balance by network (SR3), the RPC fix its SR6 row
    /// opens (SR2) — its title and its mark's letter — and, in Explore, the
    /// connection panel's chip and switcher row, the chain-down notice, and a
    /// sign-in message's chain fact. Each read the catalogue and showed the
    /// network the person added as a bare chain id.
    @Test func aNetworkThePersonAddedHasOneNameAcrossTheRescueAndExplore() throws {
        let name = "Vela Testnet"
        var view = balance([holding("VTN", chainId: Self.customChainId)])
        view.unreachableNetworks = [UnreachableNetworkWire(
            chainId: Self.customChainId, lastKnown: "not_read", lastSeenUsd: nil,
            lineKey: I18nKeys.SettingsUi.notReadYet
        )]
        view.unreachableKey = I18nKeys.SettingsUi.unreachableOne
        let sr6 = SettingsLive.withUnreachable(view, display: .usd, on: SettingsFixtures.build(.sr3, loc: loc),
                                               loc: loc, networks: networks)
        #expect(sr6.unreachable.rows.first?.name == name)

        // SR3: the network's row, still being read.
        let sr3 = SettingsLive.withBalanceDetail(view, display: .usd, on: SettingsFixtures.build(.sr3, loc: loc),
                                                 loc: loc, networks: networks)
        let pending = try #require(sr3.balanceDetail.pending.first { $0.id == String(Self.customChainId) })
        #expect(pending.name == name)
        #expect(pending.mark.letter == "V")
        let catalogue = SettingsLive.withBalanceDetail(view, display: .usd,
                                                       on: SettingsFixtures.build(.sr3, loc: loc), loc: loc)
        #expect(catalogue.balanceDetail.pending.first?.name != name, "the catalogue alone could not name it")

        // SR2: the fix the SR6 row opens.
        let sr2 = SettingsLive.withRpcFix(chainId: Self.customChainId, endpoint: "https://rpc.example/7777",
                                          on: SettingsFixtures.build(.sr2, loc: loc), loc: loc, networks: networks)
        #expect(sr2.rpcFix.name == name)
        #expect(sr2.rpcFix.mark.letter == "V")

        // Explore: the page's network, its switcher row, the chain-down line.
        let dbr = DbrViewWire(
            ready: true, consent: nil,
            tabs: [DbrTabViewWire(tab: "t1", origin: "https://app.example", connectedAddress: golden,
                                  chainId: Self.customChainId, secure: true, crashed: false)],
            sites: [], signing: nil, queuedSigning: 0
        )
        let panel = ExploreLive.connectionModel(
            dbr: dbr, tab: dbr.tabs.first, engine: nil, identity: (name: "Me", address: golden),
            chainIds: [1, Self.customChainId], networks: networks, loc: loc
        )
        #expect(panel.network.name == name)
        #expect(panel.networks.first { $0.id == Self.customChainId }?.name == name)
        #expect(panel.networks.first { $0.id == 1 }?.name == "Ethereum")
        let notice = ExploreLive.chainNotice(chainId: Self.customChainId, failed: [Self.customChainId],
                                             rateLimited: [], loc: loc, networks: networks)
        #expect(notice == loc.t("explore.chainDown", vars: ["chain": name]))
        #expect(ExploreLive.chainNotice(chainId: Self.customChainId, failed: [Self.customChainId],
                                        rateLimited: [], loc: loc)?.contains(name) == false,
                "the catalogue alone could not name it")

        // A sign-in message for that network: its chain fact.
        let message = """
        app.example wants you to sign in with your Ethereum account:
        \(golden)

        Sign in to continue.

        URI: https://app.example/login
        Version: 1
        Chain ID: \(Self.customChainId)
        Nonce: abc123xyz
        Issued At: 2026-09-14T12:00:00Z
        """
        let core = ClearSigningCore()
        let answer = try core.dispatch(eventJson: CoreJSON.string([
            "type": "message_presented", "method": "personal_sign",
            "params": ["0x" + message.utf8.map { String(format: "%02x", $0) }.joined(), golden],
            "request_origin": "https://app.example",
        ]))
        let clear = try CoreJSON.decode(ClearSigningViewWire.self,
                                        from: try #require(CoreJSON.object(answer)["view"] as? [String: Any]))
        #expect(clear.message?.siwe?.chainId == Self.customChainId)
        let context = SigningLive.Context(
            loc: loc, chainName: name, chainDot: SettingsLive.chainColor(Self.customChainId),
            nativeSymbol: "VTN", walletName: "Vela", walletAddress: golden,
            chainId: Self.customChainId, origin: "https://app.example", networks: networks
        )
        let rows = SigningLive.blocks(clear: clear, to: nil, valueHex: nil, dataBytes: 0, context: context)
            .compactMap { block -> [SigningRow]? in
                if case .rows(let rows) = block { return rows }
                return nil
            }
            .flatMap { $0 }
        #expect(rows.first { $0.label == loc.t("componentsUi.signing.labelChain") }?.value == name)
    }

    /// A fee in the coin of a network the person added says its unit — on
    /// the send form, its speeds, its confirm and the signing sheet, which
    /// all write it with `feeLine`. The catalogue left the figure bare.
    @Test func aFeeOnANetworkThePersonAddedHasItsUnit() throws {
        let estimate = try CoreJSON.decode(FeeEstimateWire.self, from: [
            "chain_id": Self.customChainId, "total_wei": "2100000000000000",
            "max_fee_per_gas": "2000000000", "network_fee_per_gas": "1000000000",
            "relayer_fee_per_gas": "1000000000", "bundler_gas_price": "1000000000",
            "in_band_gas_basis": "0", "effective_gas_price": NSNull(), "max_gas_price": NSNull(),
            "total_gas": "21000", "deployed": true, "tier": "fast", "quoted": true,
            "fee_asset": ["type": "native"], "fee_recipient": "0xfee",
        ] as [String: Any])
        #expect(SendLive.feeText(estimate, networks: networks) == "0.0021 VTN")
        #expect(SendLive.feeLine(estimate, view: nil, fee: nil, display: .usd, networks: networks)
                == "0.0021 VTN")
        #expect(SendLive.feeText(estimate).hasSuffix(" "), "the catalogue alone had no unit")
        let context = SigningLive.Context(
            loc: loc, chainName: "Vela Testnet", chainDot: SettingsLive.chainColor(Self.customChainId),
            nativeSymbol: "VTN", walletName: "Vela", walletAddress: golden,
            chainId: Self.customChainId, networks: networks
        )
        let fee = FeeViewWire(busy: false, failed: nil, fee: estimate, stale: false, feeToken: nil,
                              options: [], confirmFeeReady: true)
        let clear = ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil, surface: .clearSign,
            confirm: .confirm, blindTyped: nil, dangerHaptic: false, plainSend: nil
        )
        guard case .onchain(_, let value, _, _, _) = SigningLive.feeModel(clear: clear, fee: fee, context: context)
        else {
            Issue.record("a transaction's fee row is on-chain")
            return
        }
        #expect(value == "~0.0021 VTN")
    }

    /// A transaction on a network the person added has its Network row, in
    /// the network's own mark — and a transfer of ETH on Base wears Base's
    /// logo there, never its coin's.
    @Test func aTransactionsNetworkRowWearsTheNetworksOwnMark() {
        let own = FlowsLive.txDetail(item(id: "a", chainId: Self.customChainId), record: nil,
                                     on: baseTxDetail, loc: loc, networks: networks)
        let label = loc.t("componentsTx.detail.labelChain")
        let fact = own.facts.first { $0.label == label }
        #expect(fact?.value == "Vela Testnet")
        guard case .token(let mark)? = fact?.lead else {
            Issue.record("the network row has no mark")
            return
        }
        #expect(mark.logoURLs == [Marks.chainLogoURL(Self.customChainId)].compactMap { $0 })
        #expect(mark.badgeHidden)
        #expect(mark.glyph == "VTN")

        let base = FlowsLive.txDetail(item(id: "b", chainId: 8453, symbol: "ETH"), record: nil,
                                      on: baseTxDetail, loc: loc, networks: networks)
        guard case .token(let baseMark)? = base.facts.first(where: { $0.label == label })?.lead else {
            Issue.record("Base's network row has no mark")
            return
        }
        #expect(baseMark.logoURLs == [Marks.chainLogoURL(8453)].compactMap { $0 })
    }
}
