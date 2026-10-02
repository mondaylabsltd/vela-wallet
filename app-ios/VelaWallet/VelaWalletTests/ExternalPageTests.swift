//
//  ExternalPageTests.swift
//  VelaWalletTests
//
//  Spec 088 FR-004: `velawallet://open?url=…` from another app or a website.
//  The link only ASKS: the core names the host the person will see, only an
//  https page with a plain host is ever asked about, and nothing loads until
//  the answer is yes.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ExternalPageTests {

    private func browser() -> BrowserController {
        let suite = "vela.tests.external.\(UUID().uuidString)"
        UserDefaults().removePersistentDomain(forName: suite)
        return BrowserController(store: VelaStore(defaults: UserDefaults(suiteName: suite)!), now: { 0 })
    }

    @Test func aLinkFromOutsideAsksAboutTheHostAndLoadsNothing() {
        let browser = browser()
        browser.askToOpen("https://app.uniswap.org/swap?chain=base")
        #expect(browser.externalPage == .init(url: "https://app.uniswap.org/swap?chain=base", host: "app.uniswap.org"))
        #expect(browser.explore.tabs.isEmpty, "nothing loads before the person answers")
        // "No" forgets it.
        #expect(browser.answerExternal(false) == false)
        #expect(browser.externalPage == nil)
        #expect(browser.explore.tabs.isEmpty)
    }

    @Test func onlyAnHttpsPageWithAPlainHostIsAskedAbout() {
        let browser = browser()
        for url in [
            "http://app.uniswap.org",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://wallet.example@evil.example/",
            "https://",
            "app.uniswap.org",
        ] {
            browser.askToOpen(url)
            #expect(browser.externalPage == nil, "\(url)")
        }
        browser.askToOpen("https://dapp.example:8443/x")
        #expect(browser.externalPage?.host == "dapp.example:8443", "a non-default port is part of where the page comes from")
    }

    /// The tokeniser hands the page to the browser; the core's rule and the
    /// person's answer are what stand between a link and a loaded page.
    @Test func theDeepLinkCarriesThePageToTheQuestion() {
        guard case .open(let url)? = PayLink.parse("velawallet://open?url=https%3A%2F%2Fapp.example%2Fa") else {
            Issue.record("velawallet://open did not parse")
            return
        }
        let browser = browser()
        browser.askToOpen(url)
        #expect(browser.externalPage?.host == "app.example")
    }
}
