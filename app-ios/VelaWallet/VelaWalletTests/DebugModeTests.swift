//
//  DebugModeTests.swift
//  VelaWalletTests
//
//  Settings' hidden debug mode (spec 091), iOS's half.
//
//  Every RULE here is the core's and is pinned by its own suite: which pages
//  are offered the wallet (`dapp_permissions::offers_wallet`), the seven taps
//  (`prefs::version_tapped`), what is stored and how it reads
//  (`prefs::DebugMode`). What is proved here is this app's wiring around them:
//  the preference is read and written in the core's spelling, a tap is handed
//  to the core and a reveal stored once, the browser core hears the mode at
//  boot and on every change, and every tab's document-start script follows it.
//

import Foundation
import Testing
import VelaCore
import WebKit
@testable import VelaWallet

private let lan = "http://192.168.1.5:3000"

@MainActor
struct DebugModeTests {

    private func fresh() -> (UserDefaults, VelaStore) {
        let defaults = UserDefaults(suiteName: "vela.tests.debug.\(UUID().uuidString)")!
        return (defaults, VelaStore(defaults: defaults))
    }

    private func booted(_ store: VelaStore) -> Preferences {
        let prefs = Preferences(store: store)
        prefs.boot()
        return prefs
    }

    // MARK: - The stored preference

    /// Absent is hidden; `off` and `on` read as written; anything this build
    /// does not write reads as hidden — and hidden is off.
    @Test func theStoredValueReadsAsTheCoreReadsIt() {
        let (_, store) = fresh()
        #expect(booted(store).debugMode == .hidden)
        for (raw, mode) in [("off", DebugMode.off), ("on", .on), ("ON", .hidden), ("true", .hidden)] {
            let (_, store) = fresh()
            store.writeString(VelaStore.Key.debugMode, raw)
            let prefs = booted(store)
            #expect(prefs.debugMode == mode, "\(raw)")
            #expect(prefs.debugMode.isOn == (mode == .on))
            #expect(prefs.debugMode.revealed == (mode != .hidden))
        }
        #expect(VelaStore.Key.debugMode == "vela.debugMode", "the core's prefs::keys::DEBUG_MODE")
    }

    /// Revealing stores the switch OFF; the switch stores `on` / `off`; a
    /// relaunch reads back what was stored. Every spelling is the core's.
    @Test func revealStoresOffAndTheSwitchStoresOnAndOff() {
        let (defaults, store) = fresh()
        let prefs = booted(store)

        prefs.revealDebugMode()
        #expect(prefs.debugMode == .off)
        #expect(defaults.string(forKey: VelaStore.Key.debugMode) == prefsDebugModeValue(on: false))
        #expect(defaults.string(forKey: VelaStore.Key.debugMode) == "off")

        prefs.setDebugMode(true)
        #expect(prefs.debugMode == .on)
        #expect(defaults.string(forKey: VelaStore.Key.debugMode) == "on")
        #expect(booted(store).debugMode == .on, "the switch stays where it was left across launches")

        // A second reveal never turns debug mode off.
        prefs.revealDebugMode()
        #expect(prefs.debugMode == .on)

        prefs.setDebugMode(false)
        #expect(prefs.debugMode == .off)
        #expect(defaults.string(forKey: VelaStore.Key.debugMode) == "off")
        #expect(booted(store).debugMode == .off, "still revealed, so it can be turned on again")
    }

    /// A Release build has no debug mode (owner, 2026-10-02): whatever a
    /// Debug build left in the store reads hidden — off — and no number of
    /// taps reveals the switch. The build fact is the only thing this app
    /// hands the core.
    @Test func aReleaseBuildHasNoDebugModeWhateverIsStored() {
        for raw in ["on", "off"] {
            let (_, store) = fresh()
            store.writeString(VelaStore.Key.debugMode, raw)
            let prefs = Preferences(store: store, developerBuild: false)
            prefs.boot()
            #expect(prefs.debugMode == .hidden, "\(raw)")
            #expect(!prefs.debugMode.isOn && !prefs.debugMode.revealed)
        }
        var counter = VersionTapCounter()
        counter.developerBuild = false
        for index in 0..<30 {
            let revealed = counter.tap(nowMs: 1_000 + Double(index) * 200, mode: .hidden)
            #expect(!revealed)
        }
        #expect(counter.taps.count == 0, "nothing is even counted")
        // This test bundle is a Debug build: the fact the app hands the core.
        #expect(DebugMode.developerBuild)
    }

