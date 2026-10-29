/**
 * One wallet per browser window, as pure rules.
 *
 * The owner's report: the wallet open in a tab AND in the side panel of the
 * same window, the two showing different things — "this is too strange". A
 * window holds ONE live wallet. A tab is the bigger, deliberate surface, so
 * where both are in one window the tab is the wallet and the side panel
 * steps aside: it says Vela is open in a tab, with "Go to tab" and "Use here
 * instead" (`panelStanding`), and the toolbar goes to that tab rather than
 * opening a second one (`toolbarTarget`).
 *
 * A dApp's request is not a wallet surface for this rule (spec 082): it still
 * opens the asking tab's side panel, which raises the request over the wallet
 * for as long as it owes it — the routing is the worker's and is untouched.
 *
 * Dependency-free, with no extension or DOM API: the worker and the panel
 * both apply these, and `one-surface.test.ts` drives every branch.
 */

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
 *     person said "Use here instead" for every one of them (`usedHere`, tab
 *     ids), or the panel is busy with a request — one it owes, or one whose
 *     receipt or landing is on screen;
 *   - `elsewhere` — "Vela is open in a tab", naming that tab (`tabId`).
 *
 * @param {{ walletTabs?: { id?: number; active?: boolean }[]; usedHere?: number[]; busy?: boolean }} facts
 * @returns {{ kind: 'live' } | { kind: 'elsewhere'; tabId: number }}
 */
export function panelStanding({ walletTabs, usedHere = [], busy = false }) {
	const others = (walletTabs ?? []).filter(
		(tab) => typeof tab.id === 'number' && !usedHere.includes(tab.id)
	);
	if (busy || others.length === 0) return { kind: 'live' };
	const tab = others.find((t) => t.active) ?? others[0];
	return { kind: 'elsewhere', tabId: /** @type {number} */ (tab.id) };
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
