//
//  Spec100Tests.swift
//  VelaWalletTests
//
//  Spec 100: a page's `wallet_addEthereumChain` through BOTH real machines
//  and this shell's executors, carried between them the way `RootView`
//  carries it — `forward_to_add_network` → `dapp_add_requested`,
//  `dapp_add_settled` → `add_network_answered`. A fake network answers the
//  sheet's probes (`NetworkAdminStub` for the rest); storage is real. The rules
//  themselves are the core's `app_dapp_add_network_100` tests.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

private let page = "http://127.0.0.1:8137"
private let sepolia = 11_155_111
private let catalogRpc = "https://rpc.sepolia.example"

@MainActor
struct Spec100Tests {
    /// The browser harness, the settings store, and the relay between them.
    @MainActor
    final class Rig {
        let harness: BrowserHarness
        let settings: SettingsStore
        let shelf: VelaStore
        /// Every RPC the sheet's check asked.
        let probes: Box<[String]>

        init(deployed: Bool) {
            harness = BrowserHarness()
            let defaults = UserDefaults(suiteName: "vela.tests.spec100.\(UUID().uuidString)")!
            shelf = VelaStore(defaults: defaults)
            let accounts = AccountStore(defaults: defaults)
            let executor = NetworkAdminExecutor(store: shelf, accounts: accounts)
            let stub = NetworkAdminStub.perform(
                executor: executor,
                reported: { _ in sepolia },
                chains: [sepolia: [
                    "chain_id": sepolia, "name": "Ethereum Sepolia",
                    "native_currency_symbol": "ETH", "rpc": [catalogRpc],
                ]]
            )
            let probes = Box<[String]>([])
            settings = SettingsStore(
                store: shelf, accounts: accounts,
                pool: RpcPool(store: shelf, accounts: accounts),
                networkPerform: { operation in
                    let url = operation["url"] as? String ?? ""
                    switch operation["type"] as? String ?? "" {
                    case "probe_rpc":
                        probes.value.append(url)
                    case "rpc_get_code":
                        return CoreJSON.string([
                            "type": "code", "url": url, "address": operation["address"] ?? "",
                            "code": deployed ? "0x6080604052" : "0x",
                        ])
                    case "rpc_call_p256":
                        return CoreJSON.string([
                            "type": "p256_call", "url": url,
                            "result": "0x" + String(repeating: "0", count: 63) + "1",
                        ])
                    default: break
                    }
                    return await stub(operation)
                }
            )
            self.probes = probes
            // The relay, as `RootView` wires it.
            harness.browser.ports.onForwardToAddNetwork = { [settings] forward in settings.dappAddRequested(forward) }
            harness.browser.ports.onCancelAddNetwork = { [settings] tab, id in settings.dappAddCancelled(tab: tab, id: id) }
            executor.onDappAddSettled = { [browser = harness.browser] tab, id, outcome in
                browser.addNetworkAnswered(tab: tab, id: id, outcome: outcome)
            }
        }

        final class Box<T> {
            var value: T
            init(_ value: T) { self.value = value }
        }

        func boot() async {
            settings.openNetworks()
            while !settings.isLoaded { try? await Task.sleep(nanoseconds: 10_000_000) }
            await harness.boot()
            await harness.hello("t1", doc: "d1", origin: page)
        }

        func addSepolia(id: String) async {
            await harness.ask(
                "t1", doc: "d1", origin: page, id: id, method: "wallet_addEthereumChain",
                params: [["chainId": "0xaa36a7", "chainName": "My Sepolia", "rpcUrls": ["https://rpc.invalid"]]]
            )
        }

        /// Until the sheet is in `phase` (or the networks machine is idle).
        func sheet(_ phase: NetDappAddPhaseWire) async -> NetDappAddViewWire? {
            for _ in 0..<1000 {
                if settings.networkAdmin?.dappAdd?.phase == phase { break }
                try? await Task.sleep(nanoseconds: 10_000_000)
            }
            return settings.networkAdmin?.dappAdd
        }

        func answer(_ id: String) async -> BrowserHarness.Delivery? {
            for _ in 0..<1000 {
                if let found = harness.answer("t1", id: id) { return found }
                try? await Task.sleep(nanoseconds: 10_000_000)
            }
            return nil
        }
    }

    @Test func anApprovedAddIsSavedTheSettingsWayAndAnswersNullWithChainChanged() async throws {
        let rig = Rig(deployed: true)
        await rig.boot()
        await rig.addSepolia(id: "a1")
        let sheet = try #require(await rig.sheet(.ready))
        #expect(sheet.name == "Ethereum Sepolia")
        #expect(!rig.probes.value.contains { $0.contains("rpc.invalid") }, "the page's RPC is never asked for a chain the catalog knows")

        // The sheet, in Settings' words.
        let model = ExploreLive.addNetwork(sheet, loc: Loc(overrideTag: "en", preferredLanguages: []))
        #expect(model.lead == "127.0.0.1:8137 asks to add a network")
        #expect(model.add == "Add Network")
        #expect(model.rows.first?.value == "Ethereum Sepolia")
        #expect(model.fromSite == nil)

        rig.settings.dappAddApproved()
        let answer = try #require(await rig.answer("a1"))
        #expect(answer.errorCode == nil)
        #expect(answer.result is NSNull)
        await rig.harness.until { rig.harness.events("t1").contains { $0.event == "chainChanged" } }
        #expect(rig.harness.events("t1").last { $0.event == "chainChanged" }?.message["data"] as? String == "0xaa36a7")
        #expect(rig.shelf.readList(VelaStore.Key.customNetworks).contains { ($0["chainId"] as? NSNumber)?.intValue == sepolia })
        #expect(rig.settings.networkAdmin?.dappAdd == nil)
    }

    @Test func anIncompatibleChainIsRefused4902AndNothingIsSaved() async throws {
        let rig = Rig(deployed: false)
        await rig.boot()
        await rig.addSepolia(id: "a1")
        let sheet = try #require(await rig.sheet(.notCompatible))
        let model = ExploreLive.addNetwork(sheet, loc: Loc(overrideTag: "en", preferredLanguages: []))
        #expect(model.add == nil, "no Add for a chain this wallet refuses")
        #expect(model.dismiss == "Done")
        #expect(model.setupTool == "Open Chain Setup Tool")

        rig.settings.dappAddDeclined()
        let answer = try #require(await rig.answer("a1"))
        #expect(answer.errorCode == 4902)
        #expect(rig.shelf.readList(VelaStore.Key.customNetworks).isEmpty)
    }

    @Test func aSecondAddWhileTheSheetIsOpenIs32002() async throws {
        let rig = Rig(deployed: true)
        await rig.boot()
        await rig.addSepolia(id: "a1")
        _ = await rig.sheet(.ready)
        await rig.addSepolia(id: "a2")
        #expect(rig.harness.answer("t1", id: "a2")?.errorCode == -32002)
        rig.settings.dappAddDeclined()
        #expect(await rig.answer("a1")?.errorCode == 4001)
    }

    @Test func theRecordsWordsCoverTheNewReasons() {
        #expect(DbrRecordWords.reason("not_compatible") == "addToken.errorNotCompatible")
        #expect(DbrRecordWords.reason("bad_rpc") == "componentsUi.browserStatus.reason.badRpc")
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(loc.t("componentsUi.browserStatus.reason.badRpc") == "The site gave no usable RPC for this network")
    }
}
