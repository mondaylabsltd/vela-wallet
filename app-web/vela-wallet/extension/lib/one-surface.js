/**
 * One wallet per browser window, as pure rules.
 *
 * The owner's report: the wallet open in a tab AND in the side panel of the
 * same window, the two showing different things — "this is too strange". A
 * window holds ONE live wallet. A tab is the bigger, deliberate surface, so
 * where both are in one window the tab is the wallet by default and the side
 * panel steps aside: it says Vela is open in a tab, with "Go to tab" and "Use
 * here instead" (`panelStanding`), and the toolbar goes to that tab rather
 * than opening a second one (`toolbarTarget`).
 *
 * "Use here instead" closes nothing — a half-filled send is never cut off.
 * It hands the window to the other surface (`OWNER_PREFIX`), and the one
 * that had it steps aside the same way, its wallet still loaded beneath:
 * the tab then says Vela is open in the side panel (`tabStanding`).
 *
 * A surface in the middle of something never steps aside: a dApp's request
 * the panel owes, shows or is landing (spec 082 routes it there, and the
 * worker's routing is untouched), or a send or signature in flight.
 *
 * Dependency-free, with no extension or DOM API: the worker, the panel and
 * the tab all apply these, and `one-surface.test.ts` drives every branch.
 */

/**
 * `storage.session`: `<prefix><windowId>` = `'panel'` while the person has
 * chosen the side panel over that window's wallet tab. Absent: the tab is the
 * window's wallet (the default). The panel writes it on "Use here instead"
 * and drops it when it goes; the tab and the toolbar drop it to take the
 * window back.
 */
export const OWNER_PREFIX = 'vela.surface.window.';

/** The owner key of window `windowId`. */
export const ownerKey = (windowId) => `${OWNER_PREFIX}${windowId}`;

/**
 * Is `url` one of the wallet's own pages, open as a wallet — rather than a
 * request window's page, the side panel's doorway or a file the harness
 * reads? `base` is `chrome.runtime.getURL('')`.
 */
export function isWalletPageUrl(url, base) {
	if (typeof url !== 'string' || typeof base !== 'string' || !url.startsWith(base)) return false;
	let parsed;
	try {
		parsed = new URL(url);
	} catch {
		return false;
	}
	const path = parsed.pathname;
	if (!path.endsWith('.html')) return false;
	// The request window's page and its doorway (`open.html?rid=…`).
	if (path.endsWith('/request.html') || parsed.searchParams.has('rid')) return false;
	// The side panel's doorway, and the panel's own document.
	if (path === '/panel.html' || parsed.searchParams.has('panel')) return false;
	return true;
}

/**
 * The wallet tabs in window `windowId`, from `chrome.tabs.query` rows. A tab
 * that is still loading names its target in `pendingUrl`.
 */
export function walletTabsIn(tabs, windowId, base) {
	if (!Array.isArray(tabs)) return [];
	return tabs.filter(
		(tab) =>
			tab &&
			typeof tab.id === 'number' &&
			tab.windowId === windowId &&
			(isWalletPageUrl(tab.url, base) || isWalletPageUrl(tab.pendingUrl, base))
	);
}

/**
 * Where the toolbar button takes the person, clicked in window `windowId`
 * (`undefined` when Chrome did not say). `tabs` are the normal windows'
 * tabs.
 *
 *   - this window's wallet tab: focus it — never a second one beside it;
 *   - else another window's: focus that (spec 089 FR-005, as before);
 *   - else open one, here.
 *
 * The tab it focuses keeps the page it was on: navigating it again reloaded
 * the wallet under the person and lost whatever it was showing.
 */
export function toolbarTarget(tabs, windowId, base) {
	const here = typeof windowId === 'number' ? walletTabsIn(tabs, windowId, base) : [];
	const pick = here.find((tab) => tab.active) ?? here[0];
	if (pick) return { kind: 'focus', tabId: pick.id, windowId: pick.windowId };
	const elsewhere = (Array.isArray(tabs) ? tabs : []).find(
		(tab) => tab && typeof tab.id === 'number' && isWalletPageUrl(tab.url ?? tab.pendingUrl, base)
	);
	if (elsewhere) return { kind: 'focus', tabId: elsewhere.id, windowId: elsewhere.windowId };
	return { kind: 'open', windowId };
}

/**
 * May the worker close window `windowId`'s side panel? Only when the panel
 * owes no request (a record of that window's panel is still in the ledger):
 * closing it then would settle that request under the person (RB10).
 */
export function panelMayClose(records, windowId) {
	return !records.some(
		(record) => record?.surface === 'panel' && record.surfaceWindowId === windowId
	);
}

/**
 * What the side panel shows:
 *
 *   - `live` — the wallet, as ever: no wallet tab in its window, or the
 *     person chose the panel for this window (`owner === 'panel'`), or the
 *     panel is busy — a request it owes, shows or is landing, or a send or
 *     signature of its own in flight;
 *   - `elsewhere` — "Vela is open in a tab", naming that tab (`tabId`).
 *
 * @param {{ walletTabs?: { id?: number; active?: boolean }[]; owner?: string | null; busy?: boolean }} facts
 * @returns {{ kind: 'live' } | { kind: 'elsewhere'; tabId: number }}
 */
export function panelStanding({ walletTabs, owner = null, busy = false }) {
	const tabs = (walletTabs ?? []).filter((tab) => typeof tab.id === 'number');
	if (busy || owner === 'panel' || tabs.length === 0) return { kind: 'live' };
	const tab = tabs.find((t) => t.active) ?? tabs[0];
	return { kind: 'elsewhere', tabId: /** @type {number} */ (tab.id) };
}

/**
 * What a wallet tab shows — the panel's rule, the other way round:
 *
 *   - `aside` — "Vela is open in the side panel": the person chose the panel
 *     for this window (`owner === 'panel'`) and a side panel is up (`panelUp`
 *     — `false` only when Chrome says no side panel is open anywhere, so a
 *     choice left behind by a panel that went without a word never strands
 *     the tab);
 *   - `live` — otherwise, and always while the tab is busy: a send or a
 *     signature of its own in flight.
 *
 * @param {{ owner?: string | null; panelUp?: boolean | null; busy?: boolean }} facts
 * @returns {{ kind: 'live' } | { kind: 'aside' }}
 */
export function tabStanding({ owner = null, panelUp = null, busy = false }) {
	if (busy || owner !== 'panel' || panelUp === false) return { kind: 'live' };
	return { kind: 'aside' };
}

/**
 * Close window `windowId`'s side panel, when this Chrome can: `sidePanel.close`
 * arrived in Chrome 141, and the manifest's floor is 122 — feature-detected,
 * never assumed. Resolves `true` when Chrome closed it.
 */
export async function closeSidePanel(sidePanel, windowId) {
	if (typeof sidePanel?.close !== 'function' || typeof windowId !== 'number') return false;
	try {
		await sidePanel.close({ windowId });
		return true;
	} catch {
		return false;
	}
}
