/**
 * Canonical explore fixtures (spec 022, data-model.md §2 — the single canon
 * all four platforms port). Content is verbatim from `design/explore/`;
 * builders merge it with resolved messages into display-ready view models.
 * Pure data + assembly: no fetching, no URL parsing, no business state.
 */
import { IDENTITY } from '$lib/wallet/fixtures';
import type { SidebarModel } from '$lib/wallet/model';
import type { ExploreMessages } from './messages';
import type {
	BrowserModel,
	ConnectionSheet,
	GroupManageSheet,
	SiteMenuSheet,
	ExploreDesktopModel,
	ExploreDesktopStateId,
	ExploreHomeModel,
	ExploreStateId,
	GroupModel,
	MenuItemModel,
	SiteModel,
	TabModel,
	TileModel
} from './model';

type Identicon = (seed: string) => string;

/** `{{var}}` interpolation for the handful of templated explore strings. */
export function fill(template: string, vars: Record<string, string>): string {
	return Object.entries(vars).reduce(
		(out, [name, value]) => out.replaceAll(`{{${name}}}`, value),
		template
	);
}

/**
 * The wallet's own sidebar with Explore selected. Explore does not grow a
 * second rail: DE1–DE4 draw the spec-015 sidebar unchanged, because a browser
 * that hid the wallet's navigation would be an app inside an app.
 */
export function exploreSidebar(sidebar: SidebarModel): SidebarModel {
	return {
		...sidebar,
		nav: sidebar.nav.map((item) => ({ ...item, selected: item.id === 'explore' }))
	};
}

/** Every phone state, in mock order (the gallery's inventory). */
export const MOBILE_STATES = ['e1', 'e2', 'e3', 'e4', 'e5', 'e6', 'e7', 'e8'] as const;

/** Every desktop state. DE4 is the third column carrying a signing request. */
export const DESKTOP_STATES = ['de1', 'de2', 'de3', 'de4'] as const;

// --- Canon ----------------------------------------------------------------

/** Brand colours: site content, not theme tokens (the wallet's chain-colour rule). */
export const SITES = {
	uniswap: {
		id: 'uniswap',
		name: 'Uniswap',
		host: 'app.uniswap.org',
		letter: 'U',
		tint: '#FF007A'
	},
	aave: { id: 'aave', name: 'Aave', host: 'app.aave.com', letter: 'A', tint: '#8B6DFF' },
	pancake: {
		id: 'pancake',
		name: 'PancakeSwap',
		host: 'pancakeswap.finance',
		letter: 'P',
		tint: '#1FC7D4'
	},
	polymarket: {
		id: 'polymarket',
		name: 'Polymarket',
		host: 'polymarket.com',
		letter: 'P',
		tint: '#4267F4'
	},
	opensea: { id: 'opensea', name: 'OpenSea', host: 'opensea.io', letter: 'O', tint: '#2081E2' },
	lido: { id: 'lido', name: 'Lido', host: 'stake.lido.fi', letter: 'L', tint: '#F0616D' },
	ens: { id: 'ens', name: 'ENS', host: 'app.ens.domains', letter: 'E', tint: '#5284FF' },
	hyperliquid: {
		id: 'hyperliquid',
		name: 'Hyperliquid',
		host: 'app.hyperliquid.xyz',
		letter: 'H',
		tint: '#50D2C1'
	},
	curve: { id: 'curve', name: 'Curve', host: 'curve.fi', letter: 'C', tint: '#7B7BE8' },
	limitless: {
		id: 'limitless',
		name: 'Limitless',
		host: 'limitless.exchange',
		letter: 'L',
		tint: '#8B6DFF'
	}
} satisfies Record<string, SiteModel>;

/** The eight favourites, in mock order (E2/DE2). */
const FAVORITES: SiteModel[] = [
	SITES.uniswap,
	SITES.aave,
	SITES.pancake,
	SITES.polymarket,
	SITES.opensea,
	SITES.lido,
	SITES.ens
];

const withMeta = (site: SiteModel, meta: string, subtitle?: string): SiteModel => ({
	...site,
	meta,
	subtitle: subtitle ?? site.host
});

/** Recent rows. The phone shows one; the desktop's wider grid shows four. */
const RECENT_PHONE: SiteModel[] = [withMeta(SITES.hyperliquid, '刚刚')];
const RECENT_DESKTOP: SiteModel[] = [
	withMeta(SITES.hyperliquid, '刚刚'),
	withMeta(SITES.polymarket, '昨天'),
	withMeta(SITES.uniswap, ''),
	withMeta(SITES.opensea, '昨天')
];

/**
 * Custom groups. Titles and blurbs are what the person typed, so they are
 * fixture content — verbatim from the mock, never translated (spec 015 rule).
 */
