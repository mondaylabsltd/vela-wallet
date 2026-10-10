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

        /// `p256: false` is a chain with no P-256 verifier: the known-good
        /// signature does not verify at `0x100`, and nothing lives there.
        init(deployed: Bool, p256: Bool = true) {
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
                pool: RpcPool(store: shelf, accounts: accounts, offline: true),
                networkPerform: { operation in
                    let url = operation["url"] as? String ?? ""
                    switch operation["type"] as? String ?? "" {
                    case "probe_rpc":
                        probes.value.append(url)
                    case "rpc_get_code":
                        let address = operation["address"] as? String ?? ""
                        let precompile = address.lowercased().hasSuffix("0100")
                            && address.dropFirst(2).dropLast(4).allSatisfy { $0 == "0" }
                        return CoreJSON.string([
                            "type": "code", "url": url, "address": address,
                            "code": deployed && (p256 || !precompile) ? "0x6080604052" : "0x",
                        ])
                    case "rpc_call_p256":
                        return CoreJSON.string([
                            "type": "p256_call", "url": url,
                            "result": p256 ? "0x" + String(repeating: "0", count: 63) + "1" : "0x",
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
        // PR 3 — missing contracts: the line speaks of contracts, and Chain
        // Setup opens on THIS chain (the core's `setup_url`).
        #expect(sheet.compat?.blocker == "missing_contracts")
        #expect(model.note == "Some contracts Vela needs aren't on this network yet. "
                + "Chain Setup shows which ones and who can deploy them.")
        #expect(model.setupTool == "Open Chain Setup Tool")
        #expect(model.setupUrl == "https://getvela.app/chain-setup?chain=11155111")
        #expect(model.checks.last?.ok == true, "the P-256 row is the precompile's, and it answered")

        rig.settings.dappAddDeclined()
        let answer = try #require(await rig.answer("a1"))
        #expect(answer.errorCode == 4902)
        #expect(rig.shelf.readList(VelaStore.Key.customNetworks).isEmpty)
    }

    /// PR 3 — a chain with no P-256 verifier, on the REAL core: the refusal
    /// says so plainly (Vela wallets cannot work there; money sent would be
    /// stuck) and offers NO Chain Setup — a precompile is the chain's own to
    /// add, there is nothing to deploy. Every contract can be there and the
    /// answer is the same; and it wins over missing contracts.
    @Test func aChainWithNoP256VerifierIsRefusedPlainlyWithNoSetupTool() async throws {
        for deployed in [true, false] {
            let rig = Rig(deployed: deployed, p256: false)
            await rig.boot()
            await rig.addSepolia(id: "a1")
            let sheet = try #require(await rig.sheet(.notCompatible))
            #expect(sheet.compat?.blocker == "no_p256", "contracts deployed: \(deployed)")
            #expect(sheet.compat?.setupUrl == nil)

            let model = ExploreLive.addNetwork(sheet, loc: Loc(overrideTag: "en", preferredLanguages: []))
            #expect(model.note?.hasPrefix("This network can't check passkey signatures") == true)
            #expect(model.note?.contains("Don't send money to your Vela address on this network") == true)
            #expect(model.setupTool == nil, "a button to a tool that cannot help")
            #expect(model.setupUrl == nil)
            #expect(model.add == nil)
            #expect(model.checks.last?.label == "P-256 precompile")
            #expect(model.checks.last?.ok == false)

            // The same check, as Settings draws it.
            let compat = try #require(sheet.compat)
            let rows = SettingsLive.checks(compat, loc: Loc(overrideTag: "en", preferredLanguages: []))
            #expect(rows.first { $0.label == "P-256 precompile" }?.ok == false)
            let refusal = try #require(SettingsLive.refusal(compat, loc: Loc(overrideTag: "zh", preferredLanguages: [])))
            #expect(refusal.callout.text.hasPrefix("这个网络无法验证通行密钥签名"))
            #expect(refusal.setup == nil)

            rig.settings.dappAddDeclined()
            #expect(try #require(await rig.answer("a1")).errorCode == 4902)
        }
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
