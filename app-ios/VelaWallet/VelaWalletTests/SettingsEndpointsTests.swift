//
//  SettingsEndpointsTests.swift
//  VelaWalletTests
//
//  设置 → 服务端点, round-tripped through the real `network_admin` core.
//
//  ## Why these assert on the view and on storage, never on the JSON
//
//  The bug this file exists for (spec 081 FR-001) was an event whose payload
//  named its subject `id` where the core's `Event` names it `field`. Serde
//  refused the whole event, so `update` never ran: the page took keystrokes,
//  drew them, and saved nothing. A test that built the dictionary and checked
//  its keys would have passed on the broken build — the JSON was perfectly
//  well-formed, it was simply not this machine's. So every test below sends
//  what the drawn control sends and then asks the CORE what it now holds.
//
//  Hermetic apart from the blur path, which is the one event that writes: a
//  blur also starts a probe wave, and those four requests go to the real
//  services. Nothing is asserted about them — the assertions are the stored
//  record and the core's own model — so a machine with no network still
//  passes.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// Somewhere for a fault to land that a closure can write to.
///
/// A reference rather than a captured `var` so the test reads what the core's
/// fault channel actually reported, not a copy taken before it did.
@MainActor
private final class FaultLog {
    var messages: [String] = []
}

/// `.serialized` for the reason `NetworkAdminLiveTests` gives: these are
/// `@MainActor` and each one polls the main actor while the effect loop runs,
/// so in parallel they starve each other.
@MainActor
@Suite(.serialized)
struct SettingsEndpointsTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// One wallet's worth of storage, unshared, plus the machine over it.
    private func booted() async -> (SettingsStore, AccountStore) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let shelf = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let store = SettingsStore(store: shelf, accounts: accounts,
                                  pool: RpcPool(store: shelf, accounts: accounts))
        // `open()` alone: `endpoints_opened` would start a probe wave, and
        // nothing below asserts on a probe.
        store.open()
        await settle(until: { store.isLoaded })
        return (store, accounts)
    }

    private func settle(
        until condition: @escaping () -> Bool,
        seconds: Double = 10
    ) async {
        await settle(untilAsync: { condition() }, seconds: seconds)
    }

    /// The same wait, for a condition that has to ask an actor.
    ///
    /// Storage is one: the core renders the accepted value and hands the WRITE
    /// to the shell as an effect, so a view that already shows the new endpoint
    /// says nothing yet about what is on disk.
    private func settle(
        untilAsync condition: @escaping () async -> Bool,
        seconds: Double = 10
    ) async {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if await condition() { return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    private func endpoint(
        _ field: NetEndpointFieldWire, in store: SettingsStore
    ) -> NetEndpointViewWire? {
        store.networkAdmin?.endpoints.first { $0.field == field }
    }

    private func provider(
        _ id: NetProviderIdWire, in store: SettingsStore
    ) -> NetProviderViewWire? {
        store.networkAdmin?.providers.first { $0.provider == id }
    }

    // MARK: - The events the drawn controls raise

    /// **The regression itself.** A keystroke in the passkey-index box has to
    /// arrive as a change the core made, not as a well-formed dictionary.
    @Test func typingAnEndpointChangesWhatTheCoreHolds() async {
        let (store, _) = await booted()
        let before = endpoint(.passkeyIndex, in: store)?.value

        store.editEndpoint(id: NetEndpointFieldWire.passkeyIndex.rawValue,
                           value: "https://index.example")
        await settle(until: { self.endpoint(.passkeyIndex, in: store)?.value
            == "https://index.example" })

        #expect(endpoint(.passkeyIndex, in: store)?.value == "https://index.example",
                "the core still holds \(before ?? "nothing"); the event never reached `update`")
        // One field, and only that field: `field` is what tells them apart, and
        // an event that named the wrong one would move the wrong box.
        #expect(endpoint(.bundlerService, in: store)?.value
            == endpoint(.bundlerService, in: store)?.defaultValue)
    }

    /// Leaving the box is what writes — and what is written is where the
    /// passkey-index client reads from, which is spec 081 FR-002's link: an
    /// index recorded in 设置 is the index the wallet then talks to.
    ///
    /// The value carries a trailing newline and leading spaces on purpose. The
    /// core trims and strips them (invariant ⑥); a shell that wrote the raw
    /// field would save a URL that cannot be requested.
    @Test func leavingAnEndpointBoxRecordsItWhereTheIndexClientReadsIt() async {
        let (store, accounts) = await booted()
        let field = NetEndpointFieldWire.passkeyIndex

        store.editEndpoint(id: field.rawValue, value: "  https://stub.index.test\r\n")
        store.blurEndpoint(id: field.rawValue)
        await settle(untilAsync: {
            (await accounts.loadServiceEndpoints()["passkeyIndexURL"] as? String) != nil
        })

        #expect(endpoint(field, in: store)?.value == "https://stub.index.test",
                "the core did not clean and keep the typed endpoint")

        let stored = await accounts.loadServiceEndpoints()["passkeyIndexURL"] as? String
        #expect(stored == "https://stub.index.test",
                "nothing was written to vela.serviceEndpoints")
        // The accessor every sign-in and every name lookup goes through.
        let configured = await accounts.loadRegistryURL()
        #expect(configured == "https://stub.index.test",
                "the index client would still be pointed at Vela's own")
    }

    /// 恢复默认值 puts back the URLs the core ships, and the core's default for
    /// the passkey index is not the host the drawing used to show.
    @Test func resettingRestoresTheDefaultsTheCoreShips() async {
        let (store, _) = await booted()
        let field = NetEndpointFieldWire.passkeyIndex

        store.editEndpoint(id: field.rawValue, value: "https://mine.example")
        store.blurEndpoint(id: field.rawValue)
        await settle(until: { self.endpoint(field, in: store)?.value == "https://mine.example" })

        store.resetEndpoints()
        await settle(until: {
            guard let row = self.endpoint(field, in: store) else { return false }
            return row.value == row.defaultValue
        })

        let row = endpoint(field, in: store)
        #expect(row?.value == row?.defaultValue, "reset did not reach the core")
        #expect(row?.value.contains("p256-index-rs") == false,
                "the default is the retired index host: \(row?.value ?? "")")
    }

    /// A provider key is a different event with the same defect — `provider`,
    /// not `id` — and the same proof: the core's own card changes.
    @Test func typingAProviderKeyChangesWhatTheCoreHolds() async {
        let (store, _) = await booted()
        store.openProviders()
        await settle(until: { self.provider(.alchemy, in: store) != nil })

        store.editProviderKey(id: NetProviderIdWire.alchemy.rawValue, value: "test-key")
        await settle(until: { self.provider(.alchemy, in: store)?.key == "test-key" })

        #expect(provider(.alchemy, in: store)?.key == "test-key",
                "the provider event never reached `update`")
        #expect(provider(.drpc, in: store)?.key == "", "the wrong provider was edited")
    }

    /// The core refuses the shape iOS used to send, and accepts the one it
    /// sends now — asserted against the machine itself so the claim survives
    /// any later change to how the store builds its dictionaries.
    ///
    /// This is the test that would have caught the bug on the day it shipped.
    @Test func theCoreRefusesThePayloadShapeThisFixReplaced() async {
        let faults = FaultLog()
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let executor = NetworkAdminExecutor(store: VelaStore(defaults: defaults),
                                            accounts: AccountStore(defaults: defaults))
        let core = CoreStore<NetViewWire>(
            bridge: NetworkAdminCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onFault: { faults.messages.append(String(describing: $0)) }
        )
        core.boot(CoreJSON.string(["type": "started"]))
        await settle(until: { core.view?.loaded == true })

        // What iOS sent until 081: the drawing's slug under the wrong key.
        core.dispatch(CoreJSON.string([
            "type": "endpoint_edited", "id": "passkey", "value": "https://refused.example",
        ]))
        await settle(until: { !faults.messages.isEmpty }, seconds: 2)
        #expect(!faults.messages.isEmpty, "the core accepted an event it has no field for")
        #expect(core.view?.endpoints.contains { $0.value == "https://refused.example" } == false)

        // What it sends now.
        core.dispatch(CoreJSON.string([
            "type": "endpoint_edited", "field": "passkey_index", "value": "https://accepted.example",
        ]))
        await settle(until: { core.view?.endpoints.contains {
            $0.value == "https://accepted.example"
        } == true })
        #expect(core.view?.endpoints.contains { $0.value == "https://accepted.example" } == true)
    }

    // MARK: - What the page draws

    /// Every id the live page puts on a field is one the store can name, and
    /// naming it moves that field in the core.
    ///
    /// The two halves used to be written independently — the drawing's slugs on
    /// one side, the core's fields on the other — and nothing made them meet.
    /// This is that meeting, over all four at once.
    @Test func everyFieldTheLivePageDrawsRoundTripsToTheCore() async {
        let (store, _) = await booted()
        guard let view = store.networkAdmin else {
            Issue.record("the machine never produced a view")
            return
        }
        let page = SettingsLive.withEndpoints(
            view, on: SettingsFixtures.build(.st12, loc: loc), loc: loc
        )
        #expect(page.endpoints.fields.count == 4)

        for field in page.endpoints.fields {
            store.editEndpoint(id: field.id, value: "https://\(field.id).example")
        }
        await settle(until: {
            store.networkAdmin?.endpoints.allSatisfy {
                $0.value == "https://\($0.field.rawValue).example"
            } == true
        })

        for row in store.networkAdmin?.endpoints ?? [] {
            #expect(row.value == "https://\(row.field.rawValue).example",
                    "\(row.field.rawValue) never took the edit the page sent")
        }
    }

    /// The page shows the core's four services and nothing the drawing made up
    /// — no retired host presented as somebody's own index, and no latency
    /// badge over an endpoint that has not been probed.
    @Test func theLivePageDropsTheDrawingsHostAndItsInventedLatency() {
        let drawn = SettingsFixtures.build(.st12, loc: loc)
        let page = SettingsLive.withEndpoints(view(), on: drawn, loc: loc)
        #expect(page.endpoints.fields.map(\.id) == [
            "ethereum_data", "passkey_index", "bundler_service", "fiat_rates",
        ])
        // Every VALUE is the core's, so nothing the drawing held — a retired
        // host among it — can reach the screen.
        #expect(page.endpoints.fields.map(\.value) == view().endpoints.map(\.value))
        // Still being checked ⇒ no pill. A badge here is a claim that something
        // was measured, and the drawing's 62ms/88ms/104ms were not.
        #expect(page.endpoints.fields[0].badge == nil)
        #expect(page.endpoints.fields[1].badge?.label == "120ms")
        #expect(page.endpoints.fields[2].badge?.tone == .error)
        // The default is offered as the placeholder, never as a chosen value.
        #expect(page.endpoints.fields[3].value.isEmpty)
        #expect(page.endpoints.fields[3].placeholder == "https://rates.example")
        // The labels and the reset button stay the drawing's words.
        #expect(page.endpoints.title == drawn.endpoints.title)
        #expect(page.endpoints.reset == drawn.endpoints.reset)
        #expect(page.endpoints.fields[1].label == drawn.endpoints.fields[1].label)
    }

    /// A provider with no key is shown as having none, and the card carries the
    /// core's id so a keystroke can name it.
    @Test func theProviderCardsCarryTheCoresAnswerNotTheDrawings() {
        let drawn = SettingsFixtures.build(.st11, loc: loc)
        let page = SettingsLive.withProviders(view(), on: drawn, loc: loc)
        #expect(page.rpcProviders.providers.map(\.id) == ["alchemy", "drpc", "ankr"])
        // The mock key the drawing prefills Alchemy's box with is gone: these
        // boxes are now editable AND live, and a keystroke after a mock key
        // would have sent the mock key to be saved.
        #expect(page.rpcProviders.providers.map(\.field.value) == ["live-key", "", "ankr-key"])
        #expect(page.rpcProviders.providers[0].badge.tone == .ok)
        // `has_key` is the core's word, not `key.isEmpty`: an unfinished test
        // must not be reported as a supported-network count either.
        #expect(page.rpcProviders.providers[1].badge.tone == .neutral)
        #expect(page.rpcProviders.providers[1].support == nil)
        #expect(page.rpcProviders.providers[2].support?.isEmpty == false)
    }

    // MARK: - A view to draw from

    /// The four endpoints in one of each health, and three providers in one of
    /// each configured state.
    private func view() -> NetViewWire {
        NetViewWire(
            loaded: true,
            networks: [],
            wizard: NetWizardViewWire(
                phase: .idle, query: "", customRpc: "", suggestions: [],
                chainInfo: nil, compat: nil, error: nil, canAdd: false
            ),
            endpoints: [
                NetEndpointViewWire(field: .ethereumData, value: "https://chain.example",
                                    defaultValue: "https://chain.default", health: .checking),
                NetEndpointViewWire(field: .passkeyIndex, value: "https://index.example",
                                    defaultValue: "https://index.default",
                                    health: .ok(latencyMs: 120, rateCount: nil)),
                NetEndpointViewWire(field: .bundlerService, value: "http://relay.example",
                                    defaultValue: "https://relay.default", health: .notHttps),
                NetEndpointViewWire(field: .fiatRates, value: "",
                                    defaultValue: "https://rates.example",
                                    health: .unreachable(httpStatus: nil, latencyMs: nil)),
            ],
            providers: [
                NetProviderViewWire(provider: .alchemy, key: "live-key", hasKey: true, test: nil),
                NetProviderViewWire(
                    provider: .drpc, key: "", hasKey: false,
                    test: NetProviderTestViewWire(done: false, results: [], okCount: 0, total: 4)
                ),
                NetProviderViewWire(
                    provider: .ankr, key: "ankr-key", hasKey: true,
                    test: NetProviderTestViewWire(done: true, results: [], okCount: 3, total: 4)
                ),
            ],
            lastAddedChainId: nil
        )
    }
}
