/**
 * The Safari hand-off transport (native AUDIT §6.5): the signing page in a
 * tab, its messages through Vela's content script — `handOff`'s window and
 * message events, kept to exactly what that tab, by the browser's word, said.
 */
import { describe, expect, it, vi } from 'vitest';
import type { PageHandoffDeps } from '$lib/signing/page-handoff.svelte';
import { safariSignerDeps, type SafariSignerApi } from './safari-signer';

const PAGE = 'https://sign.getvela.app/b/abc/sign?ch=post&lang=en';

function fakeApi() {
	const removed: ((tabId: number) => void)[] = [];
	const listeners = new Set<(m: unknown, s: { tab?: { id?: number }; url?: string }) => void>();
	const api = {
		tabs: {
			create: vi.fn(async () => ({ id: 77 })),
			remove: vi.fn(async (id: number) => {
				for (const f of removed) f(id);
			}),
			update: vi.fn(async () => ({})),
			sendMessage: vi.fn(async () => ({})),
			getCurrent: vi.fn(async () => ({ id: 5 })),
			onRemoved: { addListener: (f: (id: number) => void) => removed.push(f) }
		},
		runtime: {
			onMessage: {
				addListener: (f: (m: unknown, s: { tab?: { id?: number }; url?: string }) => void) =>
					listeners.add(f),
				removeListener: (f: (m: unknown, s: unknown) => void) =>
					listeners.delete(f as (m: unknown, s: { tab?: { id?: number }; url?: string }) => void)
			}
		}
	};
	const fromTab = (tabId: number, url: string, data: unknown) => {
		for (const f of listeners)
			f({ type: 'vela.signer.fromPage', data }, { tab: { id: tabId }, url });
	};
	return { api: api as unknown as SafariSignerApi, raw: api, fromTab, listeners };
}

const base = { open: () => null, listen: () => () => {} } as unknown as PageHandoffDeps;
const tick = () => new Promise((r) => setTimeout(r, 0));

describe('safariSignerDeps', () => {
	it('is the window transport when there is no extension API', () => {
		expect(safariSignerDeps(base, null)).toBe(base);
	});

	it('opens the page in a tab, and queues what is posted before the tab exists', async () => {
		const { api, raw } = fakeApi();
		const deps = safariSignerDeps(base, api);
		const win = deps.open(PAGE) as unknown as { postMessage(m: unknown): void; closed: boolean };
		expect(raw.tabs.create).toHaveBeenCalledWith({ url: PAGE, active: true });
		win.postMessage({ vela: 'intent', id: 'x' });
		await tick();
		expect(raw.tabs.sendMessage).toHaveBeenCalledWith(77, {
			type: 'vela.signer.toPage',
			data: { vela: 'intent', id: 'x' }
		});
		expect(win.closed).toBe(false);
	});

	it("hears only that tab, with the browser's word for its origin", async () => {
		const { api, fromTab } = fakeApi();
		const deps = safariSignerDeps(base, api);
		const win = deps.open(PAGE);
		await tick();
		const heard: MessageEvent[] = [];
		const stop = deps.listen((event) => heard.push(event));
		fromTab(77, PAGE, { vela: 'ready', v: 1 });
		fromTab(12, 'https://evil.example/', { vela: 'result', id: 'x' }); // another tab
		expect(heard).toHaveLength(1);
		expect(heard[0].source).toBe(win);
		expect(heard[0].origin).toBe('https://sign.getvela.app');
		expect(heard[0].data).toEqual({ vela: 'ready', v: 1 });
		stop();
		fromTab(77, PAGE, { vela: 'ready', v: 1 });
		expect(heard).toHaveLength(1);
	});

	it('closing puts the person back on the wallet, then closes the tab', async () => {
		const { api, raw } = fakeApi();
		const deps = safariSignerDeps(base, api);
		const win = deps.open(PAGE) as unknown as { close(): void; closed: boolean };
		await tick();
		win.close();
		await tick();
		await tick();
		expect(raw.tabs.update).toHaveBeenCalledWith(5, { active: true });
		expect(raw.tabs.remove).toHaveBeenCalledWith(77);
		expect(win.closed).toBe(true);
	});

	it('a tab the person closed is a closed window', async () => {
		const { api, raw } = fakeApi();
		const deps = safariSignerDeps(base, api);
		const win = deps.open(PAGE) as unknown as { closed: boolean };
		await tick();
		await raw.tabs.remove(77);
		expect(win.closed).toBe(true);
	});
});
