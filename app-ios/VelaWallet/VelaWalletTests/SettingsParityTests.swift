//
//  SettingsParityTests.swift
//  VelaWalletTests
//
//  The rows Settings draws say what is true (spec 072 T030, T033, T035).
//
//  Each test builds the page from the REAL `network_admin` view (or the real
//  preference), and asserts that what used to be a fixture — "Alchemy ·
//  Connected · alch_k3y…9fQ2" for everyone, "12 networks" on every phone, a
//  language row reading "System" — is now this device's own answer.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SettingsParityTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// A loaded `network_admin` view over what the store holds — the machine's
    /// own answer, not a hand-written view.
    private func view(
        providerKeys: [String: Any] = [:],
        customNetworks: [[String: Any]] = []
    ) throws -> NetViewWire {
        let core = NetworkAdminCore()
        let started = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "started"])))
        for effect in started["effects"] as? [[String: Any]] ?? [] {
            guard (effect["operation"] as? [String: Any])?["type"] as? String == "read_store",
                  let id = (effect["id"] as? NSNumber)?.uint64Value else { continue }
            _ = try core.resolveEffect(effectId: id, resultJson: CoreJSON.string([
                "type": "store_loaded",
                "custom_networks": customNetworks.map(NetworkAdminExecutor.customNetworkToWire),
                "network_configs": [],
                "endpoints": NetworkAdminExecutor.storedEndpointsToWire([:]),
                "provider_keys": NetworkAdminExecutor.providerKeysToWire(providerKeys),
            ]))
        }
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(core.view()))
    }

    private let zora: [String: Any] = [
        "id": "custom-7777777", "displayName": "Zora", "chainId": 7_777_777,
        "iconLabel": "Z", "iconColor": "#fff", "iconBg": "#000", "logoURL": "",
        "isL2": true, "rpcURL": "https://rpc.zora.energy", "explorerURL": "",
        "bundlerURL": "", "nativeSymbol": "ETH", "addedAt": "2026-09-22T00:00:00Z",
    ]

    private func live(_ view: NetViewWire) -> SettingsScreenModel {
        var model = SettingsLive.withNetworks(view, on: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        model = SettingsLive.withEndpoints(view, on: model, loc: loc)
        model = SettingsLive.withProviders(view, on: model, loc: loc)
        return model
    }

    // MARK: - The providers and endpoints pages (T030)

    /// The providers page is the saved keys — and nobody's fixture key.
    @Test func theProvidersPageIsTheSavedKeys() throws {
        let model = live(try view(providerKeys: ["alchemy": "my-alchemy-key"]))
        let cards = Dictionary(uniqueKeysWithValues: model.rpcProviders.providers.map { ($0.id, $0) })

        #expect(Set(cards.keys) == ["alchemy", "drpc", "ankr"], "the card ids are the core's provider names")
        #expect(cards["alchemy"]?.field.value == "my-alchemy-key")
        #expect(cards["alchemy"]?.badge.tone == .ok)
        // With a key, the field checks it; without one, it goes to get one.
        #expect(cards["alchemy"]?.linkUrl == nil)
        #expect(cards["alchemy"]?.field.action == loc.t(I18nKeys.SettingsUi.providerCheckKey))
        #expect(cards["drpc"]?.field.value == "")
        #expect(cards["drpc"]?.badge.tone == .neutral)
        #expect(cards["drpc"]?.linkUrl == "https://drpc.org/")
        #expect(!model.rpcProviders.providers.contains { $0.field.value.contains("alch_k3y") })
    }

    /// The endpoints page's fields are keyed by the core's own field names —
    /// the ids `endpoint_edited` carries back — with the defaults as
    /// placeholders, not the drawing's four fixed URLs and latencies.
    @Test func theEndpointsPageIsTheCoresFields() throws {
        let model = live(try view())
        #expect(model.endpoints.fields.map(\.id) == NetEndpointFieldWire.allCases.map(\.rawValue))
        for field in model.endpoints.fields {
            #expect(field.placeholder?.hasPrefix("https://") == true, "\(field.id) has no default")
            #expect(field.badge?.label != "62ms", "\(field.id) wears the fixture's latency")
        }
    }

    // MARK: - Networks (T033, T035)

    /// The advanced row counts THIS device's networks, custom ones included.
    @Test func theNetworksRowCountsTheLiveList() throws {
        let view = try view(customNetworks: [zora])
        let row = live(view).sections.flatMap(\.rows).first { $0.id == "networks" }
        #expect(row?.value == loc.t(I18nKeys.SettingsUi.networkCount,
                                    vars: ["count": String(view.networks.count)]))
        #expect(view.networks.contains { $0.chainId == 7_777_777 })
    }

    /// A custom network's page says it is custom — not "built-in · cannot be
    /// removed" — and removing it asks, naming it.
    @Test func aCustomNetworkIsNamedAsOneAndItsRemovalAsks() throws {
        let model = live(try view(customNetworks: [zora]))
        let row = try #require(model.networks.first { $0.chainId == 7_777_777 })
        #expect(row.removable)
        #expect(model.networkDetails[row.id]?.note == loc.t(I18nKeys.SettingsUi.networkCustom))

        let confirm = SettingsLive.removeNetworkConfirm(row, loc: loc)
        #expect(confirm.title == loc.t(I18nKeys.SettingsUi.networkRemoveTitle))
        #expect(confirm.note?.contains("Zora") == true)
        #expect(confirm.danger)
        // The bin's accessible name is "Remove network", never "Add network".
        #expect(loc.t(I18nKeys.SettingsUi.networkRemoveTitle) != model.addNetworkLabel)
    }

    /// A check that could not run is worded "unable to verify" with a way to
    /// run it again — never "incompatible", and never a button to a setup
    /// tool no client has a page for.
    @Test func anUnverifiableChainIsNotCalledIncompatible() throws {
        let wizard = try CoreJSON.decode(NetWizardViewWire.self, from: [
            "phase": "checked", "query": "7777777", "custom_rpc": "", "suggestions": [],
            "chain_info": [
                "chain_id": 7_777_777, "name": "Zora", "short_name": "zora", "native_name": "Ether",
                "native_symbol": "ETH", "native_decimals": 18, "rpc_url": "https://rpc.zora.energy",
                "rpc_urls": ["https://rpc.zora.energy"], "explorer_url": "", "logo_url": "",
                "is_testnet": false,
            ],
            "compat": [
                "chain_id": 7_777_777, "compatible": false, "multi_key_ready": false, "contracts": [],
                "p256_available": NSNull(), "best_rpc_url": NSNull(), "best_rpc_latency_ms": NSNull(),
                "rpc_failure": "all_probes_failed",
            ],
            "error": NSNull(), "can_add": false,
        ])
        let panel = SettingsLive.wizard(wizard, loc: loc, fallback: SettingsFixtures.build(.st10, loc: loc).addNetwork)

        #expect(panel.candidate?.badge?.label == loc.t(I18nKeys.SettingsUi.addUnableToVerify))
        #expect(panel.candidate?.badge?.label != loc.t(I18nKeys.SettingsUi.addIncompatible))
        #expect(panel.retry != nil)
        #expect(panel.recheck != nil)
        #expect(panel.primary == nil)
        #expect(panel.secondary == nil)
    }

    /// The core's `check_failed` refusal decodes. It did not, and a view
    /// carrying it stopped the whole settings screen hearing the core.
    @Test func aFailedCheckDecodes() throws {
        let error = try CoreJSON.decode(NetWizardErrorWire.self, from: ["type": "check_failed", "chain_id": 42_220])
        #expect(error == .checkFailed(chainId: 42_220))
    }

    // MARK: - Localization rows (T035)

    private func preferences() -> Preferences {
        let defaults = UserDefaults(suiteName: "vela.tests.parity.\(UUID().uuidString)")!
        let prefs = Preferences(store: VelaStore(defaults: defaults))
        prefs.boot()
        return prefs
    }

    private func languageRow(_ prefs: Preferences) -> String? {
        SettingsLive.withPreferences(prefs, on: SettingsFixtures.build(.st1, loc: loc), loc: loc)
            .sections.flatMap(\.rows).first { $0.id == "language" }?.value
    }

    /// The language row names the language — the chosen one, or the one the
    /// device resolves to — rather than "System" alone.
    @Test func theLanguageRowNamesTheLanguage() {
        let prefs = preferences()
        let following = languageRow(prefs)
        #expect(following == "English · \(loc.t(I18nKeys.SettingsUi.commonSystem))")

        prefs.setLanguage("ja")
        #expect(languageRow(prefs) == "日本語")
    }

    /// A chosen currency is named even while it cannot be priced — not
    /// reported as USD, which says the choice did not take.
    @Test func aChosenUnpricedCurrencyIsNamed() {
        #expect(SettingsLive.currencyRowValue(CurrencyViewWire(code: "CNY", rate: nil, committed: true)) == "CNY")
        #expect(SettingsLive.currencyRowValue(CurrencyViewWire(code: "USD", rate: 1, committed: false))
                .hasPrefix("USD · $"))
        #expect(SettingsLive.currencyRowValue(CurrencyViewWire(code: "EUR", rate: 0.5, committed: true))
                .hasPrefix("EUR · €"))
    }
}
