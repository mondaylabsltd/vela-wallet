//
//  AgentTests.swift
//  VelaWalletTests
//
//  The AI agent's phone half (spec 104 phase 7), with nothing faked below
//  it: the core's runner on its own thread, this app's bridge hopping to the
//  main actor, the real stores over a real `UserDefaults`. A person's tap goes
//  in through the intents and comes out as the store's own change, seen —
//  the same road an agent's request takes.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct AgentTests {

    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let bob = "0x44aaf19ce84f22101b5d6cba918b92dca5f19c21"

    /// A graph signed in as one account, its address book open.
    private func signedIn() async -> AppGraph {
        let defaults = UserDefaults(suiteName: "vela.tests.agent.\(UUID().uuidString)")!
        let shelf = VelaStore(defaults: defaults)
        shelf.writeList(VelaStore.Key.accounts, [[
            "id": "cred-1", "name": "Ann", "address": me,
            "public_key_hex": "04ab", "created_at_iso": "2026-10-04T10:00:00.000Z",
        ]])
        shelf.writeString(VelaStore.Key.activeIndex, "0")
        let graph = AppGraph(loc: Loc(overrideTag: nil, preferredLanguages: ["en"]), firstLaunch: false, defaults: defaults)
        await graph.launch()
        await Wait.until({ !graph.session.view.address.isEmpty }, orIdle: { graph.session.isIdle })
        graph.contacts.open(myAddress: graph.session.view.address)
        await Wait.until({ graph.contacts.isLoaded }, orIdle: { graph.contacts.isIdle })
        return graph
    }

    @Test func aPersonsTapGoesThroughTheIntentsAndLandsInTheStore() async throws {
        let graph = await signedIn()
        let outcome = await graph.agent.person(["type": "save_contact", "address": bob, "name": "Bob"])
        #expect(outcome.status == "completed", "\(outcome)")
        #expect(graph.contacts.contact(at: bob)?.name == "Bob")
        #expect(graph.agent.activity.isEmpty, "the person's own taps are not the agent's activity")

        // And back out by the same road.
        let deleted = await graph.agent.person(["type": "delete_contact", "address": bob])
        #expect(deleted.status == "completed", "\(deleted)")
        #expect(graph.contacts.contact(at: bob) == nil)
    }

    @Test func whatTheCoreRefusesIsNotDone() async {
        let graph = await signedIn()
        let before = graph.contacts.view?.contacts.count
        let outcome = await graph.agent.person(["type": "save_contact", "address": "0x123", "name": "Nobody"])
        #expect(outcome.status == "rejected")
        #expect(outcome.code == "invalid_input", "\(outcome)")
        #expect(graph.contacts.view?.contacts.count == before)

        // The phone shares no usage statistics: nothing to switch.
        let analytics = await graph.agent.person([
            "type": "set_preference", "preference": ["kind": "analytics", "value": true],
        ])
        #expect(analytics.status == "rejected", "\(analytics)")
    }

    @Test func aPreferenceIsStoredAndTakesEffectNow() async {
        let graph = await signedIn()
        let outcome = await graph.agent.person([
            "type": "set_preference", "preference": ["kind": "theme", "value": "dark"],
        ])
        #expect(outcome.status == "completed", "\(outcome)")
        #expect(graph.preferences.theme == .dark)
        graph.preferences.setTheme(.system)
    }

    @Test func theViewsAreTheMachinesOwnUnderTheNamesTheCoreReads() async throws {
        let graph = await signedIn()
        let views = CoreFacts(json: AgentShellBridge.views(graph))
        #expect(views["session"]["address"].string.caseInsensitiveCompare(me) == .orderedSame)
        #expect(views["contacts"]["loaded"].bool)
        #expect(!views["balance"].isNull)
        #expect(!views["feed"].isNull)
        #expect(views["preferences"]["theme"].string == "system")
        #expect(views["preferences"]["text_scale"].string == "standard")
        #expect(views["preferences"]["analytics"].raw as? Bool == false)
    }

    @Test func nobodySignedInIsOnlyTheSession() async {
        let defaults = UserDefaults(suiteName: "vela.tests.agent.\(UUID().uuidString)")!
        let graph = AppGraph(loc: Loc(overrideTag: nil, preferredLanguages: ["en"]), firstLaunch: false, defaults: defaults)
        let views = CoreFacts(json: AgentShellBridge.views(graph))
        #expect(!views["session"].isNull)
        #expect(views["contacts"].isNull, "an account's machines are not read while nobody is in")
        // And a screen nobody can be shown is refused, saying why.
        let refused = await AgentPerform(graph: graph, agent: graph.agent, page: nil)
            .perform(#"{"type":"show","screen":{"type":"home"}}"#)
        #expect(CoreFacts(json: refused)["refused"]["code"].string == "not_ready")
    }

    @Test func theSettingsAreStoredWithTheTokenInItsVault() async {
        let defaults = UserDefaults(suiteName: "vela.tests.agent.\(UUID().uuidString)")!
        let shelf = VelaStore(defaults: defaults)
        let vault = MemoryTokenVault()
        let agent = AgentStore(shelf: shelf, vault: vault)
        #expect(!agent.config.enabled, "off unless the person turns it on")
        #expect(!agent.showsCapsule)

        // Not ready (switched off), so nothing is dialled.
        agent.apply(enabled: false, url: "  wss://bridge.example.com/connect ", token: " t0k ")
        let stored = shelf.readObject(VelaStore.Key.agent)
        #expect(stored["enabled"] as? Bool == false)
        #expect(stored["url"] as? String == "wss://bridge.example.com/connect")
        #expect(stored["token"] == nil, "the token is the vault's, never the record's")
        #expect(vault.read() == "t0k")
        #expect(AgentStore(shelf: shelf, vault: vault).config == agent.config, "read back as stored")
        #expect(agent.endpoint?.hasPrefix("https://bridge.example.com/") == true, "\(agent.endpoint ?? "nil")")
        #expect(agent.link == .off)

        // An empty token is forgotten; so is any token on Erase This Device.
        agent.apply(enabled: nil, url: nil, token: "")
        #expect(vault.read() == nil)
        agent.apply(enabled: nil, url: nil, token: "again")
        agent.forgetToken()
        #expect(vault.read() == nil)
        #expect(agent.config.token.isEmpty)

        // Encrypted, or this machine.
        #expect(AgentConfig(enabled: true, url: "wss://bridge.example.com/connect", token: "t").ready)
        #expect(!AgentConfig(enabled: true, url: "ws://bridge.example.com/connect", token: "t").ready)
        #expect(AgentConfig(enabled: true, url: "ws://127.0.0.1:8787/connect", token: "t").ready)
        #expect(!AgentConfig(enabled: true, url: "wss://bridge.example.com/connect", token: " ").ready)
        agent.apply(enabled: nil, url: "ws://bridge.example.com/connect", token: nil)
        #expect(agent.urlProblem == "insecure")
    }

    @Test func aTokenAnOlderBuildKeptInTheRecordMovesToTheVault() {
        let defaults = UserDefaults(suiteName: "vela.tests.agent.\(UUID().uuidString)")!
        let shelf = VelaStore(defaults: defaults)
        shelf.writeObject(VelaStore.Key.agent, ["enabled": false, "url": "wss://bridge.example.com/connect", "token": "old"])
        let vault = MemoryTokenVault()
        let agent = AgentStore(shelf: shelf, vault: vault)
        #expect(agent.config.token == "old")
        #expect(vault.read() == "old")
        #expect(shelf.readObject(VelaStore.Key.agent)["token"] == nil, "and the record no longer holds it")
    }

    /// Only where this build may use the Keychain: a signed one. CI's unsigned
    /// simulator runs cannot (`errSecMissingEntitlement`), and say so here.
    @Test(.enabled(if: KeychainTokenVault.available, "an unsigned build has no Keychain"))
    func theKeychainKeepsTheTokenAndForgetsIt() {
        let vault = KeychainTokenVault(service: "app.getvela.VelaWallet.agent.tests", account: UUID().uuidString)
        defer { vault.delete() }
        #expect(vault.read() == nil)
        #expect(vault.write("secret-1"))
        #expect(vault.read() == "secret-1")
        #expect(vault.write("secret-2"), "a second write replaces the first")
        #expect(vault.read() == "secret-2")
        vault.delete()
        #expect(vault.read() == nil)
    }

    @Test func everyActionAnAgentMayAskForHasWords() throws {
        let loc = Loc(overrideTag: nil, preferredLanguages: ["en"])
        let catalog = try #require(try JSONSerialization.jsonObject(with: Data(intentCatalog().utf8)) as? [[String: Any]])
        #expect(!catalog.isEmpty)
        for entry in catalog where entry["person_only"] as? Bool != true {
            let kind = try #require(entry["action"] as? String)
            #expect(AgentWords.action(kind, loc: loc) != kind, "\(kind) has no words")
        }
    }
}
