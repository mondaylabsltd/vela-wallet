/**
 * Explore message manifest (spec 022 §5).
 *
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like `wallet/messages.ts`.
 * `nav.*` re-resolves the spec-015 tab-bar keys so one messages object feeds
 * the whole screen, chrome included.
 */

export interface ExploreMessages {
	title: string;
	searchPlaceholder: string;
	scan: string;
	startTitle: string;
	startHint: string;
	startCta: string;
	favorites: string;
	recent: string;
	edit: string;
	done: string;
	add: string;
	clear: string;
	manageGroups: string;
	rename: string;
	hide: string;
	show: string;
	delete: string;
	openInNewTab: string;
	removeFromFavorites: string;
	/** Template — '{{n}} sites'. */
	siteCount: string;
	tabs: string;
	newTab: string;
	startPage: string;
	closeAllTabs: string;
	closeTab: string;
	/** Template — '{{n}} tabs open'. */
	openTabs: string;
	addToFavorites: string;
	addedToFavorites: string;
	share: string;
	copyLink: string;
	refresh: string;
	openInSystemBrowser: string;
	disconnect: string;
	closePage: string;
	secureSite: string;
	connectedTag: string;
	connectionTitle: string;
	switchAccount: string;
	network: string;
	connectionExplainer: string;
	autoRequestHint: string;
	back: string;
	forward: string;
	reload: string;
	siteMenu: string;
	account: string;
	addressBar: string;
	close: string;
	nav: { wallet: string; contacts: string; explore: string; settings: string };
	/** Reused from the wallet vocabulary — the third column's close button. */
	closeLabel: string;
}