const CUSTOM_GROUPS: GroupModel[] = [
	{
		id: 'trading',
		title: '交易',
		kind: 'custom',
		action: 'menu',
		hidden: false,
		sites: [
			{ ...SITES.curve, subtitle: '稳定币兑换' },
			{ ...SITES.hyperliquid, subtitle: '永续合约交易' }
		]
	},
	{
		id: 'prediction',
		title: '预测市场',
		kind: 'custom',
		action: 'menu',
		// NOT pre-hidden: E2 and DE2 both draw this group. E3 shows it hidden
		// because the sheet is where hiding HAPPENS — shipping it hidden made
		// the web the only client missing a group (caught by fixtures.test.ts).
		hidden: false,
		sites: [
			{ ...SITES.polymarket, subtitle: '事件预测市场' },
			{ ...SITES.limitless, subtitle: '预测市场' }
		]
	}
];

/**
 * The page the browser is showing. Fixture content: this is a stand-in for a
 * real site, so its words are the site's, not the wallet's.
 */
const DEMO_PAGE = {
	title: '兑换',
	fields: [
		{ value: '0.5', symbol: 'ETH' },
		{ value: '1,280.42', symbol: 'USDC' }
	],
	cta: '兑换',
	ctaTint: '#FF007A'
};

export const NETWORK = { name: 'Ethereum', dot: '#627EEA' };

// --- Assembly -------------------------------------------------------------

function favoritesSection(m: ExploreMessages) {
	const tiles: TileModel[] = [
		...FAVORITES.map((site) => ({ kind: 'site' as const, site })),
		{ kind: 'add' as const, label: m.add }
	];
	return { title: m.favorites, action: m.edit, tiles };
}

function groups(m: ExploreMessages, recent: SiteModel[]): GroupModel[] {
	return [
		{
			id: 'recent',
			title: m.recent,
			kind: 'recent',
			action: 'clear',
			hidden: false,
			sites: recent
		},
		...CUSTOM_GROUPS.filter((g) => !g.hidden)
	];
}

function browser(m: ExploreMessages, identicon: Identicon, connected: boolean): BrowserModel {
	return {
		url: SITES.uniswap.host,
		host: SITES.uniswap.host,
		secure: true,
		connected,
		canBack: true,
		canForward: false,
		bookmarked: false,
		account: { name: IDENTITY.name, identiconSvg: identicon(IDENTITY.addressFull) },
		tabCount: 2,
		page: DEMO_PAGE
	};
}

function tabs(m: ExploreMessages, selected: 'uniswap' | 'polymarket' | 'start'): TabModel[] {
	return [
		{
			id: 'uniswap',
			title: SITES.uniswap.name,
			site: SITES.uniswap,
			selected: selected === 'uniswap',
			startPage: false
		},
		{
			id: 'polymarket',
			title: SITES.polymarket.name,
			site: SITES.polymarket,
			selected: selected === 'polymarket',
			startPage: false
		},
		{ id: 'start', title: m.startPage, selected: selected === 'start', startPage: true }
	];
}

/** E6's site menu, in mock order. */
function siteMenuSheet(m: ExploreMessages): SiteMenuSheet {
	return {
		kind: 'site-menu',
		site: SITES.uniswap,
		statusLine: m.secureSite,
		items: siteMenuItems(m)
	};
}

/** The same menu as a sheet, for the phone gallery (E8). */
function recentMenuSheet(m: ExploreMessages): SiteMenuSheet {
	return {
		kind: 'site-menu',
		site: SITES.polymarket,
		// A history row's second line is where it has BEEN, not whether the
		// page is secure right now — there is no page open to be secure.
		statusLine: SITES.polymarket.host,
		items: recentMenuItems(m)
	};
}

function siteMenuItems(m: ExploreMessages): MenuItemModel[] {
	return [
		{ id: 'refresh', icon: 'refresh-cw', label: m.refresh },
		{ id: 'share', icon: 'share-2', label: m.share },
		{ id: 'copy', icon: 'copy', label: m.copyLink },
		{ id: 'favorite', icon: 'star', label: m.addToFavorites },
		{ id: 'system', icon: 'external-link', label: m.openInSystemBrowser },
		{ id: 'disconnect', icon: 'power', label: m.disconnect },
		{ id: 'close', icon: 'x', label: m.closePage }
	];
}

/**
 * The menu on a row in Recent (spec 032 phase 40).
 *
 * History rows had no menu at all, so the core's `delete_origin` — one site
 * forgotten, rather than the whole list cleared — could not be reached from
 * any client. Three items in the order somebody wants them: the two harmless
 * ones, then the destructive one behind a divider.
 */
function recentMenuItems(m: ExploreMessages): MenuItemModel[] {
	return [
		{ id: 'new-tab', icon: 'external-link', label: m.openInNewTab },
		{ id: 'favorite', icon: 'star', label: m.addToFavorites },
		{ id: 'delete', icon: 'trash-2', label: m.delete, danger: true }
	];
}

/** DE2's right-click menu on a favourite tile. */
function tileMenuItems(m: ExploreMessages): MenuItemModel[] {
	return [
		{ id: 'new-tab', icon: 'external-link', label: m.openInNewTab },
		{ id: 'rename', icon: 'pencil', label: m.rename },
		{ id: 'move', icon: 'folder-plus', label: m.moveToGroup },
		{ id: 'remove', icon: 'trash-2', label: m.removeFromFavorites, danger: true }
	];
}

