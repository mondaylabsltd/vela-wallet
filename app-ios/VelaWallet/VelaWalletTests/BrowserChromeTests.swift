//
//  BrowserChromeTests.swift
//  VelaWalletTests
//
//  Spec 079 US5: the browser's chrome is honest and complete — a lock with no
//  words, a consent that says what it asks and connects, network rows with
//  their logo and what the account holds there. Hermetic.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct BrowserChromeTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])
    private let me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func dbr(consent: Bool, secure: Bool = true) -> DbrViewWire {
        DbrViewWire(
            ready: true,
            consent: consent
                ? DbrConsentViewWire(tab: "t1", origin: "https://app.uniswap.org",
                                     methods: ["eth_requestAccounts"], address: me, chainId: 100)
                : nil,
            tabs: [DbrTabViewWire(tab: "t1", origin: "https://app.uniswap.org",
                                  connectedAddress: consent ? nil : me, chainId: 100,
                                  secure: secure, crashed: false)],
            sites: [], signing: nil, queuedSigning: 0
        )
    }

    private func connection(consent: Bool, holdings: [Int: String] = [:]) -> ConnectionModel {
        let view = dbr(consent: consent)
        return ExploreLive.connectionModel(
            dbr: view, tab: view.tabs.first, engine: nil, identity: (name: "Me", address: me),
            chainIds: [1, 100, 8453], holdings: holdings, loc: loc
        )
    }

    /// The consent names what it asks — "连接到 {host}" — and its approve word
    /// is every client's "连接", no longer "批准".
    @Test func theConsentSaysWhatItAsksAndConnects() {
        let asking = connection(consent: true)
        #expect(asking.title == loc.t("connect.browser.title", vars: ["host": "app.uniswap.org"]))
        #expect(asking.title.contains("app.uniswap.org"))
        #expect(asking.consent?.approve == loc.t("connect.browser.connect"))
        #expect(asking.consent?.approve != loc.t("connect.dapp.approve"))
        #expect(asking.statusLine.isEmpty, "no safety words while it asks")

        let connected = connection(consent: false)
        #expect(connected.consent == nil)
        #expect(connected.statusLine == loc.t("explore.connectedTag"), "已连接 is a fact, and it stays")
    }

    /// Every network row leads with its logo and shows what the account holds
    /// there, from the home screen's figures — nothing where it is not known.
    @Test func networkRowsCarryTheirLogoAndTheAccountsBalance() {
        let model = connection(consent: false, holdings: [100: "¥11.91", 8453: "¥2.78"])
        #expect(model.networkLogoUrl == Marks.chainLogoURL(100))
        #expect(model.networks.map(\.id) == [1, 100, 8453])
        #expect(model.networks.allSatisfy { $0.logoUrl == Marks.chainLogoURL($0.id) })
        #expect(model.networks.first { $0.id == 100 }?.amount == "¥11.91")
        #expect(model.networks.first { $0.id == 1 }?.amount == nil, "unknown is blank, never ¥0.00")
    }

    private func token(_ chain: Int, _ balance: String, price: Double?, spam: Bool = false) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chain, symbol: "X", name: "X", balance: balance, decimals: 18,
                         tokenAddress: nil, priceUsd: price, spam: spam)
    }

    private func balance(_ tokens: [BalanceTokenWire], failed: [Int] = [], hidden: Bool = false) -> BalanceViewWire {
        BalanceViewWire(
            address: me, displayTotalUsd: 1, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: hidden, refreshing: false, lastRefreshedAtMs: nil, tokens: tokens,
            unpricedTokens: [], failedChainIds: failed, rateLimitedChainIds: [], bannerChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    /// The holdings are the home screen's own figures, per network, in the
    /// display currency — and a failed chain, spam, an unpriced token, dust
    /// or a hidden balance says nothing rather than a zero.
    @Test func theHoldingsAreTheHomeScreensFiguresOrNothing() {
        let display = WalletLive.Display(code: "USD", rate: 1)
        let figures = WalletLive.networkHoldings(balance([
            token(100, "2", price: 1),
            token(100, "1.5", price: 2),
            token(100, "9999", price: 1, spam: true),
            token(1, "1", price: nil),
            token(8453, "0.001", price: 1),
            token(42161, "5", price: 1),
        ], failed: [42161]), display: display)
        #expect(figures[100]?.hasSuffix(Formats.number(5, minimumFractionDigits: 2, maximumFractionDigits: 2)) == true)
        #expect(figures[1] == nil, "unpriced")
        #expect(figures[8453] == nil, "under half a cent")
        #expect(figures[42161] == nil, "a chain whose read failed")
        #expect(WalletLive.networkHoldings(balance([token(100, "2", price: 1)], hidden: true), display: display).isEmpty,
                "a hidden balance stays hidden here too")
        #expect(WalletLive.networkHoldings(nil, display: display).isEmpty)
    }

    /// Spec 079 US6: a tab's card is the page as it was last seen; a start
    /// page keeps its drawing; a tab never photographed keeps the stand-in.
    @Test func eachTabsCardIsItsOwnPage() {
        let explore = ExploreViewWire(
            favorites: [], groups: [],
            tabs: [
                ExploreTabWire(id: "a", url: "https://app.uniswap.org/", title: "Uniswap", host: "app.uniswap.org"),
                ExploreTabWire(id: "b", url: "https://bscscan.com/", title: "BscScan", host: "bscscan.com"),
                ExploreTabWire(id: "c", url: nil, title: "", host: ""),
            ],
            selectedTab: "a", favoritesHidden: false, recentHidden: false,
            favoritesFull: false, tabsFull: false, ready: true
        )
        let shot = UIImage()
        let tabs = ExploreLive.tabs(explore: explore, snapshot: { $0 == "a" || $0 == "c" ? shot : nil }, loc: loc)
        #expect(tabs[0].snapshot === shot)
        #expect(tabs[1].snapshot == nil, "not photographed yet: the stand-in")
        #expect(tabs[2].snapshot == nil, "a start page keeps its drawing")
        #expect(tabs[0].site?.letter == "U")
    }

    /// The http lock is its own glyph — lucide `lock-open` — not the warning
    /// triangle, and the site menu carries no status words at all.
    @Test func anHttpPageGetsAnOpenLockAndNoWords() {
        #expect(LucideGlyph.lockOpen.svg.contains("9.9-1"), "lucide lock-open's shackle")
        #expect(LucideGlyph.lockOpen.svg != LucideGlyph.lock.svg)
        let model = ExploreLive.home(
            explore: .empty, history: .empty, dbr: dbr(consent: false, secure: false), engine: nil,
            identity: (name: "Me", address: me), loc: loc
        )
        if case .siteMenu(_, let statusLine, _) = model.menus.siteMenu {
            #expect(statusLine.isEmpty)
        } else {
            Issue.record("the site menu is a site menu")
        }
    }
}
