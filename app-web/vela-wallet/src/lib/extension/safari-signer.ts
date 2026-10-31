/**
 * The trusted signing page's hand-off, in the Safari package (native AUDIT
 * §6.5) — the same `handOff` (`$lib/signing/page-handoff.svelte.ts`), over a
 * transport Safari can carry.
 *
 * Measured on iOS Safari: a page an extension page opens with `window.open`
 * has no `window.opener`, so the page has no window to answer and the
 * hand-off's postMessage goes nowhere. So, in the Safari package only:
 *
 *   - the page opens in a TAB (`tabs.create`), returned to `handOff` as a
 *     window-shaped object ({@link RelayWindow}) — `postMessage`, `close`,
 *     `focus`, `closed`;
 *   - what `handOff` posts to it travels as `tabs.sendMessage` to that tab,
 *     where Vela's content script on the signing page
 *     (`extension/safari/attest.js`) posts it into the page;
 *   - what the page answers comes back from that content script by
 *     `runtime.sendMessage`, and reaches `handOff` as a message event whose
 *     `source` is the tab's window object and whose `origin` is the BROWSER's
 *     word for the sending frame (`sender.url`) — only from that tab.
 *
 * Nothing about what is accepted changes: `handOff` still checks the origin,
 * the request id, and hands the answer to the core, which accepts only a
 * signature by the account's own key over the bytes it computed itself.
 */
import type { PageHandoffDeps } from '$lib/signing/page-handoff.svelte';

/** What of the extension API this needs (Safari's `browser`, also named `chrome`). */
export interface SafariSignerApi {
	tabs: {
		create(options: { url: string; active?: boolean }): Promise<{ id?: number }>;
		remove(tabId: number): Promise<void>;
		update(tabId: number, info: { active?: boolean }): Promise<unknown>;
		sendMessage(tabId: number, message: unknown): Promise<unknown>;
		getCurrent?(): Promise<{ id?: number } | undefined>;
		onRemoved: { addListener(fn: (tabId: number) => void): void };
	};
	runtime: {
		onMessage: {
			addListener(
				fn: (message: unknown, sender: { tab?: { id?: number }; url?: string }) => void
			): void;
			removeListener(fn: (message: unknown, sender: unknown) => void): void;
		};
	};
}

/** The signing page's tab, as `handOff` holds a window. */
export class RelayWindow {
	tabId: number | null = null;
	closed = false;
	#queue: unknown[] = [];
	readonly #api: SafariSignerApi;
	readonly #walletTab: Promise<number | undefined>;

	constructor(api: SafariSignerApi, url: string) {
		this.#api = api;
		this.#walletTab = Promise.resolve(api.tabs.getCurrent?.())
			.then((tab) => tab?.id)
			.catch(() => undefined);
		api.tabs.create({ url, active: true }).then(
			(tab) => {
				if (typeof tab?.id !== 'number') {
					this.closed = true;
					return;
				}
				this.tabId = tab.id;
				if (this.closed) {
					void api.tabs.remove(tab.id).catch(() => {});
					return;
				}
				for (const message of this.#queue.splice(0)) this.postMessage(message);
			},
			() => {
				this.closed = true;
			}
		);
	}

	postMessage(message: unknown): void {
		if (this.closed) return;
		if (this.tabId === null) {
			this.#queue.push(message);
			return;
		}
		void this.#api.tabs
			.sendMessage(this.tabId, { type: 'vela.signer.toPage', data: message })
			.catch(() => {});
	}

	focus(): void {
		if (this.tabId !== null)
			void this.#api.tabs.update(this.tabId, { active: true }).catch(() => {});
	}

	/** Close the page's tab — and put the person back on the wallet that opened it. */
	close(): void {
		if (this.closed) return;
		this.closed = true;
		const tabId = this.tabId;
		void this.#walletTab.then(async (wallet) => {
			if (typeof wallet === 'number')
				await this.#api.tabs.update(wallet, { active: true }).catch(() => {});
			if (tabId !== null) await this.#api.tabs.remove(tabId).catch(() => {});
		});
	}
}

/**
 * `base` with Safari's `open` and `listen`. `api` is the extension API, or
 * `null` when there is none (a test, the hosted site): then `base` as given.
 */
export function safariSignerDeps(
	base: PageHandoffDeps,
	api: SafariSignerApi | null = (globalThis as { chrome?: SafariSignerApi }).chrome ?? null
): PageHandoffDeps {
	if (!api?.tabs?.create || !api.runtime?.onMessage) return base;
	const open = new Set<RelayWindow>();
	api.tabs.onRemoved.addListener((tabId) => {
		for (const win of open) {
			if (win.tabId === tabId) {
				win.closed = true;
				open.delete(win);
			}
		}
	});
	return {
		...base,
		open: (url) => {
			const win = new RelayWindow(api, url);
			open.add(win);
			return win as unknown as Window;
		},
		listen: (handler) => {
			const listener = (message: unknown, sender: { tab?: { id?: number }; url?: string }) => {
				const m = message as { type?: string; data?: unknown } | null;
				if (m?.type !== 'vela.signer.fromPage') return;
				const win = [...open].find((w) => w.tabId !== null && w.tabId === sender?.tab?.id);
				if (!win) return;
				let origin: string;
				try {
					origin = new URL(sender.url ?? '').origin;
				} catch {
					return;
				}
				handler({ source: win, origin, data: m.data } as unknown as MessageEvent);
			};
			api.runtime.onMessage.addListener(listener);
			return () =>
				api.runtime.onMessage.removeListener(
					listener as (message: unknown, sender: unknown) => void
				);
		}
	};
}