function connectionSheet(m: ExploreMessages, identicon: Identicon): ConnectionSheet {
	return {
		kind: 'connection' as const,
		connection: {
			title: m.connectionTitle,
			site: SITES.uniswap,
			statusLine: `${m.secureSite} · ${m.connectedTag}`,
			account: {
				name: IDENTITY.name,
				address: IDENTITY.addressDisplay,
				identiconSvg: identicon(IDENTITY.addressFull)
			},
			switchLabel: m.switchAccount,
			networkLabel: m.network,
			network: NETWORK,
			explainer: m.connectionExplainer,
			disconnect: m.disconnect,
			footnote: m.autoRequestHint
		}
	};
}

function groupManageSheet(m: ExploreMessages): GroupManageSheet {
	return {
		kind: 'group-manage' as const,
		title: m.manageGroups,
		newGroup: m.newGroup,
		rows: [
			{
				id: 'favorites',
				title: m.favorites,
				meta: fill(m.siteCount, { n: '8' }),
				system: true,
				hidden: false
			},
			{ id: 'recent', title: m.recent, meta: m.systemGroup, system: true, hidden: false },
			{
				id: 'trading',
				title: CUSTOM_GROUPS[0].title,
				meta: fill(m.siteCount, { n: '4' }),
				system: false,
				hidden: false
			},
			{
				id: 'prediction',
				title: CUSTOM_GROUPS[1].title,
				meta: fill(m.hiddenCount, { n: '2' }),
				system: false,
				hidden: true
			}
		]
	};
}

/** Every phone state (E1–E7). */
export function buildMobileState(
	state: ExploreStateId,
	m: ExploreMessages,
	identicon: Identicon
): ExploreHomeModel {
	const populated = state !== 'e1';
	const browsing = state === 'e4' || state === 'e6' || state === 'e7';
	// E5 opens the switcher FROM a page, so the page's tab is the selected one
	// — the mock's accent border is on Uniswap, not on 起始页.
	const selected = browsing || state === 'e5' ? 'uniswap' : 'start';

	const base: ExploreHomeModel = {
		state,
		view: browsing ? 'browsing' : state === 'e5' ? 'tabs' : 'start',
		title: m.title,
		tabCountLabel: populated ? '2' : undefined,
		searchPlaceholder: m.searchPlaceholder,
		scanLabel: m.scan,
		empty: populated ? undefined : { title: m.startTitle, caption: m.startHint, cta: m.startCta },
		favorites: populated ? favoritesSection(m) : undefined,
		groups: populated ? groups(m, RECENT_PHONE) : [],
		browser: browser(m, identicon, true),
		tabs: tabs(m, selected),
		tabsScreen: {
			title: m.tabs,
			done: m.done,
			newTab: m.newTab,
			closeAll: m.closeAllTabs,
			close: m.closeTab
		},
		menus: {
			groupManage: groupManageSheet(m),
			siteMenu: siteMenuSheet(m),
			recentMenu: recentMenuSheet(m),
			connection: connectionSheet(m, identicon)
		},
		navLabels: m.nav
	};

	switch (state) {
		case 'e3':
			return { ...base, sheet: base.menus.groupManage };
		case 'e6':
			return { ...base, sheet: base.menus.siteMenu };
		case 'e8':
			return { ...base, sheet: base.menus.recentMenu };
		case 'e7':
			return { ...base, sheet: base.menus.connection };
		default:
			return base;
	}
}

/** Every desktop state (DE1–DE4). DE4's third column is the signing request. */
export function buildDesktopState(
	state: ExploreDesktopStateId,
	m: ExploreMessages,
	identicon: Identicon
): ExploreDesktopModel {
	const browsing = state === 'de3' || state === 'de4';
	const populated = state !== 'de1';

	return {
		state,
		tabStrip: {
			tabs: browsing
				? tabs(m, 'uniswap').filter((t) => !t.startPage)
				: [{ id: 'start', title: m.newTab, selected: true, startPage: true }],
			newTabLabel: m.newTab,
			newTabTitle: m.newTab
		},
		toolbar: {
			back: m.back,
			forward: m.forward,
			reload: m.reload,
			searchPlaceholder: m.searchPlaceholder,
			bookmark: m.addToFavorites,
			menu: m.siteMenu
		},
		browser: browser(m, identicon, browsing),
		start: {
			empty: populated ? undefined : { title: m.startTitle, caption: m.startHint, cta: m.startCta },
			favorites: populated ? favoritesSection(m) : undefined,
			groups: populated ? groups(m, RECENT_DESKTOP) : []
		},
		// DE2 pins the menu under the PancakeSwap tile, which is where the mock
		// opened it; a real right-click passes the pointer instead.
		contextMenu: state === 'de2' ? { items: tileMenuItems(m), x: 300, y: 208 } : undefined,
		connection: connectionSheet(m, identicon).connection,
		initialPanel: state === 'de4' ? 'signing' : state === 'de3' ? 'connection' : 'none',
		closeLabel: m.closeLabel
	};
}
