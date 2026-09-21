//
//  NetworkEventsTests.swift
//  VelaWalletTests
//
//  Every event this shell sends `network_admin`, put to the REAL machine
//  (spec 072 T030).
//
//  Until 072 the endpoints page, the providers page, a network's RPC edit, the
//  RPC fix's save and the scan path's add all sent events the core could not
//  read: `"id"` where it reads `field` or `provider`, no `field` at all, `text`
//  for a chain id. Each was refused with a line in the log and nothing on the
//  screen — and nothing in this target noticed, because no test ever sent them.
//  Here every one goes through `SettingsStore`'s own methods into the machine,
//  and each test asserts what the machine DID with it: a refused event changes
//  nothing, so each of these fails on the old spelling.
//
//  Hermetic. Storage is the real executor over a fresh suite (so a save is
//  checked where it lands); everything that would reach a network is answered
//  here.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct NetworkEventsTests {

    /// Which operations the machine asked for, in order, and of which URLs.
    final class Asked {
        var types: [String] = []
        var urls: [String] = []
    }

    struct World {
        let settings: SettingsStore
        let shelf: VelaStore
        let accounts: AccountStore
        let asked: Asked
    }

    /// A booted, LOADED networks machine. The core drops every mutation that
    /// arrives before `loaded`, so a test that skipped the wait would be
    /// asserting about events the machine ignored.
    ///
    /// `reported` is what an RPC answers to `eth_chainId`, per URL; `chains`
    /// is what the chain-data endpoint knows (raw, as the executor sends it).
    private func world(
        seed: (VelaStore) -> Void = { _ in },
        reported: @escaping (String) -> Int? = { _ in nil },
        chains: [Int: [String: Any]] = [:]
    ) async -> World {
        let defaults = UserDefaults(suiteName: "vela.tests.netevents.\(UUID().uuidString)")!
        let shelf = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        seed(shelf)
        let executor = NetworkAdminExecutor(store: shelf, accounts: accounts)
        let asked = Asked()
        let settings = SettingsStore(
            store: shelf, accounts: accounts,
            pool: RpcPool(store: shelf, accounts: accounts),
            networkPerform: { operation in
                let type = operation["type"] as? String ?? ""
                asked.types.append(type)
                let url = operation["url"] as? String ?? ""
                if !url.isEmpty { asked.urls.append(url) }
                switch type {
                case "probe_rpc":
                    return CoreJSON.string([
                        "type": "probed", "url": url,
                        "reported_chain_id": reported(url) ?? NSNull(), "latency_ms": 12,
                    ])
                case "probe_reachable":
                    return CoreJSON.string(["type": "reachable", "url": url, "ok": true, "latency_ms": 9])
                case "fetch_service_health":
                    return CoreJSON.string([
                        "type": "service_health", "field": operation["field"] ?? "",
                        "body": ["type": "failed"], "latency_ms": 0,
                    ])
                case "fetch_fiat_rates":
                    return CoreJSON.string(["type": "fiat_rates", "body": ["type": "failed"], "latency_ms": 0])
                case "fetch_chain_info":
                    let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
                    return CoreJSON.string([
                        "type": "chain_info", "chain_id": chainId,
                        "data": chains[chainId] ?? NSNull(),
                    ])
                case "fetch_search_index":
                    return CoreJSON.string(["type": "search_index", "chains": []])
                case "rpc_get_code":
                    return CoreJSON.string([
                        "type": "code", "url": url, "address": operation["address"] ?? "", "code": NSNull(),
                    ])
                case "rpc_call_p256":
                    return CoreJSON.string(["type": "p256_call", "url": url, "result": NSNull()])
                default:
                    // Storage, the debounce, the pool and bundler flushes: the
                    // real executor, so what is saved is what is on disk.
                    return await executor.perform(operation)
                }
            }
        )
        settings.openNetworks()
        await settle { settings.isLoaded }
        #expect(settings.isLoaded, "the networks machine never loaded")
        return World(settings: settings, shelf: shelf, accounts: accounts, asked: asked)
    }

    private func settle(seconds: Double = 5, until condition: () -> Bool) async {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline, !condition() {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    private func endpoint(_ world: World, _ field: NetEndpointFieldWire) -> NetEndpointViewWire? {
        world.settings.networkAdmin?.endpoints.first { $0.field == field }
    }

    private func provider(_ world: World, _ id: NetProviderIdWire) -> NetProviderViewWire? {
        world.settings.networkAdmin?.providers.first { $0.provider == id }
    }

    private func row(_ world: World, chainId: Int) -> NetNetworkRowWire? {
        world.settings.networkAdmin?.networks.first { $0.chainId == chainId }
    }

    // MARK: - Service endpoints

    /// `endpoint_edited` / `endpoint_blurred` carry `field` — and the blur is
    /// a SAVE, under the camelCase name every client reads.
    @Test func anEndpointEditIsShownAndItsBlurIsSaved() async {
        let world = await world()
        let url = "https://index.example.org"

        world.settings.editEndpoint(.passkeyIndex, value: url)
        #expect(endpoint(world, .passkeyIndex)?.value == url, "the edit never reached the core")

        world.settings.blurEndpoint(.passkeyIndex)
        await settle { world.shelf.readObject(VelaStore.Key.serviceEndpoints)["passkeyIndexURL"] as? String == url }
        #expect(world.shelf.readObject(VelaStore.Key.serviceEndpoints)["passkeyIndexURL"] as? String == url)
        // And the page asked the new address how it is.
        #expect(world.asked.types.contains("fetch_service_health"))
    }

    /// The page opening probes all four, so every pill is about now; a
    /// service that cannot be reached is said to be unreachable.
    @Test func openingTheEndpointsPageProbesEveryField() async {
        let world = await world()
        world.settings.openEndpoints()
        await settle {
            NetEndpointFieldWire.allCases.allSatisfy { field in
                if case .unreachable = endpoint(world, field)?.health { return true }
                return false
            }
        }
        for field in NetEndpointFieldWire.allCases {
            guard case .unreachable = endpoint(world, field)?.health else {
                Issue.record("\(field) was never probed: \(String(describing: endpoint(world, field)?.health))")
                continue
            }
        }
    }

    /// "Reset to defaults" writes the defaults over what was typed.
    @Test func resetPutsTheDefaultsBack() async {
        let world = await world()
        world.settings.editEndpoint(.bundlerService, value: "https://relay.example.org")
        world.settings.blurEndpoint(.bundlerService)
        await settle { world.shelf.readObject(VelaStore.Key.serviceEndpoints)["bundlerServiceURL"] != nil }

        world.settings.resetEndpoints()
        #expect(endpoint(world, .bundlerService)?.value != "https://relay.example.org")
        await settle { world.shelf.readObject(VelaStore.Key.serviceEndpoints)["bundlerServiceURL"] as? String != "https://relay.example.org" }
        #expect(world.shelf.readObject(VelaStore.Key.serviceEndpoints)["bundlerServiceURL"] as? String != "https://relay.example.org")
    }

    // MARK: - RPC providers

    /// `provider_key_edited` / `_blurred` / `provider_test_requested` carry
    /// `provider`. The blur saves the key; the test runs against it.
    @Test func aProviderKeyIsShownSavedAndTested() async {
        let world = await world()

        world.settings.openProviders()
        world.settings.editProviderKey(.alchemy, value: "k3y")
        #expect(provider(world, .alchemy)?.key == "k3y", "the keystroke never reached the core")

        world.settings.blurProviderKey(.alchemy)
        await settle { (world.shelf.readObject(VelaStore.Key.rpcProviders)["alchemy"] as? String) == "k3y" }
        #expect((world.shelf.readObject(VelaStore.Key.rpcProviders)["alchemy"] as? String) == "k3y")
        #expect(provider(world, .alchemy)?.hasKey == true)

        world.settings.testProvider(.alchemy)
        await settle { provider(world, .alchemy)?.test?.done == true }
        let test = provider(world, .alchemy)?.test
        #expect(test?.done == true, "the test never ran")
        #expect((test?.total ?? 0) > 0)
    }

    /// A key saved before — by this device or another — is what the page
    /// shows once it opens, and focusing another field does not remove it
    /// (the core's FR-001, reached through this shell's own events).
    @Test func aSavedKeySurvivesTheProvidersPage() async {
        let world = await world(seed: { shelf in
            shelf.writeObject(VelaStore.Key.rpcProviders, ["drpc": "saved-key"])
        })
        world.settings.openProviders()
        #expect(provider(world, .drpc)?.key == "saved-key")

        // Another provider's field touched and left.
        world.settings.editProviderKey(.ankr, value: "a")
        world.settings.blurProviderKey(.ankr)
        await settle { world.shelf.readObject(VelaStore.Key.rpcProviders)["ankr"] != nil }
        #expect((world.shelf.readObject(VelaStore.Key.rpcProviders)["drpc"] as? String) == "saved-key")
    }

    // MARK: - A network's own RPC and explorer

    /// `override_field_edited` carries `field`, for both fields; the blur
    /// saves once the RPC has answered as THIS chain.
    @Test func aNetworksRpcAndExplorerAreEditedAndSaved() async {
        let world = await world(reported: { _ in 100 })
        let rpc = "https://gnosis.example.org"
        let explorer = "https://scan.example.org"

        world.settings.expandNetwork(chainId: 100)
        world.settings.editOverride(chainId: 100, field: .rpc, value: rpc)
        world.settings.editOverride(chainId: 100, field: .explorer, value: explorer)
        #expect(row(world, chainId: 100)?.rpcUrl == rpc, "the RPC edit never reached the core")
        #expect(row(world, chainId: 100)?.explorerUrl == explorer)

        world.settings.blurOverride(chainId: 100)
        let saved = { world.shelf.readList(VelaStore.Key.networkConfig).first { ($0["chainId"] as? Int) == 100 } }
        await settle { saved()?["rpcURL"] as? String == rpc }
        #expect(saved()?["rpcURL"] as? String == rpc)
        #expect(saved()?["explorerURL"] as? String == explorer)
    }

    /// An RPC that answers as ANOTHER chain is refused, said, and not saved.
    @Test func anRpcForAnotherChainIsRefused() async {
        let world = await world(reported: { url in url.contains("bsc") ? 56 : 100 })
        world.settings.expandNetwork(chainId: 100)
        world.settings.editOverride(chainId: 100, field: .rpc, value: "https://bsc.example.org")
        world.settings.blurOverride(chainId: 100)
        await settle { row(world, chainId: 100)?.rpcChainMismatch != nil }

        #expect(row(world, chainId: 100)?.rpcChainMismatch
                == NetChainMismatchWire(expectedChainId: 100, reportedChainId: 56))
        #expect(world.shelf.readList(VelaStore.Key.networkConfig).isEmpty, "a wrong-chain RPC was saved")
    }

    // MARK: - Adding and removing

    /// The scan path's add carries `chain_id` and `now_iso`: the core looks
    /// the chain up (and here finds nothing).
    @Test func addingByChainIdAsksForTheChain() async {
        let world = await world()
        world.settings.addByChainId(7_777_777)
        await settle { world.settings.networkAdmin?.wizard.error != nil }
        #expect(world.asked.types.contains("fetch_chain_info"), "the add never reached the core")
        #expect(world.settings.networkAdmin?.wizard.error == .notFound(chainId: 7_777_777))
    }

    /// The bin's "yes" removes the custom network, from the list and the disk.
    @Test func aCustomNetworkIsRemoved() async {
        let world = await world(seed: { shelf in
            shelf.writeList(VelaStore.Key.customNetworks, [[
                "id": "custom-7777777", "displayName": "Zora", "chainId": 7_777_777,
                "iconLabel": "Z", "iconColor": "#fff", "iconBg": "#000", "logoURL": "",
                "isL2": true, "rpcURL": "https://rpc.zora.energy", "explorerURL": "",
                "bundlerURL": "", "nativeSymbol": "ETH", "addedAt": "2026-09-22T00:00:00Z",
            ]])
        })
        #expect(row(world, chainId: 7_777_777)?.isCustom == true)

        world.settings.deleteNetwork(id: "custom-7777777")
        #expect(row(world, chainId: 7_777_777) == nil)
        await settle { world.shelf.readList(VelaStore.Key.customNetworks).isEmpty }
        #expect(world.shelf.readList(VelaStore.Key.customNetworks).isEmpty)
    }

    /// "Re-check with this RPC" checks the chain AGAIN, through the RPC typed
    /// — it used to hand over the RPC and nothing else, so the link took the
    /// tap and the verdict never moved.
    @Test func recheckRunsTheCheckAgainWithTheTypedRpc() async {
        let mine = "https://my-zora-rpc.example.org"
        let world = await world(
            // The listed RPC is silent; the person's own answers as Zora.
            reported: { url in url == mine ? 7_777_777 : nil },
            chains: [7_777_777: [
                "chain_id": 7_777_777, "name": "Zora", "short_name": "zora",
                "native_currency_name": "Ether", "native_currency_symbol": "ETH",
                "native_currency_decimals": 18,
                "rpc": ["https://rpc.zora.energy"], "explorers": ["https://explorer.zora.energy"],
                "testnet": false,
            ]]
        )
        world.settings.selectChain(7_777_777)
        await settle { world.settings.networkAdmin?.wizard.compat != nil }
        // Nothing answered, so nothing was learned: unverified, not a verdict.
        #expect(world.settings.networkAdmin?.wizard.compat?.rpcFailure != nil)
        #expect(!world.asked.urls.contains(mine))

        world.settings.recheck(customRpc: mine)
        await settle { world.settings.networkAdmin?.wizard.compat?.rpcFailure == nil
            && world.settings.networkAdmin?.wizard.compat != nil }

        #expect(world.settings.networkAdmin?.wizard.customRpc == mine)
        #expect(world.asked.urls.contains(mine), "the typed RPC was never asked")
        // A verdict now — reached through the RPC the person typed.
        #expect(world.settings.networkAdmin?.wizard.compat?.rpcFailure == nil)
        #expect(world.settings.networkAdmin?.wizard.compat?.bestRpcUrl == mine)
    }

    /// Opening the wizard clears what the last visit found.
    @Test func openingTheWizardResetsIt() async {
        let world = await world()
        world.settings.search("zora")
        #expect(world.settings.networkAdmin?.wizard.query == "zora")
        world.settings.resetWizard()
        #expect(world.settings.networkAdmin?.wizard.query == "")
        #expect(world.settings.networkAdmin?.wizard.phase == .idle)
    }
}
