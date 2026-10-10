//
//  ExploreModels.swift
//  VelaWallet
//
//  Explore view models (spec 022, data-model.md §2 — the iOS port of the web
//  reference `src/lib/explore/model.ts`). Components consume ONLY these
//  display-ready shapes: no fetching, no URL parsing, no business state. A
//  real browser engine and a dApp registry replace the fixture layer that
//  builds them and nothing else.
//

import SwiftUI

enum ExploreStateId: String, CaseIterable, Identifiable {
    case e1, e2, e3, e4, e5, e6, e7
    var id: String { rawValue }
    /// Gallery chip label — mock naming, not translatable copy.
    var label: String { rawValue.uppercased() }
}

struct SiteModel: Identifiable, Hashable {
    let id: String
    let name: String
    let host: String
    /// Single grapheme drawn in the avatar until the site's own icon lands,
    /// and when none does — the core's rule (`browserSiteLetter`).
    let letter: String
    let tint: Color
    var subtitle: String?
    /// "刚刚" / "昨天" — fixture content.
    var meta: String?
    /// Spec 079: the site's own icon, best first (https only) — the one the
    /// page named when visited, then the usual places. Empty draws the letter.
    var iconUrls: [String] = []
}

enum TileModel: Identifiable {
    case site(SiteModel)
    case add(String)

    /// The "+" tile's id. Not a site: a screen that opened it as one loaded
    /// `https://add`.
    static let addId = "add"

    var id: String {
        switch self {
        case .site(let site): site.id
        case .add: Self.addId
        }
    }
}

/// `favorites` and `recent` are the only sections (issue #465: no custom
/// groups): hideable, never deletable.
enum GroupKind { case favorites, recent }

/// The trailing affordance on a section's header row.
enum GroupAction { case edit, clear }

struct GroupModel: Identifiable {
    let id: String
    let title: String
    let kind: GroupKind
    let action: GroupAction?
    let sites: [SiteModel]
    var hidden: Bool
}

struct TabModel: Identifiable {
    let id: String
    let title: String
    let site: SiteModel?
    /// The card marked as "this tab" — the core's `browserLitTab` on the
    /// live switcher (a dApp left for the wallet waits unlit under the home).
    var selected: Bool
    /// The start page's own tab — drawn with the sail, not a favicon.
    let startPage: Bool
    /// Spec 079: the page as it was last seen (`WKWebView.takeSnapshot`), so
    /// two tabs can be told apart. `nil` draws the stand-in.
    var snapshot: UIImage?
    /// Spec 099: the card's long-press menu offers "close other tabs" and
    /// "close tabs to the right" only where the core's scope takes a tab —
    /// never with one tab, never on the last card. Asked of the core
    /// (`exploreTabsClosedBy`), never worked out here.
    var closesOthers = false
    var closesRight = false
}

/// The page inside the browser. FIXTURE CONTENT, not chrome: it stands in for
/// whatever site is open, so its words are the mock's and are never
/// translated. A real WKWebView replaces this view wholesale.
struct DemoPageModel {
    struct Field { let value: String; let symbol: String }
    let title: String
    let fields: [Field]
    let cta: String
    let ctaTint: Color
}

struct BrowserModel {
    let url: String
    let host: String
    let secure: Bool
    let connected: Bool
    /// The page has somewhere to go back to. ‹ is never disabled: with no
    /// history it returns to the Explore home (DESIGN N).
    let canBack: Bool
    /// Forward lives in the site menu, greyed while this is false.
    let canForward: Bool
    let bookmarked: Bool
    var account: (name: String, seed: String)
    /// Every open tab, start pages included — the number the switcher holds.
    let tabCount: Int
    let page: DemoPageModel
}

struct SiteMenuItem: Identifiable {
    let id: String
    /// A `LucideIcon` name; the model file stays free of icon imports.
    let icon: String
    let label: String
    var danger: Bool = false
    /// Shown but greyed and inert: a row that cannot act right now (Forward
    /// with nothing ahead). It keeps its place, so the rows under a thumb do
    /// not move from one page to the next (DESIGN N).
    var disabled: Bool = false
}

