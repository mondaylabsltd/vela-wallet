//
//  NetworkAdminExecutorTests.swift
//  VelaWalletTests
//
//  Sixteen operations, four storage keys, two alphabets — and one key with two
//  writers, which is the test in here that matters most.
//
//  Nothing that reaches the network is tested here. What is tested is
//  everything that decides what the network is *asked*, and everything that
//  happens to the answer.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct NetworkAdminExecutorTests {

    private func fresh() -> (VelaStore, AccountStore, UserDefaults, NetworkAdminExecutor) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        return (store, accounts, defaults, NetworkAdminExecutor(store: store, accounts: accounts))
    }

    private func answer(_ executor: NetworkAdminExecutor, _ operation: [String: Any]) async -> [String: Any] {
        (try? CoreJSON.object(await executor.perform(operation))) ?? [:]
    }

    private func type(of answer: [String: Any]) -> String { answer["type"] as? String ?? "" }

    // MARK: - The failure contract

    /// Every named operation answers with something the core can parse. The
    /// network ones are given unroutable https URLs so they fail fast rather
    /// than reaching anything real.
    @Test func everyOperationIsAnswered() async {
        let (_, _, _, executor) = fresh()
        #expect(
            NetworkAdminExecutor.operations.count == 18,
            "round 6: `retire_provider_keys` and `write_shown_networks` in, `write_rpc_providers` out"
        )
        for name in NetworkAdminExecutor.operations {
            let reply = await answer(executor, [
                "type": name,
                "url": "https://127.0.0.1:1/never",
                "base_url": "https://127.0.0.1:1",
                "address": "0xabc",
                "chain_id": 100,
                "field": "passkey_index",
                "ms": 0,
                "networks": [], "configs": [], "keys": [:], "endpoints": [:],
            ])
            #expect(!type(of: reply).isEmpty, "no answer for `\(name)`")
        }
    }

    @Test func anUnknownOperationStillAnswers() async {
        let (_, _, _, executor) = fresh()
        let reply = await answer(executor, ["type": "invented_by_a_later_spec"])
        #expect(type(of: reply) == "store_loaded")
    }

    /// A probe that cannot reach anything answers **unknown**, not zero. A
    /// `reported_chain_id` of 0 would be a chain id, and the core compares it.
    @Test func anUnreachableProbeReportsUnknownRatherThanZero() async {
        let (_, _, _, executor) = fresh()
        let reply = await answer(executor, ["type": "probe_rpc", "url": "https://127.0.0.1:1/never"])
        #expect(type(of: reply) == "probed")
        #expect(reply["reported_chain_id"] is NSNull)
    }

    /// A plaintext RPC is refused before a request is made. A wallet that would
    /// read balances over http is a wallet anyone on the network can lie to.
    @Test func aPlaintextRpcIsRefusedWithoutAsking() async {
        let (_, _, _, executor) = fresh()
        let reply = await answer(executor, ["type": "probe_rpc", "url": "http://example.com"])
        #expect(reply["reported_chain_id"] is NSNull)
    }

    /// An unreachable rates endpoint is the core's modelled failure, never an
    /// empty rate set — the two look identical on screen and mean opposite
    /// things about whether the endpoint works.
    @Test func anUnreachableRatesEndpointFailsRatherThanReportingNoRates() async {
        let (_, _, _, executor) = fresh()
        let reply = await answer(executor, ["type": "fetch_fiat_rates", "url": "https://x.test"])
        #expect(type(of: reply) == "fiat_rates")
        #expect((reply["body"] as? [String: Any])?["type"] as? String == "failed")
    }

    /// The COUNT is the shell's — it is a question about the two shapes a
    /// swappable provider may send. Whether zero rates means "unusable" is the
    /// core's, and nothing here answers it.
    @Test func bothRatesShapesAreCounted() {
        let array = NetworkAdminExecutor.fiatRatesBody(CoreHTTP.Probe(
            body: [["quote": "EUR", "rate": 0.92], ["quote": "GBP", "rate": 0.79]],
            status: 200, latencyMs: 12
        ))
        #expect(array["type"] as? String == "rates")
        #expect(array["rate_count"] as? Int == 2)

        let object = NetworkAdminExecutor.fiatRatesBody(CoreHTTP.Probe(
            body: ["rates": ["EUR": 0.92, "GBP": 0.79, "JPY": 150]], status: 200, latencyMs: 12
        ))
        #expect(object["rate_count"] as? Int == 3)

        // A 200 carrying something else is zero rates, and the core decides
        // what that means. A 500 is an http_error, which it reads differently.
        #expect(NetworkAdminExecutor.fiatRatesBody(
            CoreHTTP.Probe(body: "<html>", status: 200, latencyMs: 1)
        )["rate_count"] as? Int == 0)
        #expect(NetworkAdminExecutor.fiatRatesBody(
            CoreHTTP.Probe(body: nil, status: 500, latencyMs: 1)
        )["type"] as? String == "http_error")
    }

    // MARK: - Storage

    @Test func anAbsentStoreLoadsAsNothingConfigured() async {
        let (_, _, _, executor) = fresh()
        let reply = await answer(executor, ["type": "read_store"])
        #expect(type(of: reply) == "store_loaded")
        #expect((reply["custom_networks"] as? [Any])?.isEmpty == true)
        // Absent endpoint fields stay ABSENT (null), so the core applies its
        // own defaults rather than taking an empty string for a choice.
        let endpoints = reply["endpoints"] as? [String: Any] ?? [:]
        #expect(endpoints["passkey_index_url"] is NSNull)
    }

    /// The stored shape is a contract with three other clients, and the
    /// capitalised initialisms are part of it: `rpcURL`, not `rpcUrl`.
    @Test func aCustomNetworkRoundTripsThroughTheStoredShape() async {
        let (_, _, defaults, executor) = fresh()
        _ = await executor.perform([
            "type": "write_custom_networks",
            "networks": [[
                "id": "xlayer", "display_name": "X Layer", "chain_id": 196,
                "icon_label": "X", "icon_color": "#fff", "icon_bg": "#000",
                "logo_url": "https://logo.test/x.png", "is_l2": true,
                "rpc_url": "https://rpc.test", "explorer_url": "https://scan.test",
                "bundler_url": "https://bundler.test", "native_symbol": "OKB",
                "added_at_iso": "2026-09-05T00:00:00Z",
            ]],
        ])

        let raw = defaults.string(forKey: VelaStore.Key.customNetworks) ?? ""
        let stored = ((try? JSONSerialization.jsonObject(with: Data(raw.utf8))) as? [[String: Any]])?.first
        #expect(stored?["rpcURL"] as? String == "https://rpc.test", "stored: \(raw)")
        #expect(stored?["explorerURL"] as? String == "https://scan.test")
        #expect(stored?["logoURL"] as? String == "https://logo.test/x.png")
        #expect(stored?["addedAt"] as? String == "2026-09-05T00:00:00Z")
        #expect(stored?["displayName"] as? String == "X Layer")

        let reply = await answer(executor, ["type": "read_store"])
        let wire = (reply["custom_networks"] as? [[String: Any]])?.first
        #expect(wire?["rpc_url"] as? String == "https://rpc.test")
        #expect(wire?["added_at_iso"] as? String == "2026-09-05T00:00:00Z")
        #expect(wire?["is_l2"] as? Bool == true)
    }

    // MARK: - Provider keys retired into overrides (round 6)

    /// The configs are written first, THEN the keys go: both said, as they
    /// happened. The configs land in the stored camelCase shape.
    @Test func retiringProviderKeysWritesTheConfigsThenRemovesTheKeys() async {
        let (store, _, defaults, executor) = fresh()
        store.writeObject(VelaStore.Key.rpcProviders, ["alchemy": "abc"])
        let reply = await answer(executor, [
            "type": "retire_provider_keys",
            "configs": [[
                "chain_id": 1, "rpc_url": "https://eth-mainnet.g.alchemy.com/v2/abc",
                "explorer_url": "", "bundler_url": "", "only_mine": false,
            ]],
        ])
        #expect(type(of: reply) == "provider_keys_retired")
        #expect(reply["configs_written"] as? Bool == true)
        #expect(reply["keys_removed"] as? Bool == true)
        #expect(defaults.object(forKey: VelaStore.Key.rpcProviders) == nil)
        let config = store.readList(VelaStore.Key.networkConfig).first
        #expect(config?["chainId"] as? Int == 1)
        #expect(config?["rpcURL"] as? String == "https://eth-mainnet.g.alchemy.com/v2/abc")
    }

    /// No configs to write (`null`): the keys alone go, and the configs on
    /// disk are left exactly as they were.
    @Test func retiringWithNoConfigsOnlyRemovesTheKeys() async {
        let (store, _, defaults, executor) = fresh()
        store.writeObject(VelaStore.Key.rpcProviders, ["drpc": "k"])
        store.writeList(VelaStore.Key.networkConfig, [["chainId": 10, "rpcURL": "https://mine.test"]])
        let reply = await answer(executor, ["type": "retire_provider_keys", "configs": NSNull()])
        #expect(reply["configs_written"] as? Bool == true)
        #expect(reply["keys_removed"] as? Bool == true)
        #expect(defaults.object(forKey: VelaStore.Key.rpcProviders) == nil)
        #expect(store.readList(VelaStore.Key.networkConfig).first?["rpcURL"] as? String == "https://mine.test")
    }

    /// The answer says what happened, in the core's two flags.
    @Test func theRetireAnswerCarriesBothFlags() throws {
        let answer = try CoreJSON.object(
            NetworkAdminExecutor.providerKeysRetired(configsWritten: false, keysRemoved: false)
        )
        #expect(answer["type"] as? String == "provider_keys_retired")
        #expect(answer["configs_written"] as? Bool == false)
        #expect(answer["keys_removed"] as? Bool == false)
    }

    /// `write_rpc_providers` is gone from the core (round 6): this executor
    /// no longer lists it, and nothing writes the provider-keys shelf.
    @Test func theStaleProviderWriteIsNotAnOperation() {
        #expect(!NetworkAdminExecutor.operations.contains("write_rpc_providers"))
        #expect(NetworkAdminExecutor.operations.contains("retire_provider_keys"))
        #expect(NetworkAdminExecutor.operations.contains("write_shown_networks"))
    }

    // MARK: - Only use my node, and a custom network's space

    /// `onlyMine` round-trips — written only when on — and `isTestnet` is
    /// kept: a config or a network rewritten here no longer loses either.
    @Test func onlyMineAndTestnetSurviveARewrite() async {
        let (store, _, _, executor) = fresh()
        _ = await executor.perform([
            "type": "write_network_configs",
            "configs": [
                ["chain_id": 1, "rpc_url": "https://mine.test", "explorer_url": "", "bundler_url": "", "only_mine": true],
                ["chain_id": 10, "rpc_url": "", "explorer_url": "", "bundler_url": "", "only_mine": false],
            ],
        ])
        let stored = store.readList(VelaStore.Key.networkConfig)
        #expect(stored[0]["onlyMine"] as? Bool == true)
        #expect(stored[1]["onlyMine"] == nil, "off is absent, so the bytes of every other config are unchanged")

        _ = await executor.perform([
            "type": "write_custom_networks",
            "networks": [[
                "id": "n", "display_name": "T", "chain_id": 77, "icon_label": "", "icon_color": "",
                "icon_bg": "", "logo_url": "", "is_l2": false, "rpc_url": "https://t.test",
                "explorer_url": "", "bundler_url": "", "native_symbol": "T", "added_at_iso": "",
                "is_testnet": true,
            ]],
        ])
        #expect(store.readList(VelaStore.Key.customNetworks).first?["isTestnet"] as? Bool == true)

        let reply = await answer(executor, ["type": "read_store"])
        let configs = reply["network_configs"] as? [[String: Any]] ?? []
        #expect(configs.first?["only_mine"] as? Bool == true)
        #expect(configs.last?["only_mine"] as? Bool == false)
        #expect((reply["custom_networks"] as? [[String: Any]])?.first?["is_testnet"] as? Bool == true)
        #expect(reply["multisig_service"] as? Bool == false, "iOS keeps no multisig accounts")
    }

    // MARK: - Shown networks (owner decision 2026-10-29)

    /// A device with no record is settled ONCE by the core, from what it
    /// holds, and the record is stored before it is used; a later load reads
    /// the stored one as it is.
    @Test func aFreshDeviceIsSettledOnceAndTheRecordStored() async {
        let (store, _, _, executor) = fresh()
        #expect(store.rawValue(VelaStore.Key.shownNetworks) == nil)
        let first = await answer(executor, ["type": "read_store"])
        let shown = first["shown_networks"] as? [String: Any]
        #expect(shown?["version"] as? Int == 1)
        #expect(shown?["shown"] is [Any])
        let stored = store.readObject(VelaStore.Key.shownNetworks)
        #expect(stored["version"] as? Int == 1, "the settled record is stored before it is used")

        store.writeObject(VelaStore.Key.shownNetworks, ["version": 1, "shown": [988]])
        let second = await answer(executor, ["type": "read_store"])
        #expect((second["shown_networks"] as? [String: Any])?["shown"] as? [Int] == [988])
    }

    /// `write_shown_networks` stores the record as it came, and tells the app.
    @Test func writingShownNetworksStoresTheRecord() async {
        let (store, _, _, executor) = fresh()
        var told = 0
        executor.onShownNetworksWritten = { told += 1 }
        let reply = await answer(executor, [
            "type": "write_shown_networks", "record": ["version": 1, "shown": [4326, 988]],
        ])
        #expect(type(of: reply) == "written")
        #expect(store.readObject(VelaStore.Key.shownNetworks)["shown"] as? [Int] == [4326, 988])
        #expect(told == 1)
    }

    /// The evidence is read from the device's own stores, in the core's shape.
    @Test func theShownNetworksEvidenceIsReadFromTheStores() {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        store.writeList(VelaStore.Key.transactionHistory, [["chainId": 4326], ["chainId": 0]])
        store.writeList(VelaStore.Key.customTokens, [["chainId": 988]])
        store.writeList(VelaStore.Key.customNetworks, [["chainId": 777]])
        store.writeList(VelaStore.Key.networkConfig, [["chainId": 98866, "rpcURL": "https://p.test"]])
        let evidence = NetworkAdminExecutor.networkUse(store: store, heldChainIds: [1_440_000])
        #expect(evidence["activity_chain_ids"] as? [Int] == [4326])
        #expect(evidence["chosen_chain_ids"] as? [Int] == [988])
        #expect(evidence["added_chain_ids"] as? [Int] == [777])
        #expect(evidence["balance_chain_ids"] as? [Int] == [1_440_000])
        #expect(evidence["multisig_chain_ids"] as? [Int] == [])
        #expect((evidence["overrides"] as? [[String: Any]])?.first?["chain_id"] as? Int == 98866)
    }

    /// A network kept only by the person's own activity on it is settled
    /// shown — the core's ruling, reached through this executor's evidence.
    @Test func activityOnAnOptInNetworkKeepsItShown() async {
        let (store, _, _, executor) = fresh()
        // A network shown only once the person shows it: hidden by a record
        // that shows none of them (with NO record, every one is shown).
        let hiddenBefore = shownNetworksHidden(stored: #"{"version":1,"shown":[]}"#)
        guard let chain = hiddenBefore.first.map(Int.init) else {
            Issue.record("the core ships no opt-in network to test against")
            return
        }
        store.writeList(VelaStore.Key.transactionHistory, [["chainId": chain]])
        let reply = await answer(executor, ["type": "read_store"])
        let shown = (reply["shown_networks"] as? [String: Any])?["shown"] as? [Int] ?? []
        #expect(shown.contains(chain), "activity on \(chain) must keep it shown")
    }

    // MARK: - The two-writer trap

    /// **The regression this cut exists to prevent.**
    ///
    /// `vela.serviceEndpoints` is written by onboarding (the passkey-index
    /// override) and by `network_admin` (all four endpoints). Two independent
    /// writers on one key means the person's custom index endpoint disappears
    /// the first time they open 设置 → 端点 — silently, because both writes
    /// succeed. Routing the executor through `AccountStore` is what prevents it.
    @Test func aSavedPasskeyIndexOverrideSurvivesAnEndpointsWrite() async {
        let (_, accounts, _, executor) = fresh()
        await accounts.saveRegistryURL("https://my-own-index.example")

        _ = await executor.perform([
            "type": "write_service_endpoints",
            "endpoints": [
                "ethereum_data_url": "https://data.test",
                "passkey_index_url": "https://my-own-index.example",
                "bundler_service_url": "https://bundler.test",
                "fiat_rates_url": "https://rates.test",
            ],
        ])

        #expect(await accounts.loadRegistryURL() == "https://my-own-index.example")

        let reply = await answer(executor, ["type": "read_store"])
        let endpoints = reply["endpoints"] as? [String: Any] ?? [:]
        #expect(endpoints["passkey_index_url"] as? String == "https://my-own-index.example")
        #expect(endpoints["ethereum_data_url"] as? String == "https://data.test")
    }

    /// The reverse direction: onboarding writing its one field must not erase
    /// the three the settings screen wrote.
    @Test func anOnboardingWriteDoesNotEraseTheOtherThreeEndpoints() async {
        let (_, accounts, _, executor) = fresh()
        _ = await executor.perform([
            "type": "write_service_endpoints",
            "endpoints": [
                "ethereum_data_url": "https://data.test",
                "passkey_index_url": "https://index.test",
                "bundler_service_url": "https://bundler.test",
                "fiat_rates_url": "https://rates.test",
            ],
        ])

        await accounts.saveRegistryURL("https://switched.example")

        let reply = await answer(executor, ["type": "read_store"])
        let endpoints = reply["endpoints"] as? [String: Any] ?? [:]
        #expect(endpoints["passkey_index_url"] as? String == "https://switched.example")
        #expect(endpoints["bundler_service_url"] as? String == "https://bundler.test",
                "an onboarding write erased a settings endpoint")
    }

    // MARK: - Codecs

    /// **The live defect, pinned.**
    ///
    /// The production chain index contains ids above `u32::MAX` (7078815900 was
    /// the first one found). The core's `chain_id` is a `u32`, and serde refuses
    /// the WHOLE `search_index` result over one bad row — so the wizard sat on
    /// 搜索中 forever with no error anywhere, because the answer never reached
    /// the machine. Dropping the row is what keeps one unrepresentable id from
    /// costing every other chain.
    @Test func aChainIdTooLargeForTheCoreIsDroppedRatherThanPoisoningTheIndex() {
        let good = NetworkAdminExecutor.searchEntryToWire([
            "chainId": 100, "name": "Gnosis", "shortName": "gno",
            "nativeCurrencySymbol": "XDAI",
        ])
        #expect(good?["chain_id"] as? Int == 100)

        for bad in [7_078_815_900, -1] {
            let row = NetworkAdminExecutor.searchEntryToWire(["chainId": bad, "name": "Nope"])
            #expect(row == nil, "chain id \(bad) survived the u32 guard")
        }
        #expect(NetworkAdminExecutor.searchEntryToWire(["name": "no id at all"]) == nil)
    }

    /// `eth_chainId` returns hex. Zero, junk and a non-string are all "no
    /// answer", never a chain id.
    @Test func chainIdParsingRefusesEverythingThatIsNotOne() {
        #expect(NetworkAdminExecutor.parseChainId("0x64") == 100)
        #expect(NetworkAdminExecutor.parseChainId("64") == 100)
        #expect(NetworkAdminExecutor.parseChainId("0x0") == nil)
        #expect(NetworkAdminExecutor.parseChainId("nonsense") == nil)
        #expect(NetworkAdminExecutor.parseChainId(100) == nil)
        #expect(NetworkAdminExecutor.parseChainId(nil) == nil)
    }
}
