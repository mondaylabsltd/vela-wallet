/* eslint-disable @typescript-eslint/no-explicit-any -- the fake runtime mirrors an untyped browser API */
/**
 * RB9 in a real Svelte runtime (spec 082 T210, G55).
 *
 * The device pass left the panel on Settings, asked from a second tab, and
 * nothing rose for 58 s — until the person happened to tap 钱包. The root
 * layout's effect is what brings the panel back to the wallet, and it read
 * `panelSurface.caller`, a plain private field: on its first run (before
 * `start()` resolved) the caller was `null`, the effect returned having
 * tracked nothing, and it never ran again. This drives an effect over a real
 * `PanelSurface` the way the layout does.
 */
import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';
import { PanelSurface, panelNeedsWallet } from './panel-surface.svelte';

function fakeChrome() {
	const toSurface: ((m: unknown) => void)[] = [];
	const chrome = {
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
	};
	return {
		chrome,
		push: (m: unknown) => {
			for (const fn of [...toSurface]) fn(m);
		}
	};
}

const OWED = {
	rid: '9:b:1',
	id: 'b:1',
	method: 'eth_requestAccounts',
	params: [],
	origin: 'https://b.example',
	tabId: 9,
	at: 1
};

describe('the layout’s RB9 effect over a real surface (G55)', () => {
	it('`caller` is itself reactive: an effect that reads only it runs again after start()', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome as any);
		const seen: (string | null)[] = [];
		const stop = $effect.root(() => {
			$effect(() => {
				// The shape the layout had: the caller first, and nothing else
				// read while it is null.
				seen.push(surface.caller?.kind ?? null);
			});
		});
		flushSync();
		await surface.start();
		flushSync();
		expect(seen).toEqual([null, 'panel']);
		stop();
		surface.stop();
	});

	it('runs again when a request becomes owed after the effect first ran with no caller', async () => {
		const env = fakeChrome();
		const surface = new PanelSurface(env.chrome as any);
		const seen: boolean[] = [];
		const stop = $effect.root(() => {
			$effect(() => {
				seen.push(
					panelNeedsWallet({
						caller: surface.caller,
						current: surface.current,
						routeId: '/[locale]/settings',
						allowedRoute: 'wallet'
					})
				);
			});
		});
		flushSync();
		// First run: the panel's port is not up yet.
		expect(seen).toEqual([false]);

		await surface.start();
		flushSync();
		expect(surface.caller).toEqual({ kind: 'panel', windowId: 3 });

		// Tab B asks while Settings shows: the effect must see it, with no tap.
		env.push({ type: 'owed', request: OWED });
		flushSync();
		expect(seen.at(-1)).toBe(true);

		// Answered or withdrawn: nothing pulls the panel any more.
		env.push({ type: 'withdrawn', rid: OWED.rid, cause: 'page_left' });
		flushSync();
		expect(seen.at(-1)).toBe(false);
		stop();
		surface.stop();
	});
});
