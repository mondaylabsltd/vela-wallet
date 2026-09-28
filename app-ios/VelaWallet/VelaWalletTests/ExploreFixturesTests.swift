//
//  ExploreFixturesTests.swift
//  VelaWalletTests
//
//  Spec 022 gates for the explore layer.
//
//  The failure this file exists for is shared with Android and unique to the
//  native clients: `Loc.t()` returns the KEY when a lookup misses (the
//  documented failure model), so a typo ships as "explore.startTitle" rendered
//  on screen. Nothing else catches that — not the compiler, not a preview.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ExploreFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// Every string a state carries, flattened — the echo check's input.
    private func strings(_ state: ExploreStateId) -> [String] {
        let m = ExploreFixtures.buildMobileState(state, loc: loc)
        var out = [
            m.title, m.searchPlaceholder, m.scanLabel,
            m.tabsScreen.title, m.tabsScreen.done, m.tabsScreen.newTab,
            m.tabsScreen.closeAll, m.tabsScreen.close,
            m.nav.wallet, m.nav.contacts, m.nav.explore, m.nav.settings,
            m.menus.connection.title, m.menus.connection.statusLine,
            m.menus.connection.switchLabel, m.menus.connection.networkLabel,
            m.menus.connection.explainer, m.menus.connection.disconnect,
            m.menus.connection.footnote,
        ]
        if let empty = m.empty { out += [empty.title, empty.caption, empty.cta] }
        if let favorites = m.favorites { out += [favorites.title, favorites.action] }
        out += m.groups.map(\.title)
        // The site menu's status line is empty by ruling (spec 079: the lock
        // alone says https), so only its items are words to check.
        if case .siteMenu(_, let statusLine, let items) = m.menus.siteMenu {
            out += (statusLine.isEmpty ? [] : [statusLine]) + items.map(\.label)
        }
        if case .groupManage(let title, let rows, let newGroup) = m.menus.groupManage {
            out += [title, newGroup] + rows.map(\.title) + rows.compactMap(\.meta)
        }
        return out
    }

    @Test func noStringEchoesItsCorpusKey() {
        for state in ExploreStateId.allCases {
            for value in strings(state) {
                #expect(!value.hasPrefix("explore."), "\(value) in \(state) is an unresolved key")
                #expect(!value.hasPrefix("componentsUi."), "\(value) in \(state) is unresolved")
                #expect(!value.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    @Test func noTemplateIsLeftUnfilled() {
        for state in ExploreStateId.allCases {
            for value in strings(state) {
                #expect(!value.contains("{{"), "\(value) in \(state) still carries a {{var}}")
            }
        }
    }

    @Test func e1IsTheEmptyStartPage() {
        let e1 = ExploreFixtures.buildMobileState(.e1, loc: loc)
        #expect(e1.empty != nil)
        #expect(e1.favorites == nil)
        #expect(e1.groups.isEmpty)
        #expect(e1.tabCountLabel == nil)
    }

    @Test func e2CarriesEightTilesAndThreeGroups() {
        let e2 = ExploreFixtures.buildMobileState(.e2, loc: loc)
        #expect(e2.favorites?.tiles.count == 8)
        if case .add = e2.favorites?.tiles.last { } else { Issue.record("last tile is not `add`") }
        #expect(e2.groups.map(\.id) == ["recent", "trading", "prediction"])
        // Custom group names are what a person typed — never translated.
        #expect(Array(e2.groups.dropFirst()).map(\.title) == ["交易", "预测市场"])
    }

    @Test func sheetsOpenOnlyWhereTheMockOpensThem() {
        #expect(ExploreFixtures.buildMobileState(.e3, loc: loc).sheet?.id == "group-manage")
        #expect(ExploreFixtures.buildMobileState(.e6, loc: loc).sheet?.id == "site-menu")
        #expect(ExploreFixtures.buildMobileState(.e7, loc: loc).sheet?.id == "connection")
        for state: ExploreStateId in [.e1, .e2, .e4, .e5] {
            #expect(ExploreFixtures.buildMobileState(state, loc: loc).sheet == nil)
        }
    }

    @Test func viewsMatchTheirMocks() {
        for state: ExploreStateId in [.e4, .e6, .e7] {
            #expect(ExploreFixtures.buildMobileState(state, loc: loc).view == .browsing)
        }
        #expect(ExploreFixtures.buildMobileState(.e5, loc: loc).view == .tabs)
        #expect(ExploreFixtures.buildMobileState(.e2, loc: loc).view == .start)
    }

    @Test func e5SelectsTheTabItWasOpenedFrom() {
        let tabs = ExploreFixtures.buildMobileState(.e5, loc: loc).tabs
        #expect(tabs.first(where: \.selected)?.id == "uniswap")
    }

    @Test func systemGroupsCanBeHiddenButNeverDeleted() {
        guard case .groupManage(_, let rows, _) =
            ExploreFixtures.buildMobileState(.e3, loc: loc).menus.groupManage
        else { Issue.record("E3 has no group manager"); return }
        #expect(rows.filter(\.system).map(\.id) == ["favorites", "recent"])
    }

    @Test func theStandInPageIsTheSitesContent() {
        let page = ExploreFixtures.buildMobileState(.e4, loc: loc).browser.page
        #expect(page.title == "兑换")
        #expect(page.fields.map(\.symbol) == ["ETH", "USDC"])
    }

    @Test func identityIsSwappedInWholesale() {
        let model = ExploreFixtures.buildMobileState(.e7, loc: loc)
            .withIdentity(name: "kimik3", address: "0x1234567890abcdef1234567890abcdefAABBCCDD")
        #expect(model.browser.account.name == "kimik3")
        #expect(model.menus.connection.account.address == "0x1234…CCDD")
    }
}
