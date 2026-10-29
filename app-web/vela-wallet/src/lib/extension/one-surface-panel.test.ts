/**
 * The side panel's half of one wallet per window (`one-surface.svelte.ts`),
 * over a fake `chrome`: it watches its own window's tabs, stands aside while
 * one is a wallet, comes back for a dApp request, and its two buttons do what
 * they say — Go to tab (the tab forward, the panel closed where Chrome lets
 * it, else closing itself) and Use here instead (the tab closed).
 */
import { describe, expect, it, vi } from 'vitest';
import { OneSurface } from './one-surface.svelte';

type Tab = { id: number; windowId: number; url: string; active?: boolean };

function fakeChrome(initial: Tab[], options: { close?: boolean } = {}) {
	let tabs = [...initial];
	const listeners: (() => void)[] = [];
	const event = { addListener: (fn: () => void) => listeners.push(fn), removeListener: () => {} };
	const chrome = {
		runtime: { getURL: (path: string) => `chrome-extension://ext/${path}` },
		windows: { getCurrent: async () => ({ id: 3 }), update: vi.fn(async () => ({})) },
		tabs: {
			query: vi.fn(async ({ windowId }: { windowId: number }) =>
				tabs.filter((t) => t.windowId === windowId)
			),
			update: vi.fn(async () => ({})),
			remove: vi.fn(async (ids: number | number[]) => {
				const gone = new Set(Array.isArray(ids) ? ids : [ids]);
				tabs = tabs.filter((t) => !gone.has(t.id));
			}),
			onCreated: event,
			onUpdated: event,
			onRemoved: event
		},
		...(options.close ? { sidePanel: { close: vi.fn(async () => {}) } } : {})
	};
	return {
		chrome,
		/** A tab appears (typed in, the install welcome, the toolbar) and Chrome says so. */
		async open(tab: Tab) {
			tabs.push(tab);
			for (const fn of listeners) fn();
			await new Promise((resolve) => setTimeout(resolve, 0));
		}
	};
}

const WALLET = 'chrome-extension://ext/en/wallet.html';

describe('the side panel, one wallet per window', () => {
	it('is the wallet while its window has no wallet tab — a dApp’s tab is none', async () => {
		const env = fakeChrome([{ id: 1, windowId: 3, url: 'https://app.uniswap.org/' }]);
		const surface = new OneSurface(env.chrome);
		await surface.start();
		expect(surface.elsewhere).toBeNull();
	});

	it('stands aside for a wallet tab opened in its window, not for one in another', async () => {
		const env = fakeChrome([]);
		const surface = new OneSurface(env.chrome);
		await surface.start();
		await env.open({ id: 8, windowId: 9, url: WALLET });
		expect(surface.elsewhere).toBeNull();
		await env.open({ id: 7, windowId: 3, url: WALLET });
		await surface.check();
		expect(surface.elsewhere).toEqual({ tabId: 7, windowId: 3 });
	});

	it('comes back for a dApp request, and stands aside again when it is done', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }]);
		const surface = new OneSurface(env.chrome);
		await surface.start();
		expect(surface.elsewhere).not.toBeNull();
		const release = surface.hold();
		expect(surface.elsewhere).toBeNull();
		release();
		release();
		expect(surface.elsewhere).toEqual({ tabId: 7, windowId: 3 });
	});

	it('Go to tab: the tab comes forward and Chrome closes the panel (141+)', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }], { close: true });
		const closeWindow = vi.fn();
		const surface = new OneSurface(env.chrome, closeWindow);
		await surface.start();
		await surface.goToTab();
		expect(env.chrome.tabs.update).toHaveBeenCalledWith(7, { active: true });
		expect(env.chrome.windows.update).toHaveBeenCalledWith(3, { focused: true });
		expect(env.chrome.sidePanel?.close).toHaveBeenCalledWith({ windowId: 3 });
		expect(closeWindow).not.toHaveBeenCalled();
	});

	it('Go to tab, before Chrome 141: the panel closes itself', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }]);
		const closeWindow = vi.fn();
		const surface = new OneSurface(env.chrome, closeWindow);
		await surface.start();
		await surface.goToTab();
		expect(closeWindow).toHaveBeenCalledTimes(1);
	});

	it('Use here instead: the window’s wallet tab closes and the panel is the wallet', async () => {
		const env = fakeChrome([
			{ id: 7, windowId: 3, url: WALLET },
			{ id: 8, windowId: 9, url: WALLET }
		]);
		const surface = new OneSurface(env.chrome);
		await surface.start();
		await surface.useHere();
		expect(env.chrome.tabs.remove).toHaveBeenCalledWith([7]);
		expect(surface.elsewhere).toBeNull();
	});

	it('Use here instead holds even when the tab would not close; a new tab asks again', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }]);
		env.chrome.tabs.remove.mockRejectedValueOnce(new Error('Tabs cannot be edited right now'));
		const surface = new OneSurface(env.chrome);
		await surface.start();
		await surface.useHere();
		expect(surface.elsewhere).toBeNull();
		await env.open({ id: 9, windowId: 3, url: WALLET });
		await surface.check();
		expect(surface.elsewhere).toEqual({ tabId: 9, windowId: 3 });
	});
});