    /// Erasing the device hides the switch again, and debug mode is off.
    ///
    /// The erase waits for WebKit (see `EraseDeviceTests`): the limit is for
    /// that, not for the switch.
    @Test(.timeLimit(.minutes(5)))
    func anEraseHidesTheSwitchAgain() async {
        let (_, store) = fresh()
        booted(store).setDebugMode(true)
        _ = await DeviceStorage.erase(store)
        #expect(booted(store).debugMode == .hidden)
    }

    // MARK: - Seven taps on the version

    /// Seven quick taps reveal it once; after the reveal is stored, no tap
    /// counts. The count and the rule are the core's — this app only keeps
    /// the count it is handed back.
    @Test func sevenQuickTapsRevealOnceAndAreStored() {
        let (_, store) = fresh()
        let prefs = booted(store)
        var counter = VersionTapCounter()
        var reveals = 0
        for index in 0..<20 {
            if counter.tap(nowMs: 1_000 + Double(index) * 300, mode: prefs.debugMode) {
                reveals += 1
                #expect(index == 6, "the seventh tap reveals")
                prefs.revealDebugMode()
            }
        }
        #expect(reveals == 1)
        #expect(prefs.debugMode == .off)

        // Revealed and switched on: still nothing counts.
        prefs.setDebugMode(true)
        for index in 0..<10 {
            let revealed = counter.tap(nowMs: 10_000 + Double(index) * 100, mode: prefs.debugMode)
            #expect(!revealed)
        }
    }

    /// Six quick taps and a pause: the next tap starts again at one.
    @Test func aSlowTapStartsTheCountAgain() {
        var counter = VersionTapCounter()
        for index in 0..<6 {
            let revealed = counter.tap(nowMs: Double(index) * 500, mode: .hidden)
            #expect(!revealed)
        }
        #expect(counter.taps.count == 6)
        let late = counter.tap(nowMs: 2_500 + 1_001, mode: .hidden)
        #expect(!late)
        #expect(counter.taps.count == 1)
    }

    // MARK: - About

    /// About carries the stored mode and the corpus's words for the switch, in
    /// every shipped locale.
    @Test func aboutCarriesTheModeAndItsWords() {
        let (_, store) = fresh()
        let prefs = booted(store)
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let hidden = SettingsLive.withPreferences(prefs, on: SettingsFixtures.build(.st14, loc: loc), loc: loc)
        #expect(hidden.about.debugMode.mode == .hidden)

        prefs.revealDebugMode()
        var model = SettingsLive.withPreferences(prefs, on: SettingsFixtures.build(.st14, loc: loc), loc: loc)
        model = SettingsLive.withAbout(version: "1.2.3", commit: "abc1234", networkCount: 24, on: model, loc: loc)
        #expect(model.about.debugMode.mode == .off, "the live About keeps the mode")
        #expect(model.about.debugMode.title == "Debug mode")

        for tag in Loc.supported {
            let words = SettingsFixtures.build(.st14, loc: Loc(overrideTag: tag, preferredLanguages: []))
                .about.debugMode
            for text in [words.title, words.body, words.revealedNotice] {
                #expect(!text.isEmpty && !text.contains("about."), "\(tag): \(text)")
            }
        }
    }

    // MARK: - The tab's script

