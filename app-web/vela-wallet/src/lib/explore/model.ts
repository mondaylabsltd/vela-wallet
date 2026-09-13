/**
 * Explore view models (spec 022, data-model.md §2).
 *
 * Components consume ONLY these display-ready shapes — no service types, no
 * fetching, no URL parsing (the rule spec 015 set and 018 kept). When a real
 * browser engine and a real dApp registry arrive, they replace the fixture
 * layer that builds these and nothing else.
 */

export type ExploreStateId =
	| 'e1'
	| 'e2'
	| 'e3'
	| 'e4'
	| 'e5'
	| 'e6'
	| 'e7'
	/** Spec 032 phase 40: the menu on a row in Recent. */
	| 'e8';
export type ExploreDesktopStateId = 'de1' | 'de2' | 'de3' | 'de4';

/** A site as the browser home draws it — a lettermark, never a fetched icon. */
export interface SiteModel {
	id: string;
	name: string;
	host: string;
	/** Single grapheme drawn in the avatar. */
	letter: string;
	/** Brand colour behind the letter; the tile tints it down itself. */
	tint: string;
	/** Row-only second line (a group's blurb), absent in the tile grid. */
	subtitle?: string;
	/** Row-only trailing text — "刚刚", "昨天". Fixture content. */
	meta?: string;
}

/** The favourites grid mixes sites with the trailing "add" affordance. */
export type TileModel = { kind: 'site'; site: SiteModel } | { kind: 'add'; label: string };

/** `favorites` and `recent` are system groups: hideable, never deletable. */
export type GroupKind = 'favorites' | 'recent' | 'custom';

/** The trailing affordance on a group's header row. */
export type GroupAction = 'edit' | 'clear' | 'menu';

export interface GroupModel {
	id: string;
	title: string;
	kind: GroupKind;
	action?: GroupAction;
	sites: SiteModel[];
	hidden: boolean;
}

export interface TabModel {
	id: string;
	title: string;
	site?: SiteModel;
	selected: boolean;
	/** The start page's own tab — drawn with the sail, not a favicon. */
	startPage: boolean;
}

/**
 * The page inside the browser.
 *
 * FIXTURE CONTENT, not app chrome: it stands in for whatever site is open, so
 * its strings are the mock's own and are never translated — the same call
 * spec 015 made for 大表哥. A real WebView replaces this component wholesale.
 */
export interface DemoPageModel {
	title: string;
	fields: { value: string; symbol: string }[];
	cta: string;
	/** Accent colour of the site's own button — the site's brand, not ours. */
	ctaTint: string;
}

export interface BrowserModel {
	url: string;
	host: string;
	secure: boolean;
	connected: boolean;
	canBack: boolean;
	canForward: boolean;
	bookmarked: boolean;
	account: { name: string; identiconSvg: string };
	tabCount: number;
	page: DemoPageModel;
}

export interface MenuItemModel {
	id: string;
	/** A `UtilityIconId`; kept loose so the model file has no icon import. */
	icon: string;
	label: string;
	danger?: boolean;
}

export interface GroupManageRow {
	id: string;
	title: string;
	/** "8 个网站" / "已隐藏" — resolved by the fixture layer. */
	meta?: string;
	system: boolean;
	hidden: boolean;
}

export interface ConnectionModel {
	title: string;
	site: SiteModel;
	statusLine: string;
	account: { name: string; address: string; identiconSvg: string };
	switchLabel: string;
	networkLabel: string;
	network: { name: string; dot: string };
	explainer: string;
	disconnect: string;
	footnote: string;
}

export interface GroupManageSheet {
	kind: 'group-manage';
	title: string;
	rows: GroupManageRow[];
	newGroup: string;
}

export interface SiteMenuSheet {
	kind: 'site-menu';
	site: SiteModel;
	statusLine: string;
	items: MenuItemModel[];
}

export interface ConnectionSheet {
	kind: 'connection';
	connection: ConnectionModel;
}

export type ExploreSheet = GroupManageSheet | SiteMenuSheet | ConnectionSheet;

/** Which surface the phone screen is showing (SPEC 动效 · 探索 手机). */
export type ExploreView = 'start' | 'browsing' | 'tabs';

export interface ExploreHomeModel {
	state: ExploreStateId;
	view: ExploreView;
	title: string;
	tabCountLabel?: string;
	searchPlaceholder: string;
	scanLabel: string;
	empty?: { title: string; caption: string; cta: string };
	favorites?: { title: string; action: string; tiles: TileModel[] };
	groups: GroupModel[];
	browser: BrowserModel;
	tabs: TabModel[];
	tabsScreen: { title: string; done: string; newTab: string; closeAll: string; close: string };
	/** Which sheet the state opens with, if any (E3/E6/E7). */
	sheet?: ExploreSheet;
	/**
	 * The sheets browsing can raise on demand. They are part of the model
	 * rather than built at the tap, so a screen never has to invent copy at
	 * interaction time — the same reason the wallet's chain sheet is a fixture.
	 */
	menus: {
		groupManage: GroupManageSheet;
		siteMenu: SiteMenuSheet;
		/** The row menu in Recent — where `delete_origin` is reached from. */
		recentMenu: SiteMenuSheet;
		connection: ConnectionSheet;
	};
	/** The four-tab bar, reused from the wallet vocabulary. */
	navLabels: { wallet: string; contacts: string; explore: string; settings: string };
}

export interface ExploreDesktopModel {
	state: ExploreDesktopStateId;
	tabStrip: { tabs: TabModel[]; newTabLabel: string; newTabTitle: string };
	toolbar: {
		back: string;
		forward: string;
		reload: string;
		searchPlaceholder: string;
		bookmark: string;
		menu: string;
	};
	browser: BrowserModel;
	/** The start page's own content, shown when the selected tab is the start page. */
	start: {
		empty?: { title: string; caption: string; cta: string };
		favorites?: { title: string; action: string; tiles: TileModel[] };
		groups: GroupModel[];
	};
	/** DE2's right-click menu on a favourite tile, pinned at a fixture point. */
	contextMenu?: { items: MenuItemModel[]; x: number; y: number };
	/** The third column's two tenants. Which one is up is screen state. */
	connection: ConnectionModel;
	initialPanel: 'none' | 'connection' | 'signing';
	closeLabel: string;
}
