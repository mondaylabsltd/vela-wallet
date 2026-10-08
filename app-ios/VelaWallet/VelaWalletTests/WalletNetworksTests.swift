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
