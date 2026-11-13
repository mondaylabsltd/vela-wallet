/**
 * One wallet per window as the side panel AND a wallet tab keep it
 * (`one-surface.svelte.ts`), over one fake `chrome` they share: the panel
 * stands aside for a wallet tab in its window; "Use here instead" closes
 * nothing — it hands the window over and the tab stands aside the same way,
 * its wallet kept; a surface busy with a request or a send never steps
 * aside; Go to tab / Go to side panel do what they say.
 */
import { describe, expect, it, vi } from 'vitest';
import { OneSurface } from './one-surface.svelte';

type Tab = { id: number; windowId: number; url: string; active?: boolean };
type Changes = Record<string, { newValue?: unknown }>;

function memStore() {
	const data = new Map<string, string>();
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v),
		removeItem: (k: string) => void data.delete(k)
	};
}

function fakeChrome(initial: Tab[], options: { close?: boolean; panels?: number } = {}) {
	const tabs = [...initial];
	const tabListeners: (() => void)[] = [];
	const storageListeners: ((c: Changes, area: string) => void)[] = [];
	const session = new Map<string, unknown>();
	const tabEvent = {
		addListener: (fn: () => void) => tabListeners.push(fn),
		removeListener: () => {}
	};
	const emit = (changes: Changes) => {
		for (const fn of storageListeners) fn(changes, 'session');
	};
	const state = { panels: options.panels ?? 1 };
	const chrome = {
		runtime: {
			getURL: (path: string) => `chrome-extension://ext/${path}`,
			getContexts: vi.fn(async () => Array.from({ length: state.panels }, () => ({ windowId: -1 })))
		},
		windows: { getCurrent: async () => ({ id: 3 }), update: vi.fn(async () => ({})) },
		tabs: {
			query: vi.fn(async ({ windowId }: { windowId: number }) =>
				tabs.filter((t) => t.windowId === windowId)
			),
			update: vi.fn(async () => ({})),
			remove: vi.fn(async () => {}),
			onCreated: tabEvent,
			onUpdated: tabEvent,
			onRemoved: tabEvent
		},
		storage: {
			session: {
				get: async (key: string) => (session.has(key) ? { [key]: session.get(key) } : {}),
				set: async (items: Record<string, unknown>) => {
					for (const [k, v] of Object.entries(items)) session.set(k, v);
					emit(Object.fromEntries(Object.entries(items).map(([k, v]) => [k, { newValue: v }])));
				},
				remove: async (key: string) => {
					session.delete(key);
					emit({ [key]: {} });
				}
			},
			onChanged: {
				addListener: (fn: (c: Changes, area: string) => void) => storageListeners.push(fn),
				removeListener: () => {}
			}
		},
		sidePanel: {
			open: vi.fn(async () => {}),
			...(options.close ? { close: vi.fn(async () => {}) } : {})
		}
	};
	return {
		chrome,
		session,
		state,
		/** A tab appears (typed in, the install welcome, the toolbar) and Chrome says so. */
		async open(tab: Tab) {
			tabs.push(tab);
			for (const fn of tabListeners) fn();
			await tick();
		}
	};
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const WALLET = 'chrome-extension://ext/en/wallet.html';

describe('the side panel, one wallet per window', () => {
	it('is the wallet while its window has no wallet tab — a dApp’s tab is none', async () => {
		const env = fakeChrome([{ id: 1, windowId: 3, url: 'https://app.uniswap.org/' }]);
		const panel = new OneSurface(env.chrome, undefined, memStore());
		await panel.start('panel');
		expect(panel.elsewhere).toBeNull();
	});

	it('stands aside for a wallet tab opened in its window, not for one in another', async () => {
		const env = fakeChrome([]);
		const panel = new OneSurface(env.chrome, undefined, memStore());
		await panel.start('panel');
		await env.open({ id: 8, windowId: 9, url: WALLET });
		expect(panel.elsewhere).toBeNull();
		await env.open({ id: 7, windowId: 3, url: WALLET });
		await panel.check();
		expect(panel.elsewhere).toEqual({ tabId: 7, windowId: 3 });
	});

	it('comes back for a dApp request, and stands aside again when it is done', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }]);
		const panel = new OneSurface(env.chrome, undefined, memStore());
		await panel.start('panel');
		expect(panel.elsewhere).not.toBeNull();
		const release = panel.hold();
		expect(panel.elsewhere).toBeNull();
		release();
		release();
		expect(panel.elsewhere).toEqual({ tabId: 7, windowId: 3 });
	});

	it('Go to tab: the tab comes forward and Chrome closes the panel (141+)', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }], { close: true });
		const closeWindow = vi.fn();
		const panel = new OneSurface(env.chrome, closeWindow, memStore());
		await panel.start('panel');
		await panel.goToTab();
		expect(env.chrome.tabs.update).toHaveBeenCalledWith(7, { active: true });
		expect(env.chrome.windows.update).toHaveBeenCalledWith(3, { focused: true });
		expect(env.chrome.sidePanel.close).toHaveBeenCalledWith({ windowId: 3 });
		expect(closeWindow).not.toHaveBeenCalled();
	});

	it('Go to tab, before Chrome 141: the panel closes itself', async () => {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }]);
		const closeWindow = vi.fn();
		const panel = new OneSurface(env.chrome, closeWindow, memStore());
		await panel.start('panel');
		await panel.goToTab();
		expect(closeWindow).toHaveBeenCalledTimes(1);
	});
});

