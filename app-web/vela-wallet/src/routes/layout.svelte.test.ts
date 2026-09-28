/* eslint-disable @typescript-eslint/no-explicit-any -- the fake runtime mirrors an untyped browser API */
/**
 * The root layout in a browser (spec 082 T211, RB9, G55).
 *
 * The side panel showed Settings; tab B asked to connect; nothing rose for
 * 58 s, until the person happened to tap 钱包 — and a dApp would have waited
 * out its 5 minutes. The layout's effect is what brings the panel back to the
 * wallet, where a request is answered. It read a field that was not reactive,
 * so it ran once, before the panel's port was up, and never again. This
 * mounts the real layout over the real `panelSurface` and pushes a request
 * the way the worker does.
 *
 * G58: the account follow and the lower-case-grant rewrite ran only in the
 * wallet page, so a switch made in Settings reached no site and a panel that
 * booted on Settings kept its old grants. They run from this layout now, on
 * every route; what they DO is `follow.test.ts`'s, what is proved here is
 * that the layout drives them wherever the wallet boots.
 */
import { flushSync, tick } from 'svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const nav = vi.hoisted(() => ({
	route: '/[locale]/settings',
	goto: vi.fn(async () => {}),
	extension: false
}));

const follow = vi.hoisted(() => ({
	normalizeGrantSpelling: vi.fn(async () => [] as string[]),
	note: vi.fn<(view: { address: string }) => null>(() => null)
}));

vi.mock('$lib/dapp/transport', () => ({ inExtension: () => nav.extension }));
vi.mock('$lib/dapp/follow', () => ({
	normalizeGrantSpelling: follow.normalizeGrantSpelling,
	sessionFollower: { note: follow.note }
}));
// Read lazily (a getter), so the reactive session below exists by the time
// the layout's effect asks for it.
vi.mock('$lib/session/core/session.svelte', () => ({
	session: {
		get view() {
			return sessionState.view;
		}
	}
}));

vi.mock('$app/navigation', () => ({
	goto: nav.goto,
	afterNavigate: () => {},
	beforeNavigate: () => {}
}));
vi.mock('$app/state', () => ({
	page: {
		get route() {
			return { id: nav.route };
		},
		params: { locale: 'zh' },
		data: {}
	},
	updated: { current: false }
}));
vi.mock('$app/paths', () => ({
	resolve: (path: string, params?: Record<string, string>) =>
		path.replace('[locale]', params?.locale ?? 'en')
}));

import Layout from './+layout.svelte';
import { panelSurface } from '$lib/dapp/panel-surface.svelte';

function fakeChrome() {
	const toSurface: ((m: unknown) => void)[] = [];
	return {
		chrome: {
			runtime: {
				id: 'ext',
				connect: () => ({
					postMessage: () => {},
					disconnect: () => {},
					onMessage: { addListener: (fn: (m: unknown) => void) => toSurface.push(fn) },
					onDisconnect: { addListener: () => {} }
				}),
				sendMessage: async () => ({ delivered: true })
			},
			windows: { getCurrent: async () => ({ id: 3 }) },
			storage: { onChanged: { addListener: () => {}, removeListener: () => {} } }
		},
		push: (m: unknown) => {
			for (const fn of [...toSurface]) fn(m);
		}
	};
}

const children = createRawSnippet(() => ({ render: () => '<p>settings</p>' }));

/** The session the layout follows, as reactive as the real one. */
class FakeSession {
	view = $state({
		loading: false,
		has_wallet: true,
		address: '0xOne',
		active_index: 0,
		accounts: [] as { account: { address: string } }[]
	});
}
const sessionState = new FakeSession();

afterEach(() => {
	panelSurface.stop();
	delete (globalThis as { chrome?: unknown }).chrome;
	nav.goto.mockClear();
	nav.extension = false;
	follow.normalizeGrantSpelling.mockClear();
	follow.note.mockClear();
});

describe('RB9 from the root layout (G55)', () => {
	it('a request owed while Settings shows takes the panel to the wallet, with no tap', async () => {
		const env = fakeChrome();
		(globalThis as { chrome?: unknown }).chrome = env.chrome as any;
		nav.route = '/[locale]/settings';
		const screen = render(Layout, { props: { children } as any });
		await tick();
		// The panel's port comes up after the layout's effect first ran.
		await panelSurface.start({ kind: 'panel', windowId: 3 });
		flushSync();
		expect(nav.goto).not.toHaveBeenCalled();

		env.push({
			type: 'owed',
			request: {
				rid: '9:b:1',
				id: 'b:1',
				method: 'eth_requestAccounts',
				params: [],
				origin: 'https://b.example',
				tabId: 9,
				at: 1
			}
		});
		flushSync();
		await tick();
		expect(nav.goto).toHaveBeenCalledWith('/zh/wallet');
		await screen.unmount();
	});

	it('stays where it is on the wallet, and when nothing is owed', async () => {
		const env = fakeChrome();
		(globalThis as { chrome?: unknown }).chrome = env.chrome as any;
		nav.route = '/[locale]/wallet';
		const screen = render(Layout, { props: { children } as any });
		await panelSurface.start({ kind: 'panel', windowId: 3 });
		env.push({
			type: 'owed',
			request: {
				rid: '9:b:1',
				id: 'b:1',
				method: 'eth_requestAccounts',
				params: [],
				origin: 'https://b.example',
				tabId: 9,
				at: 1
			}
		});
		flushSync();
		await tick();
		expect(nav.goto).not.toHaveBeenCalled();
		await screen.unmount();
	});
});

describe('the account follow and the grant spelling, from every route (G58)', () => {
	it('booting on Settings rewrites old grants once and follows every session view', async () => {
		nav.extension = true;
		nav.route = '/[locale]/settings';
		const screen = render(Layout, { props: { children } as any });
		await vi.waitFor(() => expect(follow.normalizeGrantSpelling).toHaveBeenCalledTimes(1));
		await vi.waitFor(() => expect(follow.note).toHaveBeenCalled());
		expect(follow.note.mock.calls.at(-1)?.[0].address).toBe('0xOne');

		// 切换账户 → Parallel Two, on Settings: the follow sees it here.
		sessionState.view.address = '0xTwo';
		flushSync();
		expect(follow.note.mock.calls.at(-1)?.[0].address).toBe('0xTwo');
		expect(follow.normalizeGrantSpelling).toHaveBeenCalledTimes(1);
		await screen.unmount();
	});

	it('is not loaded outside the extension', async () => {
		nav.extension = false;
		const screen = render(Layout, { props: { children } as any });
		await tick();
		await new Promise((r) => setTimeout(r, 50));
		expect(follow.normalizeGrantSpelling).not.toHaveBeenCalled();
		expect(follow.note).not.toHaveBeenCalled();
		await screen.unmount();
	});
});
