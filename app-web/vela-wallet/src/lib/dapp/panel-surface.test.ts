/* eslint-disable @typescript-eslint/no-explicit-any -- the fake runtime mirrors an untyped browser API */
/**
 * The panel's side of a request's life (spec 082 T080: RB4, RB7, RB9, RB10,
 * RB15), driven with a fake runtime that plays the worker's half of the
 * `vela.surface` port.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	PANEL_FLAG_KEY,
	PanelSurface,
	SURFACE_PORT,
	isPanelDocument,
	panelNeedsWallet
} from './panel-surface.svelte';

function request(rid: string, origin = 'https://a.example') {
	return {
		rid,
		id: rid.split(':').slice(1).join(':'),
		method: 'personal_sign',
		params: [],
		origin,
		tabId: Number(rid.split(':')[0]),
		at: 1
	};
}

function fakeChrome() {
	const posted: any[] = [];
	const toSurface: ((m: unknown) => void)[] = [];
	const closers: (() => void)[] = [];
	const sent: any[] = [];
	const connects: string[] = [];
	const storageListeners: ((changes: Record<string, unknown>, area: string) => void)[] = [];
	const chrome = {
		runtime: {
			id: 'ext',
			connect: vi.fn(({ name }: { name: string }) => {
				connects.push(name);
				return {
					postMessage: (m: unknown) => posted.push(m),
					disconnect: () => {},
					onMessage: { addListener: (fn: (m: unknown) => void) => toSurface.push(fn) },
					onDisconnect: { addListener: (fn: () => void) => closers.push(fn) }
				};
			}),
			sendMessage: vi.fn(async (m: unknown) => {
				sent.push(m);
				return { delivered: true };
			})
		},
		windows: { getCurrent: async () => ({ id: 3 }) },
		storage: {
			onChanged: {
				addListener: (fn: (changes: Record<string, unknown>, area: string) => void) =>
					storageListeners.push(fn),
				removeListener: (fn: (changes: Record<string, unknown>, area: string) => void) => {
					const at = storageListeners.indexOf(fn);
					if (at >= 0) storageListeners.splice(at, 1);
				}
			}
		}
	};
	return {
		chrome,
		posted,
		sent,
		connects,
		/** The worker pushes a message down the port. */
		push: (m: unknown) => {
			for (const fn of [...toSurface]) fn(m);
		},
		/** The worker died: the port closes. */
		drop: () => {
			const fns = [...closers];
			closers.length = 0;
			for (const fn of fns) fn();
		},
		/** The worker wrote to `storage.session` (a request arrived, a claim moved it). */
		storageChange: (changes: Record<string, unknown>, area = 'session') => {
			for (const fn of [...storageListeners]) fn(changes, area);
		}
	};
}

function memoryStore() {
	const data = new Map<string, string>();
	return {
		getItem: (k: string) => data.get(k) ?? null,
		setItem: (k: string, v: string) => void data.set(k, v)
	};
}

afterEach(() => {
	vi.useRealTimers();
	delete (globalThis as { chrome?: unknown }).chrome;
});

describe('the panel stays the panel (RB9, G23c)', () => {
	beforeEach(() => {
		(globalThis as { chrome?: unknown }).chrome = fakeChrome().chrome;
	});

	it('survives Wallet → Settings → Wallet, which drops `?panel`', () => {
		const store = memoryStore();
		expect(isPanelDocument({ protocol: 'chrome-extension:', search: '?panel' }, store)).toBe(true);
		expect(store.getItem(PANEL_FLAG_KEY)).toBe('1');
		// Settings, then back to the wallet by `goto(walletHref)`: no query.
		expect(isPanelDocument({ protocol: 'chrome-extension:', search: '' }, store)).toBe(true);
	});

	it('is not the panel in an ordinary tab, nor on the hosted site', () => {
		const store = memoryStore();
		expect(isPanelDocument({ protocol: 'chrome-extension:', search: '' }, store)).toBe(false);
		expect(isPanelDocument({ protocol: 'https:', search: '?panel' }, store)).toBe(false);
	});
});

