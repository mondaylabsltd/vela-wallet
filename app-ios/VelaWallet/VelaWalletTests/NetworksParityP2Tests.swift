//
//  NetworksParityP2Tests.swift
//  VelaWalletTests
//
//  Native parity Phase 2, P2.1: the network pages say what the web's say
//  (`app-web/vela-wallet/src/lib/settings/live.ts`, `dapp/add-network.ts`) —
//  "More networks", a Vela network taken out and shown again, "Only use my
//  node" and the pool's word on the person's node, the URL refusal, the
//  multisig service's field, and the page's "Add unverified network".
//
//  Pure projections: each branch from a hand-built core view.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct NetworksParityP2Tests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    private func network(
        _ chainId: Int = 100, name: String = "Gnosis", custom: Bool = false, optIn: Bool = false,
        ownRpc: Bool = false, onlyMine: Bool = false, rpcRefused: Bool = false, explorerRefused: Bool = false,
        health: NetProbeHealthWire? = .ok(latencyMs: 80)
    ) -> NetNetworkRowWire {
        NetNetworkRowWire(
            id: custom ? "custom-\(chainId)" : name.lowercased(), chainId: chainId, displayName: name,
            nativeSymbol: "xDAI", isCustom: custom, rpcUrl: "https://rpc.example", explorerUrl: "https://scan.example",
            bundlerUrl: "", rpcHealth: health, explorerHealth: nil, rpcChainMismatch: nil, rpcSaveDeferred: false,
            isTestnet: false, optIn: optIn, ownRpc: ownRpc, onlyMine: onlyMine,
            rpcUrlRefused: rpcRefused, explorerUrlRefused: explorerRefused
        )
    }

    private func wizard(
        phase: NetWizardPhaseWire = .idle, suggestions: [NetChainIndexEntryWire] = [],
        info: NetChainInfoWire? = nil, canAdd: Bool = false, velaNetwork: Bool = false
    ) -> NetWizardViewWire {
        NetWizardViewWire(
            phase: phase, query: "", customRpc: "", suggestions: suggestions, chainInfo: info,
            compat: nil, error: nil, canAdd: canAdd, velaNetwork: velaNetwork
        )
    }

    private func view(_ networks: [NetNetworkRowWire], hidden: [Int] = []) -> NetViewWire {
        NetViewWire(
            loaded: true, networks: networks, wizard: wizard(), endpoints: [], lastAddedChainId: nil,
            hiddenVelaChainIds: hidden
        )
    }

    private var fallbackDetail: NetworkDetailModel { SettingsFixtures.build(.st9b, loc: en).networkDetail }

    // MARK: - The list

    /// Custom: its bin removes it. A Vela network shown on the person's word:
    /// its bin takes it out and the question says the settings stay. One the
    /// wallet ships with: no bin.
    @Test func whoGetsABinAndWhatItAsks() {
        let custom = SettingsLive.row(network(7_777_777, name: "Zora", custom: true), loc: en)
        #expect(custom.removable && custom.removeBody == nil)
        #expect(SettingsLive.removeNetworkConfirm(custom, loc: en).body == en.t("settingsModals.network.removeBody"))

        let stable = SettingsLive.row(network(988, name: "Stable", optIn: true), loc: en)
        #expect(stable.removable)
        #expect(stable.removeBody == "It leaves your network list. Its settings are kept, and Add Network brings it back anytime.")
        #expect(SettingsLive.removeNetworkConfirm(stable, loc: en).body == stable.removeBody)

        let gnosis = SettingsLive.row(network(), loc: en)
        #expect(!gnosis.removable)
        // A custom network wears its tag, never a latency (the web's row).
        #expect(custom.badge == nil && custom.tag != nil)
        #expect(gnosis.badge != nil)
    }

    /// "Also supported, not in your list: Stable and MegaETH." — the list in
    /// the reader's language (r10, `Loc.list`), under the
    /// MAINNET heading only, by name, and nothing when nothing is out.
    @Test func moreNetworksIsSaidUnderTheMainnetHeading() {
        let more = SettingsLive.moreNetworks(view([network()], hidden: [988, 4_326]), loc: en)
        #expect(more == "Also supported, not in your list: Stable and MegaETH. Add one from Add Network.")
        #expect(SettingsLive.moreNetworks(view([network()]), loc: en) == nil)

        let base = SettingsFixtures.build(.st9, loc: en)
        let mainnet = SettingsLive.withNetworks(view([network()], hidden: [988]), on: base, loc: en, space: .mainnet)
        #expect(mainnet.networkSpace?.hint.hasSuffix("Also supported, not in your list: Stable. Add one from Add Network.") == true)
        let testnet = SettingsLive.withNetworks(view([network()], hidden: [988]), on: base, loc: en, space: .testnet)
        #expect(testnet.networkSpace?.hint.contains("Stable") == false)
        let chinese = SettingsLive.moreNetworks(view([network()], hidden: [988]), loc: zh)
        #expect(chinese?.contains("Stable") == true && chinese?.contains("添加网络") == true)
    }

    /// The pool's word on the person's own node, on the network's row and its
    /// page — and only where the RPC is theirs.
    @Test func theOwnNodeNoticeIsThePoolsTwoLists() {
        let nodes = SettingsLive.OwnNodes(fallback: [100], unreached: [8_453])
        let fallback = SettingsLive.row(network(ownRpc: true), loc: en, nodes: nodes).notice
        #expect(fallback?.tone == .warning)
        #expect(fallback?.text == "Your node isn't answering — using public nodes for now")

        let down = SettingsLive.row(network(8_453, name: "Base", ownRpc: true), loc: en, nodes: nodes).notice
        #expect(down?.tone == .danger)
        #expect(down?.text == "Can't reach your node for Base")

        // Vela's own endpoint: nothing to say about "your node".
        #expect(SettingsLive.row(network(), loc: en, nodes: nodes).notice == nil)
        #expect(SettingsLive.row(network(ownRpc: true), loc: en, nodes: nil).notice == nil)

        let page = SettingsLive.detail(network(ownRpc: true), loc: en, fallback: fallbackDetail, nodes: nodes)
        #expect(page.nodeCallout?.tone == .warning)
    }

    /// The switch is offered only where the RPC is the person's own, in the
    /// corpus's words, in the state the core saved.
    @Test func onlyUseMyNodeIsOfferedOnlyOnTheirOwnRpc() {
        #expect(SettingsLive.detail(network(), loc: en, fallback: fallbackDetail).onlyMine == nil)
        let on = SettingsLive.detail(network(ownRpc: true, onlyMine: true), loc: en, fallback: fallbackDetail).onlyMine
        #expect(on?.on == true)
        #expect(on?.title == "Only use my node")
        #expect(on?.detail.hasPrefix("Nothing about your account goes to public nodes.") == true)
        let off = SettingsLive.detail(network(ownRpc: true), loc: zh, fallback: fallbackDetail).onlyMine
        #expect(off?.on == false && off?.title == "只使用我的节点")
    }

    /// set-6: a refused address is never drawn beside a green pill. The pill
    /// says HTTPS is required and the callout says nothing was saved.
    @Test func aRefusedAddressSaysItWasNotSaved() {
        let page = SettingsLive.detail(network(rpcRefused: true), loc: en, fallback: fallbackDetail)
        #expect(page.rpc.badge?.tone == .error)
        #expect(page.rpc.badge?.label == "HTTPS required")
        #expect(page.rpc.tone == .error)
        #expect(page.badge.tone == .error)
        #expect(page.callout?.tone == .danger)
        #expect(page.callout?.text.hasPrefix("Not saved — use an https:// address") == true)

        // A probe that failed says Offline, in the reader's words (the web's probePill).
        #expect(SettingsLive.row(network(health: .error), loc: en).badge?.label == "Offline")
        #expect(SettingsLive.row(network(health: .error), loc: zh).badge?.label == "离线")

        let explorer = SettingsLive.detail(network(explorerRefused: true), loc: en, fallback: fallbackDetail)
        #expect(explorer.explorer.badge?.label == "HTTPS required")
        #expect(explorer.rpc.badge?.tone != .error)
    }

    // MARK: - Add network shows a Vela network again

    @Test func aVelaNetworkIsShownAgainNotChecked() {
        let fallback = SettingsFixtures.build(.st10, loc: en).addNetwork
        let search = SettingsLive.wizard(wizard(phase: .suggested, suggestions: [
            NetChainIndexEntryWire(chainId: 988, name: "Stable", shortName: "stable",
                                   nativeCurrencySymbol: "USDT0", hasLogo: true, vela: true),
            NetChainIndexEntryWire(chainId: 7_777_777, name: "Zora", shortName: "zora",
                                   nativeCurrencySymbol: "ETH", hasLogo: true),
        ]), loc: en, fallback: fallback)
        #expect(search.results.map(\.tag) == ["Vela network", nil])

        let info = NetChainInfoWire(
            chainId: 988, name: "Stable", shortName: "stable", nativeName: "USDT0", nativeSymbol: "USDT0",
            nativeDecimals: 18, rpcUrl: "https://rpc.stable.xyz", rpcUrls: [], explorerUrl: "", logoUrl: "",
            isTestnet: false
        )
        let checked = SettingsLive.wizard(
            wizard(phase: .checked, info: info, canAdd: true, velaNetwork: true), loc: en, fallback: fallback
        )
        #expect(checked.candidate?.badge?.label == "Vela network")
        #expect(checked.callout?.tone == .info)
        #expect(checked.callout?.text.hasPrefix("Vela supports this network.") == true)
        #expect(checked.checks.isEmpty, "nothing was checked, and no row says it was")
        #expect(checked.customRpc == nil && checked.recheck == nil)
        #expect(checked.primary == en.t(I18nKeys.SettingsUi.addButton))
    }

    // MARK: - The multisig service's field

    @Test func theMultisigServiceHasItsOwnWords() throws {
        let raw: [String: Any] = [
            "field": "genesis_service", "value": "https://genesis.example", "default_value": "https://genesis.getvela.app",
            "health": ["type": "checking"],
        ]
        let endpoint = try CoreJSON.decode(NetEndpointViewWire.self, from: raw)
        #expect(endpoint.field == .genesisService)
        let model = SettingsLive.withEndpoints(
            NetViewWire(loaded: true, networks: [], wizard: wizard(), endpoints: [endpoint], lastAddedChainId: nil),
            on: SettingsFixtures.build(.st12, loc: en), loc: en
        )
        #expect(model.endpoints.fields.first?.label == "Multisig service")
        #expect(model.endpoints.fields.first?.id == "genesis_service")
    }

    // MARK: - A page asks to add a network (dapp-13)

    private func dappAdd(
        phase: NetDappAddPhaseWire, canAdd: Bool = false, unverified: Bool? = nil, vela: Bool? = nil,
        rpcFromSite: Bool? = nil
    ) -> NetDappAddViewWire {
        NetDappAddViewWire(
            tab: "t1", id: "a1", origin: "https://app.example", host: "app.example",
            chainId: 31_337, name: "Local", nativeSymbol: "ETH", rpcHost: "127.0.0.1:8545",
            explorerHost: nil, fromSite: false, phase: phase, reportedChainId: nil, compat: nil,
            canAdd: canAdd, canAddUnverified: unverified, velaNetwork: vela, rpcFromSite: rpcFromSite
        )
    }

    @Test func aCheckThatCouldNotBeMadeOffersTheUnverifiedAddWithItsRisk() {
        let sheet = ExploreLive.addNetwork(dappAdd(phase: .checkFailed, unverified: true), loc: en)
        #expect(sheet.unverified?.open == en.t("connect.browser.addUnverified"))
        #expect(sheet.unverified?.warning.contains("127.0.0.1:8545") == true)
        #expect(sheet.unverified?.confirm == en.t("connect.browser.addUnverifiedConfirm"))
        #expect(sheet.note == en.t("connect.browser.addWhyNoRpc"))
        #expect(ExploreLive.addNetwork(dappAdd(phase: .checkFailed), loc: en).unverified == nil)
    }

    @Test func aVelaNetworkAskedForByAPageIsSaidAsOne() {
        let sheet = ExploreLive.addNetwork(dappAdd(phase: .ready, canAdd: true, vela: true), loc: en)
        #expect(sheet.pill?.label == "Vela network")
        #expect(sheet.note == en.t("settingsModals.addNetwork.velaNetworkHint"))
        #expect(sheet.checks.isEmpty)
        let siteRpc = ExploreLive.addNetwork(dappAdd(phase: .checking, rpcFromSite: true), loc: en)
        #expect(siteRpc.fromSite == en.t("connect.browser.addRpcFromSite"))
    }
}