    /// A tab holds exactly ONE document-start script, the core's for its
    /// mode, and swaps it whole when the mode changes.
    @Test func anEngineSwapsItsOneScript() {
        let engine = BrowserEngine(id: "debug-\(UUID().uuidString)", debugMode: false)
        defer { engine.tearDown() }
        let scripts = { engine.webView.configuration.userContentController.userScripts.map(\.source) }
        #expect(scripts() == [ProviderBridge.script(debugMode: false)])

        engine.setDebugMode(true)
        #expect(scripts() == [ProviderBridge.script(debugMode: true)])
        engine.setDebugMode(true)
        #expect(scripts() == [ProviderBridge.script(debugMode: true)], "the same mode again changes nothing")

        engine.setDebugMode(false)
        #expect(scripts() == [ProviderBridge.script(debugMode: false)])
        #expect(engine.webView.configuration.userContentController.userScripts.allSatisfy {
            $0.injectionTime == .atDocumentStart && $0.isForMainFrameOnly
        })
    }

    /// A closed tab is not handed a script again.
    @Test func aClosedTabTakesNoScript() {
        let engine = BrowserEngine(id: "debug-\(UUID().uuidString)", debugMode: true)
        engine.tearDown()
        engine.setDebugMode(false)
        #expect(engine.webView.configuration.userContentController.userScripts.isEmpty)
    }

    /// The controller: an open tab follows a change, and a new tab starts in
    /// the current mode.
    ///
    /// Each tab is a real WKWebView: on a loaded CI runner launching WebKit's
    /// processes alone passed a one-minute limit (#395, #399). The limit only
    /// stops a hang; nothing here is about time (the same reasoning as
    /// BrowserChromeTests, #382).
    @Test(.timeLimit(.minutes(5)))
    func everyTabFollowsTheMode() async throws {
        let h = BrowserHarness()
        h.browser.start()
        // Port 9 refuses at once: nothing here is about the page loading.
        h.browser.open("http://127.0.0.1:9/one")
        await Wait.until { h.browser.explore.selected.map { h.browser.engineForTesting($0.id) != nil } ?? false }
        let first = try #require(h.browser.explore.selected.flatMap { h.browser.engineForTesting($0.id) })
        #expect(!first.debugMode)

        h.browser.setDebugMode(true)
        #expect(first.debugMode)
        #expect(first.webView.configuration.userContentController.userScripts.map(\.source)
                == [ProviderBridge.script(debugMode: true)])

        h.browser.newTab()
        await Wait.until { h.browser.explore.tabs.count == 2 }
        h.browser.open("http://127.0.0.1:9/two")
        await Wait.until {
            h.browser.explore.selected.map { $0.id != first.id && h.browser.engineForTesting($0.id) != nil } ?? false
        }
        let second = try #require(h.browser.explore.selected.flatMap { h.browser.engineForTesting($0.id) })
        #expect(second !== first)
        #expect(second.debugMode, "a new tab starts with the mode in force")
        #expect(second.webView.configuration.userContentController.userScripts.map(\.source)
                == [ProviderBridge.script(debugMode: true)])

        h.browser.setDebugMode(false)
        #expect(!first.debugMode && !second.debugMode)
    }

    // MARK: - The browser core hears it

    /// Off (the default): a LAN http page that calls the bridge by hand is
    /// ignored — spec 088, unchanged.
    @Test func offALanPageIsIgnored() async {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: lan)
        await h.ask("t1", doc: "d1", origin: lan, id: "1", method: "eth_chainId")
        #expect(h.delivered.isEmpty)
    }

    /// Stated before the core boots, it is heard right behind `start`: a LAN
    /// page is answered.
    @Test func theModeSetBeforeBootIsHeardAtBoot() async {
        let h = BrowserHarness()
        h.browser.setDebugMode(true)
        await h.boot()
        await h.hello("t1", doc: "d1", origin: lan)
        await h.ask("t1", doc: "d1", origin: lan, id: "1", method: "eth_chainId")
        await h.until { h.answer("t1", id: "1") != nil }
        #expect(h.answer("t1", id: "1")?.result as? String == "0x1")
        // Public http is refused either way.
        await h.hello("t2", doc: "e1", origin: "http://dapp.example")
        await h.ask("t2", doc: "e1", origin: "http://dapp.example", id: "2", method: "eth_chainId")
        #expect(h.answers("t2").isEmpty)
    }

    /// Turned off with a LAN page using the wallet: what it had open is
    /// answered 4900 at once and its later messages are ignored. A secure
    /// page in another tab is untouched.
    @Test func turningItOffWithdrawsTheWalletAtOnce() async {
        let h = BrowserHarness()
        await h.boot()
        h.browser.setDebugMode(true)
        await h.hello("t1", doc: "d1", origin: lan)
        await h.hello("t2", doc: "e1", origin: "https://dapp.example")

        h.holdReads = true
        await h.ask("t1", doc: "d1", origin: lan, id: "a", method: "eth_blockNumber")
        await h.until { h.reads.count == 1 }

        h.browser.setDebugMode(false)
        await h.settle()
        #expect(h.answer("t1", id: "a")?.errorCode == 4900)

        h.releaseReads()
        await h.settle()
        #expect(h.answers("t1").filter { $0.id == "a" }.count == 1, "answered once, by the withdrawal")

        await h.ask("t1", doc: "d1", origin: lan, id: "b", method: "eth_chainId")
        #expect(h.answer("t1", id: "b") == nil, "the page's later messages are ignored")

        h.holdReads = false
        await h.ask("t2", doc: "e1", origin: "https://dapp.example", id: "c", method: "eth_chainId")
        await h.until { h.answer("t2", id: "c") != nil }
        #expect(h.answer("t2", id: "c")?.result as? String == "0x1", "the other tab is untouched")
    }
}