describe('the surface port (contract §14)', () => {
	it('says hello with its window, and shows what the worker says it owes', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		expect(await surface.start()).toBe(true);
		expect(env.connects).toEqual([SURFACE_PORT]);
		expect(env.posted[0]).toEqual({ type: 'hello', kind: 'panel', windowId: 3 });
		env.push({ type: 'owed', request: request('7:a') });
		expect(surface.current?.rid).toBe('7:a');
		surface.shown('7:a');
		expect(env.posted.at(-1)).toEqual({ type: 'shown', rid: '7:a' });
		surface.stop();
	});

	it('a withdrawal clears the request and tells the host, with no words (RB15)', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		const withdrawn: [string, string][] = [];
		surface.onWithdrawn((rid, cause) => withdrawn.push([rid, cause]));
		env.push({ type: 'owed', request: request('7:a') });
		env.push({ type: 'withdrawn', rid: '7:a', cause: 'page_left' });
		expect(surface.current).toBeNull();
		expect(withdrawn).toEqual([['7:a', 'page_left']]);
		surface.stop();
	});

	it('serves a second tab’s request once the first is answered (RB7, G18)', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.push({ type: 'owed', request: request('7:a') });
		const answered = surface.answer('7:a', { result: '0xsig' });
		expect(env.posted.at(-1)).toEqual({ type: 'answer', rid: '7:a', result: '0xsig' });
		env.push({ type: 'answered', rid: '7:a', delivered: true });
		await expect(answered).resolves.toBe(true);
		expect(surface.current).toBeNull();
		env.push({ type: 'owed', request: request('9:b', 'https://b.example') });
		expect(surface.current?.rid).toBe('9:b');
		surface.stop();
	});

	it('reports an answer the page did not take — it is not ignored (G17)', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.push({ type: 'owed', request: request('7:a') });
		const answered = surface.answer('7:a', { error: { code: 4001, message: 'User rejected' } });
		env.push({ type: 'answered', rid: '7:a', delivered: false });
		await expect(answered).resolves.toBe(false);
		surface.stop();
	});

	it('a submit claim carries the operation hash and its chain (RJ2)', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		const OP = `0x${'ab'.repeat(32)}`;
		void surface.claim('7:a', 'submit', { opHash: OP, chainId: 100 });
		expect(env.posted.at(-1)).toMatchObject({
			type: 'claim',
			rid: '7:a',
			phase: 'submit',
			opHash: OP,
			chainId: 100
		});
		// Only a submit carries it: before the passkey there is no operation yet.
		void surface.claim('7:a', 'sign', { opHash: OP, chainId: 100 });
		expect(env.posted.at(-1)).not.toHaveProperty('opHash');
		surface.stop();
	});

	it('carries the op hash’s chain with an answer (RF3)', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		void surface.answer('7:a', { result: `0x${'ab'.repeat(32)}` }, { chainId: 100 });
		expect(env.posted.at(-1)).toMatchObject({ type: 'answer', opHash: { chainId: 100 } });
		surface.stop();
	});
});

describe('claims (RB5)', () => {
	it('is the worker’s verdict', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		const live = surface.claim('7:a', 'sign');
		const asked = env.posted.at(-1);
		expect(asked).toMatchObject({ type: 'claim', rid: '7:a', phase: 'sign' });
		env.push({ type: 'claimResult', nonce: asked.nonce, live: true });
		await expect(live).resolves.toBe(true);
		const dead = surface.claim('7:a', 'submit');
		env.push({
			type: 'claimResult',
			nonce: env.posted.at(-1).nonce,
			live: false,
			cause: 'page_left'
		});
		await expect(dead).resolves.toBe(false);
		surface.stop();
	});

	it('is "no" when nothing answers within 5 s, after one reconnect', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		const live = surface.claim('7:a', 'sign');
		// The worker died under the claim: the port closes and reopens.
		env.drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.connects).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(5_000);
		// The second ask went out on the new port…
		expect(env.posted.filter((m) => m.type === 'claim')).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(5_000);
		// …and nothing answered it either.
		await expect(live).resolves.toBe(false);
		surface.stop();
	});

	it('the new port says hello again, so the worker can hand back what is owed (RB4)', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.push({ type: 'owed', request: request('7:a') });
		env.drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.posted.filter((m) => m.type === 'hello')).toHaveLength(2);
		surface.stop();
	});
});

