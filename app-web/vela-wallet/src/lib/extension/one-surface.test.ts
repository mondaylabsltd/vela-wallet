/**
 * One wallet per browser window (`extension/lib/one-surface.js`).
 *
 * The owner's report: Vela open in a tab (`en/wallet.html`) AND in the side
 * panel of the same window, the two showing different assets and totals. The
 * rules the worker and the panel apply so a window holds one live wallet —
 * and a dApp's request still rises where spec 082 routes it.
 */
import { describe, expect, it, vi } from 'vitest';
import {
	closeSidePanel,
	isWalletPageUrl,
	OWNER_PREFIX,
	ownerKey,
	panelMayClose,
	panelStanding,
	tabStanding,
	toolbarTarget,
	walletTabsIn
} from '../../../extension/lib/one-surface.js';

const BASE = 'chrome-extension://ext/';

describe('which extension pages are a wallet', () => {
	it.each([
		['en/wallet.html', true],
		['zh/settings.html', true],
		['en.html', true],
		['open.html', true],
		['open.html?installed=1', true],
		// The request window's page and its doorway are a request, not a wallet.
		['en/request.html?rid=7:1', false],
		['open.html?rid=7:1', false],
		// The side panel's doorway and the panel's own document.
		['panel.html', false],
		['en/wallet.html?panel', false],
		// A file the harness reads, and anything that is not an extension page.
		['manifest.json', false]
	])('%s → %s', (path, wallet) => {
		expect(isWalletPageUrl(BASE + path, BASE)).toBe(wallet);
	});

	it('is never another origin, nor nothing', () => {
		expect(isWalletPageUrl('https://getvela.app/en/wallet', BASE)).toBe(false);
		expect(isWalletPageUrl(undefined, BASE)).toBe(false);
	});

	it('finds a window’s wallet tabs, a loading one by where it is going', () => {
		const tabs = [
			{ id: 1, windowId: 3, url: `${BASE}en/wallet.html` },
			{ id: 2, windowId: 3, url: 'https://app.uniswap.org/' },
			{ id: 3, windowId: 3, url: '', pendingUrl: `${BASE}open.html` },
			{ id: 4, windowId: 9, url: `${BASE}en/wallet.html` }
		];
		expect(walletTabsIn(tabs, 3, BASE).map((t: { id: number }) => t.id)).toEqual([1, 3]);
		expect(walletTabsIn(null, 3, BASE)).toEqual([]);
	});
});

describe('the toolbar button', () => {
	const tab = (id: number, windowId: number, path: string, active = false) => ({
		id,
		windowId,
		url: BASE + path,
		active
	});

	it('focuses this window’s wallet tab — never a second one beside it', () => {
		const tabs = [tab(1, 9, 'en/wallet.html'), tab(2, 3, 'en/wallet.html')];
		expect(toolbarTarget(tabs, 3, BASE)).toEqual({ kind: 'focus', tabId: 2, windowId: 3 });
	});

	it('prefers the active one of two', () => {
		const tabs = [tab(1, 3, 'en/wallet.html'), tab(2, 3, 'en/settings.html', true)];
		expect(toolbarTarget(tabs, 3, BASE)).toEqual({ kind: 'focus', tabId: 2, windowId: 3 });
	});

	it('else another window’s, as before (089 FR-005)', () => {
		expect(toolbarTarget([tab(1, 9, 'en/wallet.html')], 3, BASE)).toEqual({
			kind: 'focus',
			tabId: 1,
			windowId: 9
		});
	});

	it('else opens one here; a request window or the harness’s manifest is no wallet', () => {
		const tabs = [tab(1, 3, 'en/request.html?rid=7:1'), tab(2, 3, 'manifest.json')];
		expect(toolbarTarget(tabs, 3, BASE)).toEqual({ kind: 'open', windowId: 3 });
		expect(toolbarTarget([], undefined, BASE)).toEqual({ kind: 'open', windowId: undefined });
	});
});

describe('may the worker close a window’s side panel', () => {
	it('not while that panel owes a request (it would be settled under the person)', () => {
		const records = [{ surface: 'panel', surfaceWindowId: 3 }];
		expect(panelMayClose(records, 3)).toBe(false);
	});

	it('yes when it owes nothing — another window’s panel, or a request window, owes its own', () => {
		const records = [
			{ surface: 'panel', surfaceWindowId: 9 },
			{ surface: 'window', surfaceWindowId: 3 }
		];
		expect(panelMayClose(records, 3)).toBe(true);
		expect(panelMayClose([], 3)).toBe(true);
	});
});

describe('what the side panel shows', () => {
	const walletTabs = [
		{ id: 1, active: false },
		{ id: 2, active: true }
	];

	it('the wallet, with no wallet tab in its window', () => {
		expect(panelStanding({ walletTabs: [] })).toEqual({ kind: 'live' });
	});

	it('"Vela is open in a tab" while one is, naming the active one', () => {
		expect(panelStanding({ walletTabs })).toEqual({ kind: 'elsewhere', tabId: 2 });
	});

	it('the wallet while a dApp request is on it — owed, shown or landing (spec 082)', () => {
		expect(panelStanding({ walletTabs, busy: true })).toEqual({ kind: 'live' });
	});

	it('the wallet once the person chose the panel for this window ("Use here instead")', () => {
		expect(panelStanding({ walletTabs, owner: 'panel' })).toEqual({ kind: 'live' });
		expect(panelStanding({ walletTabs, owner: null })).toEqual({ kind: 'elsewhere', tabId: 2 });
	});
});

describe('what a wallet tab shows — the panel’s rule the other way round', () => {
	it('the wallet by default: the tab is a window’s wallet', () => {
		expect(tabStanding({})).toEqual({ kind: 'live' });
		expect(tabStanding({ owner: null, panelUp: true })).toEqual({ kind: 'live' });
	});

	it('"Vela is open in the side panel" once the person chose the panel there', () => {
		expect(tabStanding({ owner: 'panel', panelUp: true })).toEqual({ kind: 'aside' });
		// Chrome could not say whether a panel is up: the choice stands.
		expect(tabStanding({ owner: 'panel', panelUp: null })).toEqual({ kind: 'aside' });
	});

	it('the wallet when no side panel is up anywhere — a choice left behind never strands it', () => {
		expect(tabStanding({ owner: 'panel', panelUp: false })).toEqual({ kind: 'live' });
	});

	it('the wallet while a send or a signature is in flight in it', () => {
		expect(tabStanding({ owner: 'panel', panelUp: true, busy: true })).toEqual({ kind: 'live' });
	});

	it('names each window’s choice under its own key', () => {
		expect(ownerKey(3)).toBe('vela.surface.window.3');
		expect(ownerKey(3).startsWith(OWNER_PREFIX)).toBe(true);
	});
});

describe('closing a side panel (Chrome 141+, the manifest’s floor is 122)', () => {
	it('closes the window’s panel where Chrome can', async () => {
		const close = vi.fn(async () => {});
		expect(await closeSidePanel({ close }, 3)).toBe(true);
		expect(close).toHaveBeenCalledWith({ windowId: 3 });
	});

	it('says it could not where the API is missing or refuses', async () => {
		expect(await closeSidePanel({}, 3)).toBe(false);
		expect(await closeSidePanel(undefined, 3)).toBe(false);
		expect(
			await closeSidePanel(
				{
					close: async () => {
						throw new Error('No active side panel');
					}
				},
				3
			)
		).toBe(false);
	});
});