/// One row of Manage groups: Favorites or Recent dApps, with an eye (issue
/// #465 — the two sections are all there is, so nothing is deletable and
/// nothing is new).
struct GroupManageRow: Identifiable {
    let id: String
    let title: String
    /// "8 个网站" — resolved by the fixture layer. `nil` draws no second word.
    let meta: String?
    var hidden: Bool
}

struct ConnectionModel {
    let title: String
    let site: SiteModel
    let statusLine: String
    var account: (name: String, address: String, seed: String)
    let switchLabel: String
    let networkLabel: String
    let network: (name: String, dot: Color)
    let explainer: String
    let disconnect: String
    let footnote: String
    /// Whether the origin is on TLS. Drives the padlock, and nothing else may.
    var secure: Bool = true
    /// The panel's **not-yet-connected** form: a site is asking, and the two
    /// answers replace 断开连接. Same panel, because the facts a person needs
    /// in order to decide are exactly the facts they need in order to review
    /// — who is asking, which account, which network — and a second sheet
    /// would be a second chance to get one of them wrong.
    var consent: (approve: String, reject: String)?
    /// The core holds a grant for this site (spec 097 E). Not connected and
    /// not asking, the panel offers nothing that implies access: no account
    /// it sees, no explainer, no Disconnect, no "requests appear here".
    var connected = true
    /// The site the panel is about — what Disconnect and the network picker
    /// act on. `""` in the gallery.
    var origin: String = ""
    /// The site's chain, as the core keeps it.
    var chainId: Int = 0
    /// The chains the network row offers. Empty draws the row as a label.
    var networks: [NetworkChoiceModel] = []
    /// Spec 079: the site's network's logo; the dot shows until it lands.
    var networkLogoUrl: String?
}

/// One network the connection panel's picker offers.
struct NetworkChoiceModel: Identifiable, Equatable {
    let id: Int
    let name: String
    let dot: Color
    /// Spec 079 (owner: "切换网络，没有网络logo呀"): the chain's logo.
    var logoUrl: String?
    /// Spec 079 (owner: "需要能看到这个网络上的余额吧"): what the account holds
    /// on this network, in the display currency, from the home screen's own
    /// figures — `nil` when not known or empty, never a made-up zero.
    var amount: String?
}

/// Spec 100: the add-network sheet a page opened (`NetView.dapp_add`), in
/// words — `ExploreLive.addNetwork` picks Settings' own line for each part.
struct AddNetworkSheetModel {
    struct Row: Identifiable {
        var id: String { label }
        let label: String
        let value: String
    }

    /// `tab/id` — a new request is a new sheet.
    let id: String
    let title: String
    /// "{{host}} asks to add a network" — who asks, from the transport.
    let lead: String
    let site: SiteModel
    let rows: [Row]
    /// The name and coin are the site's, not Vela's catalog's.
    let fromSite: String?
    let pill: StatusPillModel?
    let checksTitle: String?
    let checks: [CheckItemModel]
    /// The sentence under the verdict.
    let note: String?
    /// "Add Network" — only where the core says it can act.
    let add: String?
    let retry: String?
    /// "Open Chain Setup Tool" and where it goes — the core's `setup_url`
    /// for this chain, so both are present or neither is.
    let setupTool: String?
    var setupUrl: String? = nil
    /// Cancel while a decision is open, Done after a verdict — either way
    /// `dapp_add_declined`.
    let dismiss: String
}

/// Which of the sheets is open.
///
/// Separate from `ExploreSheet` — its *contents* — because a screen must store
/// the identity and resolve the contents at render time. Storing the contents
/// is how a sheet ends up showing what was true when it opened.
enum ExploreSheetKind: String, Identifiable {
    // The hyphenated spellings the fixtures have used since spec 022 — this
    // type took over `ExploreSheet`'s identity and must not change it.
    case groupManage = "group-manage"
    case siteMenu = "site-menu"
    case connection = "connection"
    /// Spec 100: a page asks to add a network. Its contents are the settings
    /// machine's, drawn from `ExploreScreen.addNetwork`.
    case addNetwork = "add-network"