/**
 * RJ20 (G63): an idle open panel restarted the worker every ~30 s. Chrome
 * stops an idle worker, the port closes, and the panel reconnected at once —
 * which started the worker again, for nothing, forever; the 200-line log ring
 * filled with `panel.up` / `sw.start` and pushed out the request lines a bug
 * report needs. The port is now held only while this surface owes something.
 */
describe('a quiet idle panel (RJ20, G63)', () => {
	const record = (rid: string, over: Record<string, unknown> = {}) => ({
		rid,
		surface: 'panel',
		surfaceWindowId: 3,
		state: 'created',
		...over
	});

	it('does not reconnect after Chrome stops an idle worker', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.drop();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(env.connects).toHaveLength(1);
		surface.stop();
	});

	it('reconnects when the worker writes a request for this window, and not for another', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.drop();
		await vi.advanceTimersByTimeAsync(1_000);
		// Another window's request, a record leaving, another area: not ours.
		env.storageChange({ 'vela.req.9:x': { newValue: record('9:x', { surfaceWindowId: 4 }) } });
		env.storageChange({ 'vela.req.7:old': { oldValue: record('7:old') } });
		env.storageChange({ 'vela.req.7:a': { newValue: record('7:a') } }, 'local');
		env.storageChange({ 'vela.sw.log': { newValue: ['…'] } });
		expect(env.connects).toHaveLength(1);
		// This window's: the port comes back and says hello, and the worker hands it over.
		env.storageChange({ 'vela.req.7:a': { newValue: record('7:a') } });
		expect(env.connects).toHaveLength(2);
		expect(env.posted.filter((m) => m.type === 'hello')).toHaveLength(2);
		env.push({ type: 'owed', request: request('7:a') });
		expect(surface.current?.rid).toBe('7:a');
		surface.stop();
	});

	it('keeps reconnecting while a request is owed', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.push({ type: 'owed', request: request('7:a') });
		env.drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.connects).toHaveLength(2);
		surface.stop();
	});

	it('keeps reconnecting while a submit it claimed is unanswered', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.push({ type: 'owed', request: request('7:a') });
		const live = surface.claim('7:a', 'submit', { opHash: `0x${'ab'.repeat(32)}`, chainId: 100 });
		env.push({ type: 'claimResult', nonce: env.posted.at(-1).nonce, live: true });
		await expect(live).resolves.toBe(true);
		// The worker handed the next request over before this one's answer came back.
		env.push({ type: 'owed', request: request('9:b') });
		env.push({ type: 'withdrawn', rid: '9:b', cause: 'page_left' });
		expect(surface.current).toBeNull();
		env.drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.connects).toHaveLength(2);
		surface.stop();
	});

	it('a claim or an answer on a quiet panel brings the port back itself', async () => {
		vi.useFakeTimers();
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome);
		await surface.start();
		env.drop();
		await vi.advanceTimersByTimeAsync(0);
		expect(env.connects).toHaveLength(1);
		void surface.claim('7:a', 'approve');
		expect(env.connects).toHaveLength(2);
		expect(env.posted.at(-1)).toMatchObject({ type: 'claim', rid: '7:a', phase: 'approve' });
		surface.stop();
	});
});

describe('the panel owes a request while another screen shows (RB9, G55)', () => {
	it('the wallet is where it is answered: any other route is told to go there', () => {
		const caller = { kind: 'panel', windowId: 3 } as const;
		const owed = request('7:a');
		expect(panelNeedsWallet({ caller, current: owed, routeId: '/[locale]/settings' })).toBe(true);
		expect(panelNeedsWallet({ caller, current: owed, routeId: '/[locale]/contacts' })).toBe(true);
		expect(panelNeedsWallet({ caller, current: owed, routeId: '/[locale]/wallet' })).toBe(false);
		expect(panelNeedsWallet({ caller, current: null, routeId: '/[locale]/settings' })).toBe(false);
		// A request window, or a page that is not a surface at all, never moves.
		expect(
			panelNeedsWallet({
				caller: { kind: 'window', rid: '7:a' },
				current: owed,
				routeId: '/[locale]/request'
			})
		).toBe(false);
		expect(panelNeedsWallet({ caller: null, current: owed, routeId: null })).toBe(false);
	});
});
