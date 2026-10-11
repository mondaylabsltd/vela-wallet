//
//  SiteLockTests.swift
//  VelaWalletTests
//
//  No padlock for an http page (device walk 2026-10-11: a padlock beside
//  `127.0.0.1:8137`, in the address bar and on the sheet that asks to
//  connect).
//
//  The mark is the core's `OriginLock` — `secure` for https and nothing else,
//  `local` for http on this device or the person's own network, `insecure`
//  for public http — and one view draws it (`SiteLockMark`). Pinned here:
//  the rule as this shell asks it; the address bar's mark, from the bar's
//  own committed address; the tab's and the consent's through the real
//  browser machine; and what each of the four surfaces actually puts in
//  front of a person, read from the hosted views' trees.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

private let loopback = "http://127.0.0.1:8137"
private let secureSite = "https://app.uniswap.org"
private let publicHttp = "http://example.com"
private let me = "0x1111111111111111111111111111111111111111"

@MainActor
@Suite(.serialized, .hangLimit)
struct SiteLockTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    // MARK: - The rule, as this shell asks it

    @Test func onlyHttpsIsThePadlock() {
        #expect(OriginLock.of(origin: secureSite) == .secure)
        #expect(OriginLock.of(origin: loopback) == .local)
        #expect(OriginLock.of(origin: "http://localhost:3000") == .local)
        #expect(OriginLock.of(origin: "http://192.168.1.20:8080") == .local)
        #expect(OriginLock.of(origin: publicHttp) == .insecure)
        #expect(OriginLock.of(origin: "") == .insecure, "no origin is nothing to vouch for")
        #expect(OriginLock.of(origin: nil) == .insecure)
        #expect(OriginLock(wire: "secure") == .secure && OriginLock(wire: "local") == .local)
        #expect(OriginLock(wire: "closed") == nil && OriginLock(wire: nil) == nil)

        // The padlock glyph is the secure mark's and nobody else's.
        #expect(SiteLockMark.glyph(.secure) == .lock)
        #expect(SiteLockMark.glyph(.local) != .lock && SiteLockMark.glyph(.local) != .lockOpen)
        #expect(SiteLockMark.glyph(.insecure) == .lockOpen)
        #expect(Set([OriginLock.secure, .local, .insecure].map { SiteLockMark.id("explore", $0) }).count == 3)

        // The words a local page's mark is called, from the corpus — and
        // never "secure".
        #expect(en.t("connect.browser.a11yLocal") == "Local page — not encrypted")
        #expect(en.t("explore.localSite") == "Local page")
        for language in Loc.supported {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let said = loc.t("connect.browser.a11yLocal")
            #expect(said != "connect.browser.a11yLocal" && !said.isEmpty, "\(language) has no word for a local page")
            #expect(said != loc.t("explore.secureSite"))
        }
    }

    // MARK: - The address bar: the bar's own committed address

    @Test func theAddressBarsMarkIsItsOwnAddress() {
        func bar(shown: String?, pending: String? = nil, failed: String? = nil) -> OriginLock? {
            ExploreScreen.barMark(browserAddressBar(shown: shown, pending: pending, failed: failed))
        }
        #expect(bar(shown: "\(loopback)/") == .local, "a padlock beside 127.0.0.1:8137")
        #expect(bar(shown: "http://localhost:5173/app") == .local)
        #expect(bar(shown: "http://192.168.1.20:8080/") == .local)
        #expect(bar(shown: "\(secureSite)/swap") == .secure)
        #expect(bar(shown: "\(publicHttp)/") == .insecure)
        // No mark where nothing from that host is on screen (082 RE1).
        #expect(bar(shown: nil, pending: "\(secureSite)/") == nil)
        #expect(bar(shown: "\(secureSite)/", failed: "\(secureSite)/x") == nil)
        #expect(bar(shown: nil) == nil)
        // A load that has not committed lends nothing to the page shown —
        // in either direction.
        #expect(bar(shown: "\(loopback)/", pending: "\(secureSite)/") == .local)
        #expect(bar(shown: "\(secureSite)/", pending: "\(loopback)/") == .secure)
        // A core that one day answers `local` itself is taken at its word.
        #expect(ExploreScreen.barMark(BrowserAddressBar(url: "\(loopback)/", host: "127.0.0.1:8137", lock: "local")) == .local)
        #expect(ExploreScreen.barMark(BrowserAddressBar(url: "\(publicHttp)/", host: "example.com", lock: "open")) == .insecure)
        #expect(ExploreScreen.barMark(BrowserAddressBar(url: "", host: "", lock: "none")) == nil)
    }

    // MARK: - The tab and the consent, through the real machine

    @Test func aLoopbackPageMayUseTheWalletAndWearsNoPadlock() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: loopback)
        await h.hello("t2", doc: "e1", origin: secureSite)

        let local = try #require(h.browser.dbr.tab("t1"))
        #expect(local.secure, "http on loopback may use the wallet")
        #expect(local.lock == "local", "the core's word for it")
        #expect(local.mark == .local)
        let https = try #require(h.browser.dbr.tab("t2"))
        #expect(https.secure && https.mark == .secure)

        // The page asks to connect: the sheet's mark is the ASKING origin's.
        await h.ask("t1", doc: "d1", origin: loopback, id: "c", method: "eth_requestAccounts")
        await h.until { h.browser.dbr.consent != nil }
        let consent = try #require(h.browser.dbr.consent)
        #expect(consent.lock == "local" && consent.mark == .local)
        let asking = ExploreLive.connectionModel(
            dbr: h.browser.dbr, tab: https, engine: nil, identity: (name: "Me", address: me), loc: zh
        )
        #expect(asking.consent != nil)
        #expect(asking.lock == .local, "the sheet asking for 127.0.0.1:8137 wears \(asking.lock)")

        // Connected: the panel's mark is the tab's.
        h.browser.consentApproved()
        await h.until { h.answer("t1", id: "c") != nil }
        await h.settle()
        let connected = try #require(h.browser.dbr.tab("t1"))
        let panel = ExploreLive.connectionModel(
            dbr: h.browser.dbr, tab: connected, engine: nil, identity: (name: "Me", address: me), loc: zh
        )
        #expect(panel.connected && panel.lock == .local)
        #expect(ExploreLive.connectionModel(
            dbr: h.browser.dbr, tab: https, engine: nil, identity: (name: "Me", address: me), loc: zh
        ).lock == .secure)
        // The chrome's model too.
        #expect(ExploreLive.browserModel(
            explore: .empty, tab: connected, engine: nil, identity: (name: "Me", address: me)
        ).lock == .local)
    }

    /// A view from a core built before the field: the mark is the same
    /// rule, asked of the origin — never the old `secure` flag.
    @Test func aViewWithoutTheFieldAsksTheOriginRule() {
        let old = DbrTabViewWire(tab: "t1", origin: loopback, connectedAddress: nil, chainId: 1,
                                 secure: true, crashed: false)
        #expect(old.lock == nil && old.mark == .local, "secure: true drew a padlock")
        let asked = DbrConsentViewWire(tab: "t1", origin: loopback, methods: ["eth_requestAccounts"],
                                       address: me, chainId: 1)
        #expect(asked.mark == .local)
        #expect(DbrTabViewWire(tab: "t2", origin: secureSite, connectedAddress: nil, chainId: 1,
                               secure: true, crashed: false).mark == .secure)
    }

    // MARK: - What a person is shown, on each surface

    private func ids(_ view: some View) async throws -> HostedScreen.Seen {
        let hosted = HostedScreen(view.themed(.light), size: CGSize(width: 390, height: 420))
        defer { hosted.close() }
        return try await hosted.read()
    }

    @Test(arguments: [OriginLock.secure, .local, .insecure])
    func eachSurfaceDrawsTheMarkItIsGiven(lock: OriginLock) async throws {
        let localLabel = zh.t("connect.browser.a11yLocal")
        let insecureLabel = zh.t("connect.browser.a11yInsecure")
        let host = "127.0.0.1:8137"
        let site = ExploreLive.site(host: host, name: host, origin: loopback)

        func check(_ seen: HostedScreen.Seen, _ prefix: String) {
            // The padlock is quiet: decorative to a screen reader, so it is
            // not in the tree — and neither of the other two is, beside it.
            let local = seen.element("\(prefix).local")
            let insecure = seen.element("\(prefix).insecure")
            switch lock {
            case .secure:
                #expect(local == nil && insecure == nil, "\(prefix): https is marked \(seen.tree.map(\.id))")
            case .local:
                #expect(local?.label == localLabel, "\(prefix): a local page is called \(local?.label ?? "nothing")")
                #expect(insecure == nil, "\(prefix): a local page wears the warning")
            case .insecure:
                #expect(insecure?.label == insecureLabel)
                #expect(local == nil)
            }
            #expect(seen.element("\(prefix).lock") == nil || lock == .secure, "\(prefix): a padlock for \(lock)")
        }

        check(try await ids(AddressBarView(
            host: host, lock: lock, backLabel: "back", menuLabel: "menu",
            insecureLabel: insecureLabel, localLabel: localLabel
        )), "explore")

        check(try await ids(SiteMenuSheetView(
            site: site, statusLine: "", items: [], closeLabel: "close", lock: lock,
            insecureLabel: insecureLabel, localLabel: localLabel
        )), "explore.menu")

        // The panel, connected and asking (the consent sheet is this panel).
        let dbr = DbrViewWire(
            ready: true, consent: nil,
            tabs: [DbrTabViewWire(tab: "t1", origin: loopback, connectedAddress: me, chainId: 100,
                                  secure: true, crashed: false, lock: lock.rawValue)],
            sites: [], signing: nil, queuedSigning: 0
        )
        var panel = ExploreLive.connectionModel(
            dbr: dbr, tab: dbr.tabs.first, engine: nil, identity: (name: "Me", address: me), loc: zh
        )
        #expect(panel.lock == lock)
        check(try await ids(ConnectionPanelView(
            connection: panel, closeLabel: "close", insecureLabel: insecureLabel, localLabel: localLabel
        )), "explore.connection")
        panel.consent = (approve: "连接", reject: "拒绝")
        check(try await ids(ConnectionPanelView(
            connection: panel, closeLabel: "close", insecureLabel: insecureLabel, localLabel: localLabel
        )), "explore.connection")
    }
}
