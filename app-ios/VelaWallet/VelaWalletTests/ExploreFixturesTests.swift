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
        if let resume = m.resume { out += [resume.title, resume.action] }
        out += m.groups.map(\.title)
        // The site menu's status line is empty by ruling (spec 079: the lock
        // alone says https), so only its items are words to check.
        if case .siteMenu(_, let statusLine, let items) = m.menus.siteMenu {
            out += (statusLine.isEmpty ? [] : [statusLine]) + items.map(\.label)
        }
        if case .groupManage(let title, let rows) = m.menus.groupManage {
            out += [title] + rows.map(\.title) + rows.compactMap(\.meta)
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

    /// E1 is a first visit: the start page is the only tab, so the home has
    /// no resume section — and, since DESIGN N, no tab count anywhere on it.
    @Test func e1IsTheEmptyStartPage() {
        let e1 = ExploreFixtures.buildMobileState(.e1, loc: loc)
        #expect(e1.empty != nil)
        #expect(e1.favorites == nil)
        #expect(e1.groups.isEmpty)
        #expect(e1.resume == nil, "no tab has a page: no section, no header, no empty words")
        #expect(e1.tabs.map(\.id) == ["start"])
        #expect(e1.browser.tabCount == 1)
    }

    /// DESIGN N, board E2: under the search field, the tabs left open — three
    /// rows (the core's RESUME_SHOWN), most recent first, start pages never
    /// among them — under a header that counts EVERY tab, the switcher's
    /// number, and an action that opens the switcher.
    @Test func e2ResumesTheOpenTabs() throws {
        let e2 = ExploreFixtures.buildMobileState(.e2, loc: loc)
        let resume = try #require(e2.resume, "E2 has open tabs with pages: the section shows")
        #expect(e2.tabs.count == 4)
        #expect(resume.title == loc.t("explore.openTabs", vars: ["n": "4"]))
        #expect(resume.title == "已打开 4 个标签页")
        #expect(resume.action == loc.t("explore.tabs"))
        #expect(resume.tabs.map(\.id) == ["uniswap", "polymarket", "aave"])
        #expect(resume.tabs.count <= ExploreFixtures.resumeShown)
        #expect(resume.tabs.map(\.site.host) == ["app.uniswap.org", "polymarket.com", "app.aave.com"])
        // Every row is a tab of the strip that has a page.
        let pages = Set(e2.tabs.filter { !$0.startPage }.map(\.id))
        #expect(resume.tabs.allSatisfy { pages.contains($0.id) })
        // The rows read as the recents do: the title over the host.
        let first = SiteRowView.lines(resume.tabs[0].site)
        #expect(first.name == "Uniswap")
        #expect(first.second == "app.uniswap.org")
    }

    /// The bar's box counts the strip, start pages included: the number the
    /// header says in words and the switcher holds.
    @Test func theBrowsingCountIsTheWholeStrip() {
        for state: ExploreStateId in [.e4, .e6, .e7] {
            let model = ExploreFixtures.buildMobileState(state, loc: loc)
            #expect(model.browser.tabCount == model.tabs.count)
        }
    }

    /// DESIGN N, board E6: Forward leads the site menu (it moved there from
    /// the old toolbar), greyed while there is nothing ahead — never hidden,
    /// so no row under the thumb moves — and close page ends it.
    @Test func theSiteMenuLeadsWithForward() {
        guard case .siteMenu(_, _, let items) = ExploreFixtures.buildMobileState(.e6, loc: loc).sheet
        else { Issue.record("E6 has no site menu"); return }
        #expect(items.map(\.id) == ["forward", "refresh", "share", "copy", "favorite", "system", "disconnect", "close"])
        #expect(items.first?.label == loc.t("explore.forward"))
        #expect(items.first?.icon == "arrowRight")
        #expect(items.first?.disabled == true, "E4's page has nothing ahead")
        #expect(items.dropFirst().allSatisfy { !$0.disabled })
        #expect(ExploreFixtures.siteMenuItems(loc, canForward: true).first?.disabled == false)
        // The live menu says the same, with the page's own forward.
        let live = ExploreLive.siteMenuItems(bookmarked: false, connected: true, canForward: true, loc: loc)
        #expect(live.first?.id == "forward")
        #expect(live.first?.disabled == false)
        #expect(ExploreLive.siteMenuItems(bookmarked: false, connected: false, loc: loc).first?.disabled == true)
        // Every icon the menu names is one the corpus draws.
        for item in items { #expect(LucideGlyph(rawValue: item.icon) != nil, "\(item.icon)") }
    }

    /// Issue #465: no custom groups — under Favorites there is Recent alone.
    @Test func e2CarriesEightTilesAndRecentAlone() {
        let e2 = ExploreFixtures.buildMobileState(.e2, loc: loc)
        #expect(e2.favorites?.tiles.count == 8)
        if case .add = e2.favorites?.tiles.last { } else { Issue.record("last tile is not `add`") }
        #expect(e2.groups.map(\.id) == ["recent"])
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

    /// Issue #465: Manage groups is exactly Favorites and Recent dApps, an
    /// eye each — no "System" word, nothing new, nothing to delete.
    @Test func manageGroupsIsTheTwoSectionsAlone() {
        guard case .groupManage(_, let rows) =
            ExploreFixtures.buildMobileState(.e3, loc: loc).menus.groupManage
        else { Issue.record("E3 has no group manager"); return }
        #expect(rows.map(\.id) == ["favorites", "recent"])
        #expect(rows.map(\.title) == [loc.t("explore.favorites"), loc.t("explore.recent")])
        #expect(rows[1].meta == nil, "Recent carries no second word")
    }

    /// The Favorites row counts its sites in each language's own plural
    /// forms — "1 site", never "1 sites"; Russian's few and many.
    @Test func theFavoritesRowCountsItsSitesByThePluralKey() throws {
        func meta(_ count: Int, _ tag: String) throws -> String? {
            let favorites = (0..<count).map { index in
                ExploreSiteWire(origin: "https://s\(index).example", url: "https://s\(index).example/",
                                host: "s\(index).example", name: "S\(index)", renamed: false, addedMs: 0)
            }
            let view = ExploreViewWire(
                favorites: favorites, tabs: [], selectedTab: nil, favoritesHidden: false,
                recentHidden: false, favoritesFull: false, tabsFull: false, ready: true
            )
            guard case .groupManage(_, let rows) = ExploreLive.groupManage(
                explore: view, loc: Loc(overrideTag: tag, preferredLanguages: [])
            ) else { throw Missing() }
            return rows.first?.meta
        }
        #expect(try meta(1, "en") == "1 site")
        #expect(try meta(2, "en") == "2 sites")
        #expect(try meta(3, "ru") == "3 сайта")
        #expect(try meta(5, "ru") == "5 сайтов")
        #expect(try meta(1, "zh") == "1 个网站")
        for tag in ["en", "ru", "zh"] {
            #expect(try meta(1, tag)?.contains("{{") == false, "\(tag): the count was filled")
            #expect(try meta(1, tag)?.contains("siteCount") == false, "\(tag): the key resolved")
        }
        guard case .groupManage(_, let drawn) = ExploreFixtures.buildMobileState(.e3, loc: loc).menus.groupManage
        else { throw Missing() }
        #expect(drawn.first?.meta == "8 个网站")
    }

    private struct Missing: Error {}

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
