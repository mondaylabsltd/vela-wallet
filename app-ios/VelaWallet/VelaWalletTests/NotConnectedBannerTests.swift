//
//  NotConnectedBannerTests.swift
//  VelaWalletTests
//
//  「此网站尚未连接 · personal_sign」 leaves the tab once the site connects
//  (device walk 2026-10-11: the banner stayed over a site that had just
//  been connected).
//
//  The rule is the core's — a `not_connected` failure stops counting once
//  the tab's site is connected — and this shell's half is that the line
//  under the address bar is drawn from the CURRENT view of the tab and from
//  nothing it kept (`ExploreScreen.statusLine` → `BrowserController
//  .currentTab` → `BrowserStatusLive.line`). The real `DappBrowserCore`
//  through the real controller and executor, with the harness's fake page.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

private let site = "https://arcorbs.example"
private let me = "0x1111111111111111111111111111111111111111"

@MainActor
@Suite(.hangLimit)
struct NotConnectedBannerTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    /// The line under the address bar for `tab`, as the screen builds it:
    /// from the controller's view of that tab, now.
    private func banner(_ h: BrowserHarness, _ tab: String, _ loc: Loc) -> BrowserStatusLine? {
        BrowserStatusLive.line(tabId: tab, tab: h.browser.dbr.tab(tab), reloadedTab: nil, loc: loc)
    }

    private func sign(_ h: BrowserHarness, id: String) async {
        await h.ask("t1", doc: "d1", origin: site, id: id, method: "personal_sign",
                    params: ["0x48656c6c6f2c2056656c61", me])
        await h.until { h.answer("t1", id: id) != nil }
    }

    private func connect(_ h: BrowserHarness, id: String) async {
        await h.ask("t1", doc: "d1", origin: site, id: id, method: "eth_requestAccounts")
        await h.until { h.browser.dbr.consent != nil || h.answer("t1", id: id) != nil }
        h.browser.consentApproved()
        await h.until { h.answer("t1", id: id) != nil }
        await h.settle()
    }

    /// Sign before connecting → the banner; connect → gone.
    @Test func theBannerGoesWhenTheSiteConnects() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: site)
        #expect(banner(h, "t1", zh) == nil, "a page that has asked nothing has nothing to say")

        await sign(h, id: "s1")
        #expect(h.answer("t1", id: "s1")?.errorCode != nil, "an unconnected page's signature is refused")
        #expect(h.forwards.isEmpty, "and no sheet opens for it")
        let refused = try #require(h.browser.dbr.tab("t1"))
        #expect(refused.connectedAddress == nil)
        #expect(refused.lastFailure?.reason == "not_connected")
        #expect(refused.lastFailure?.method == "personal_sign")
        #expect(refused.failedRecent == 1)
        #expect(banner(h, "t1", zh)?.text == "此网站尚未连接 · personal_sign")
        #expect(banner(h, "t1", en)?.text
                == "\(en.t("componentsUi.browserStatus.reason.notConnected")) · personal_sign")
        #expect(banner(h, "t1", zh)?.warning == true)

        // The site connects.
        await connect(h, id: "c1")
        #expect(h.answer("t1", id: "c1")?.result as? [String] == [me])
        let connected = try #require(h.browser.dbr.tab("t1"))
        #expect(connected.connectedAddress == me)
        #expect(connected.lastFailure == nil, "the tab still names a request that was only waiting for this")
        #expect(connected.failedRecent == 0)
        #expect(banner(h, "t1", zh) == nil, "「此网站尚未连接」 is still up over a connected site")
        // What the screen itself reads for the tab in front is that same view.
        #expect(h.browser.dbr.tab("t1") == connected)

        // And the same request now reaches the sheet instead of the banner.
        await h.ask("t1", doc: "d1", origin: site, id: "s2", method: "personal_sign",
                    params: ["0x48656c6c6f2c2056656c61", me])
        await h.until { !h.forwards.isEmpty }
        #expect(h.forwards.count == 1)
        #expect(banner(h, "t1", zh) == nil)
    }

    /// Only THAT trouble goes: one that has nothing to do with being
    /// connected is still the tab's latest after the site connects.
    @Test func troubleThatConnectingDoesNotCureStays() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: site)
        // A read nobody answers: every endpoint silent.
        h.readAnswer = nil
        await h.ask("t1", doc: "d1", origin: site, id: "r1", method: "eth_blockNumber")
        await h.until { h.answer("t1", id: "r1") != nil }
        let failed = try #require(h.browser.dbr.tab("t1")?.lastFailure)
        #expect(failed.reason != "not_connected")
        let before = try #require(banner(h, "t1", en))
        #expect(before.text.hasSuffix("· eth_blockNumber"))

        await connect(h, id: "c1")
        #expect(h.browser.dbr.tab("t1")?.connectedAddress == me)
        #expect(h.browser.dbr.tab("t1")?.lastFailure == failed)
        #expect(banner(h, "t1", en) == before, "connecting hid a failure it did not cure")
    }

    /// Disconnected again, the next refused request says so again: nothing
    /// was remembered as "already said".
    @Test func disconnectedAgainTheBannerCanComeBack() async throws {
        let h = BrowserHarness()
        await h.boot()
        await h.hello("t1", doc: "d1", origin: site)
        await sign(h, id: "s1")
        await connect(h, id: "c1")
        #expect(banner(h, "t1", zh) == nil)

        h.browser.revoke(origin: site)
        await h.settle()
        #expect(h.browser.dbr.tab("t1")?.connectedAddress == nil)
        await sign(h, id: "s3")
        #expect(h.browser.dbr.tab("t1")?.lastFailure?.reason == "not_connected")
        #expect(banner(h, "t1", zh)?.text == "此网站尚未连接 · personal_sign")
    }
}