    var id: String { rawValue }

    func resolved(in model: ExploreHomeModel) -> ExploreSheet {
        switch self {
        case .groupManage: model.menus.groupManage
        case .siteMenu: model.menus.siteMenu
        case .connection: .connection(model.menus.connection)
        case .addNetwork: .addNetwork
        }
    }
}

enum ExploreSheet: Identifiable {
    case groupManage(title: String, rows: [GroupManageRow])
    case siteMenu(site: SiteModel, statusLine: String, items: [SiteMenuItem])
    case connection(ConnectionModel)
    case addNetwork

    var id: String { kind.rawValue }

    var kind: ExploreSheetKind {
        switch self {
        case .groupManage: .groupManage
        case .siteMenu: .siteMenu
        case .connection: .connection
        case .addNetwork: .addNetwork
        }
    }
}

/// Which surface the screen is showing (SPEC 动效 · 探索 手机).
enum ExploreView { case start, browsing, tabs }

struct TabsScreenCopy {
    let title: String
    let done: String
    let newTab: String
    let closeAll: String
    let close: String
    /// Spec 099: a card's long-press menu — close it (`close`), the others,
    /// the ones to its right, all of them (`closeAll`).
    let closeOthers: String
    let closeRight: String
}

/// One row of the home's resume section: a tab that has a page, as the core
/// hands it over in `ExploreView.resumable` — drawn as a site row (its title
/// over its host, its mark) and opened by the TAB's id.
struct ResumeTabModel: Identifiable {
    let id: String
    let site: SiteModel
}

/// The home's resume section (DESIGN N): drawn only while a tab has a page.
/// The header counts EVERY open tab — the switcher's number — and its action
/// opens the switcher; the rows are the core's, in the core's order.
///
/// One pointer, like `SettingsScreenModel`: `ExploreHomeModel` sits right at
/// the screen-model stack budget (`ScreenModelStackTests`), and every byte
/// added inline is paid in every unoptimised frame that copies it. Immutable,
/// so sharing the box is never seen.
struct ResumeSectionModel {
    private final class Box {
        let title: String
        let action: String
        let tabs: [ResumeTabModel]

        init(title: String, action: String, tabs: [ResumeTabModel]) {
            self.title = title
            self.action = action
            self.tabs = tabs
        }
    }

    private let box: Box

    init(title: String, action: String, tabs: [ResumeTabModel]) {
        box = Box(title: title, action: action, tabs: tabs)
    }

    /// `explore.openTabs_*`, the form for the number of open tabs, filled with it.
    var title: String { box.title }
    /// `explore.tabs`.
    var action: String { box.action }
    var tabs: [ResumeTabModel] { box.tabs }
}

struct ExploreHomeModel {
    let state: ExploreStateId
    let view: ExploreView
    let title: String
    let searchPlaceholder: String
    let scanLabel: String
    let empty: (title: String, caption: String, cta: String)?
    /// Under the search field; `nil` while no tab has a page. The title row
    /// carries no tab count any more: this header says it, in words, and
    /// opens the same switcher.
    let resume: ResumeSectionModel?
    let favorites: (title: String, action: String, tiles: [TileModel])?
    var groups: [GroupModel]
    var browser: BrowserModel
    let tabs: [TabModel]
    let tabsScreen: TabsScreenCopy
    /// Which sheet the state opens with, if any (E3/E6/E7).
    let sheet: ExploreSheet?
    /// The sheets browsing can raise on demand — part of the model rather than
    /// built at the tap, so a screen never invents copy at interaction time.
    var menus: (groupManage: ExploreSheet, siteMenu: ExploreSheet, connection: ConnectionModel)
    let nav: TabsModel
}