describe('"Use here instead" closes nothing: the other surface steps aside', () => {
	async function both(options: { panels?: number } = {}) {
		const env = fakeChrome([{ id: 7, windowId: 3, url: WALLET }], options);
		const panelStore = memStore();
		const panel = new OneSurface(env.chrome, undefined, panelStore);
		const tab = new OneSurface(env.chrome, undefined, memStore());
		await panel.start('panel');
		await tab.start('tab');
		return { env, panel, tab, panelStore };
	}

	it('by default the tab is the wallet and the panel stands aside', async () => {
		const { panel, tab } = await both();
		expect(panel.elsewhere).toEqual({ tabId: 7, windowId: 3 });
		expect(tab.tabAside).toBe(false);
	});

	it('in the panel: the panel is the wallet, the tab is NOT closed and says so', async () => {
		const { env, panel, tab } = await both();
		await panel.useHere();
		await tick();
		expect(env.chrome.tabs.remove).not.toHaveBeenCalled();
		expect(env.session.get('vela.surface.window.3')).toBe('panel');
		expect(panel.elsewhere).toBeNull();
		expect(tab.tabAside).toBe(true);
	});

	it('in the tab: the window goes back to the tab, and the panel stands aside again', async () => {
		const { env, panel, tab, panelStore } = await both();
		await panel.useHere();
		await tick();
		await tab.useHere();
		await tick();
		expect(env.session.has('vela.surface.window.3')).toBe(false);
		expect(tab.tabAside).toBe(false);
		expect(panel.elsewhere).toEqual({ tabId: 7, windowId: 3 });
		// …and the panel's claim is over, so a reload of it does not take the window again.
		expect(panelStore.getItem('vela.surface.claimed')).toBeNull();
	});

	it('a tab with a send or signature in flight keeps the wallet', async () => {
		const { panel, tab } = await both();
		const release = tab.hold();
		await panel.useHere();
		await tick();
		expect(tab.tabAside).toBe(false);
		release();
		expect(tab.tabAside).toBe(true);
	});

	it('a choice the panel left behind (no side panel up anywhere) never strands the tab', async () => {
		const { env, panel, tab } = await both();
		await panel.useHere();
		await tick();
		env.state.panels = 0;
		await tab.check();
		expect(tab.tabAside).toBe(false);
	});

	it('a panel that took its window gives it back when it goes, and takes it again on a reload', async () => {
		// The document's own events (node has no `window`).
		const doc = new EventTarget();
		vi.stubGlobal('addEventListener', doc.addEventListener.bind(doc));
		vi.stubGlobal('removeEventListener', doc.removeEventListener.bind(doc));
		const { env, panel, tab, panelStore } = await both();
		await panel.useHere();
		await tick();
		doc.dispatchEvent(new Event('pagehide'));
		await tick();
		expect(env.session.has('vela.surface.window.3')).toBe(false);
		expect(tab.tabAside).toBe(false);
		// The same panel document reloaded (its language changed): still the person's choice.
		const again = new OneSurface(env.chrome, undefined, panelStore);
		await again.start('panel');
		await tick();
		expect(env.session.get('vela.surface.window.3')).toBe('panel');
		expect(tab.tabAside).toBe(true);
		panel.stop();
		again.stop();
		vi.unstubAllGlobals();
	});

	it('Go to side panel brings the panel up in this window, straight from the click', async () => {
		const { env, panel, tab } = await both();
		await panel.useHere();
		await tick();
		tab.goToPanel();
		expect(env.chrome.sidePanel.open).toHaveBeenCalledWith({ windowId: 3 });
	});
});
