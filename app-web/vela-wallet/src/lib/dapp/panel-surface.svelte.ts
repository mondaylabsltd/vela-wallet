/**
 * The surface's side of a request's life (spec 082 RB4, RB7, RB9, RB10,
 * RB15; contract §14) — the side panel, or a dedicated request window.
 *
 * Before 082 the panel ASKED the worker what its tab owed (`requestCurrent`,
 * filtered by the active tab — G18) and learned of new requests by watching
 * storage; the worker knew nothing about the panel, so Chrome's ✕ answered
 * nobody (EX6) and a panel that had left `?panel` behind on the way to
 * Settings stopped being the panel (G23c).
 *
 * Now the surface holds a `vela.surface` port to the worker for as long as it
 * lives:
 *
 *   - `hello` names it (its window, or the one request a window was opened
 *     for); the worker pushes `owed` — the oldest request of that window,
 *     whichever tab asked — and `withdrawn` when a request ends without a
 *     decision (the page left, the limit passed): the card or sheet closes
 *     with no words (RB15);
 *   - `claim` asks, at approve / sign / submit, whether the request is still
 *     live; no answer in 5 s after one reconnect counts as "no", because a
 *     false "no" costs one re-approval and a false "yes" a signature nobody is
 *     there to receive (RB5);
 *   - `answer` hands the answer back and hears whether the page took it;
 *   - the port closing — Chrome's ✕, a reload — is how the worker learns the
 *     surface is gone, and a 20 s ping keeps it awake while a person decides.
 *
 * The panel stays the panel (RB9): the first `?panel` load marks the document
 * in sessionStorage, so Wallet → Settings → Wallet (which drops the query) is
 * still the panel. The port is started from the root layout, so it outlives
 * every in-app navigation.
 */
import type { ExtensionRequest } from './transport';

/**
 * The worker's names (`extension/lib/protocol.js`), declared here because the
 * app bundle must not pull in the worker's modules; `one-surface.test.ts`
 * pins each to the worker's own.
 */
export const SURFACE_PORT = 'vela.surface';
/** The worker's ledger prefix in `storage.session`. */
export const REQUEST_PREFIX = 'vela.req.';
/** sessionStorage: this document was opened as the side panel. */
export const PANEL_FLAG_KEY = 'vela.surface.panel';

const PING_MS = 20_000;
const CLAIM_TIMEOUT_MS = 5_000;
const ANSWER_TIMEOUT_MS = 10_000;
const RECONNECT_MS = [0, 250, 1_000, 3_000];

export type SurfaceCaller = { kind: 'panel'; windowId: number } | { kind: 'window'; rid: string };
export type ClaimPhase = 'approve' | 'sign' | 'submit';
export type SurfaceAnswer = {
	result?: unknown;
	error?: { code: number; message: string };
};

interface PortLike {
	postMessage(message: unknown): void;
	disconnect(): void;
	onMessage: { addListener(fn: (message: unknown) => void): void };
	onDisconnect: { addListener(fn: () => void): void };
}

export interface SurfaceRuntime {
	connect(info: { name: string }): PortLike;
	sendMessage(message: unknown): Promise<unknown>;
}

interface ChromeLike {
	runtime?: SurfaceRuntime & { id?: string };
	windows?: { getCurrent(): Promise<{ id?: number }> };
	storage?: {
		onChanged?: {
			addListener(fn: (changes: Record<string, unknown>, area: string) => void): void;
			removeListener?(fn: (changes: Record<string, unknown>, area: string) => void): void;
		};
	};
}

function chromeApi(): ChromeLike | undefined {
	return (globalThis as { chrome?: ChromeLike }).chrome;
}

/**
 * Is this document the extension's side panel? `?panel` (what the panel's
 * doorway adds) or the mark the first `?panel` load left (RB9).
 */
export function isPanelDocument(
	loc: { protocol: string; search: string } | undefined = globalThis.location,
	store: Pick<Storage, 'getItem' | 'setItem'> | undefined = globalThis.sessionStorage
): boolean {
	if (!loc || loc.protocol !== 'chrome-extension:') return false;
	if (typeof chromeApi()?.runtime?.sendMessage !== 'function') return false;
	if (/[?&]panel(?:[=&]|$)/.test(loc.search)) {
		try {
			store?.setItem(PANEL_FLAG_KEY, '1');
		} catch {
			/* storage denied — the query still says so on this load */
		}
		return true;
	}
	try {
		return store?.getItem(PANEL_FLAG_KEY) === '1';
	} catch {
		return false;
	}
}

function isRequest(value: unknown): value is ExtensionRequest {
	if (!value || typeof value !== 'object') return false;
	const v = value as Partial<ExtensionRequest>;
	return (
		typeof v.rid === 'string' &&
		typeof v.id === 'string' &&
		typeof v.method === 'string' &&
		Array.isArray(v.params) &&
		typeof v.origin === 'string' &&
		typeof v.tabId === 'number'
	);
}

type Withdrawn = (rid: string, cause: string) => void;

export class PanelSurface {
	/** The request this surface owes an answer for, as the worker handed it. */
	current = $state<ExtensionRequest | null>(null);

	#runtime: SurfaceRuntime | null;
	#chrome: ChromeLike | undefined;
	#caller: SurfaceCaller | null = null;
	#port: PortLike | null = null;
	#nonce = 0;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- port bookkeeping, never drawn
	#claims = new Map<number, (live: boolean) => void>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- port bookkeeping, never drawn
	#answers = new Map<string, (delivered: boolean) => void>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- listeners, never drawn
	#withdrawn = new Set<Withdrawn>();
	#failures = 0;
	#reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	#ping: ReturnType<typeof setInterval> | null = null;
	#wake: ((changes: Record<string, unknown>, area: string) => void) | null = null;
	#stopped = false;

	constructor(chrome?: ChromeLike) {
		this.#chrome = chrome;
		this.#runtime = chrome?.runtime ?? null;
	}

	#api(): ChromeLike | undefined {
		return this.#chrome ?? chromeApi();
	}

	/** Who this surface is, once started. */
	get caller(): SurfaceCaller | null {
		return this.#caller;
	}

	/**
	 * Open the port and say who this is. A panel names its window; a request
	 * window passes its `rid`. `false` outside the extension.
	 */
	async start(caller?: SurfaceCaller): Promise<boolean> {
		if (this.#caller) return true;
		const api = this.#api();
		this.#runtime ??= api?.runtime ?? null;
		if (typeof this.#runtime?.connect !== 'function') return false;
		let resolved = caller ?? null;
		if (!resolved) {
			try {
				const win = await api?.windows?.getCurrent();
				if (typeof win?.id === 'number') resolved = { kind: 'panel', windowId: win.id };
			} catch {
				resolved = null;
			}
		}
		if (!resolved || this.#caller) return this.#caller !== null;
		this.#caller = resolved;
		this.#stopped = false;
		this.#connect();
		// The worker also writes the ledger into storage.session; a write while
		// this port is down (a worker restart the port has not noticed yet)
		// brings it back at once (RB8).
		this.#wake = (changes, area) => {
			if (area !== 'session' || this.#port) return;
			if (Object.keys(changes).some((key) => key.startsWith(REQUEST_PREFIX))) this.#reconnectNow();
		};
		try {
			api?.storage?.onChanged?.addListener(this.#wake);
		} catch {
			this.#wake = null;
		}
		this.#ping = setInterval(() => {
			if (this.current) this.#post({ type: 'ping' });
		}, PING_MS);
		return true;
	}

	/** Close the port for good (a test, or a surface being torn down). */
	stop(): void {
		this.#stopped = true;
		if (this.#reconnectTimer) clearTimeout(this.#reconnectTimer);
		if (this.#ping) clearInterval(this.#ping);
		this.#reconnectTimer = null;
		this.#ping = null;
		if (this.#wake) {
			try {
				this.#api()?.storage?.onChanged?.removeListener?.(this.#wake);
			} catch {
				/* the page is going away */
			}
		}
		this.#wake = null;
		const port = this.#port;
		this.#port = null;
		try {
			port?.disconnect();
		} catch {
			/* already gone */
		}
		for (const resolve of this.#claims.values()) resolve(false);
		this.#claims.clear();
		this.#caller = null;
		this.current = null;
	}

	/** Called with `(rid, cause)` when the worker withdraws a request. */
	onWithdrawn(callback: Withdrawn): () => void {
		this.#withdrawn.add(callback);
		return () => this.#withdrawn.delete(callback);
	}

	/** The card or sheet for `rid` is on screen (`created` → `shown`). */
	shown(rid: string): void {
		this.#post({ type: 'shown', rid });
	}

	/**
	 * Is `rid` still live enough to act on (RB5)? Asked twice at most — the
	 * second time on a port reopened if the first one died (a worker restart);
	 * no answer to either within 5 s is "no".
	 */
	async claim(rid: string, phase: ClaimPhase): Promise<boolean> {
		if (!this.#caller) return false;
		for (let attempt = 0; attempt < 2; attempt += 1) {
			if (attempt > 0) this.#reconnectNow();
			const answer = await this.#claimOnce(rid, phase);
			if (answer !== null) return answer;
		}
		return false;
	}

	#claimOnce(rid: string, phase: ClaimPhase): Promise<boolean | null> {
		return new Promise((resolve) => {
			const nonce = (this.#nonce += 1);
			const timer = setTimeout(() => {
				this.#claims.delete(nonce);
				resolve(null);
			}, CLAIM_TIMEOUT_MS);
			this.#claims.set(nonce, (live) => {
				clearTimeout(timer);
				this.#claims.delete(nonce);
				resolve(live);
			});
			this.#post({ type: 'claim', rid, phase, nonce });
		});
	}

	/**
	 * Hand the answer back. Resolves `true` when the page took it; `false`
	 * when it did not (the page is gone) — the caller must act on that, not
	 * assume the dApp heard. `opHash` rides along with an answer that is an
	 * operation hash, so the worker can translate the page's receipt reads for
	 * it (RF3).
	 */
	async answer(
		rid: string,
		payload: SurfaceAnswer,
		opHash?: { chainId: number }
	): Promise<boolean> {
		const message = {
			type: 'answer',
			rid,
			...(payload.error ? { error: payload.error } : { result: payload.result }),
			...(opHash ? { opHash } : {})
		};
		if (this.#port) {
			const viaPort = await new Promise<boolean | null>((resolve) => {
				const timer = setTimeout(() => {
					this.#answers.delete(rid);
					resolve(null);
				}, ANSWER_TIMEOUT_MS);
				this.#answers.set(rid, (delivered) => {
					clearTimeout(timer);
					this.#answers.delete(rid);
					resolve(delivered);
				});
				if (!this.#post(message)) {
					clearTimeout(timer);
					this.#answers.delete(rid);
					resolve(null);
				}
			});
			if (viaPort !== null) {
				this.#settleCurrent(rid);
				return viaPort;
			}
		}
		// The port is down: the worker's message fallback (contract §14).
		const runtime = this.#runtime ?? this.#api()?.runtime ?? null;
		let delivered: boolean;
		try {
			const reply = (await runtime?.sendMessage({ ...message, type: 'requestAnswer' })) as {
				delivered?: boolean;
			} | null;
			delivered = reply?.delivered === true;
		} catch {
			delivered = false;
		}
		this.#settleCurrent(rid);
		return delivered;
	}

	#settleCurrent(rid: string): void {
		if (this.current?.rid === rid) this.current = null;
	}

	#post(message: unknown): boolean {
		if (!this.#port) {
			this.#reconnectNow();
		}
		if (!this.#port) return false;
		try {
			this.#port.postMessage(message);
			return true;
		} catch {
			return false;
		}
	}

	#connect(): void {
		if (this.#port || !this.#caller || this.#stopped) return;
		let port: PortLike;
		try {
			port = this.#runtime!.connect({ name: SURFACE_PORT });
		} catch {
			this.#scheduleReconnect();
			return;
		}
		this.#port = port;
		const upAt = Date.now();
		port.onMessage.addListener((message) => this.#onMessage(message));
		port.onDisconnect.addListener(() => {
			if (this.#port !== port) return;
			this.#port = null;
			if (Date.now() - upAt > 5_000) this.#failures = 0;
			this.#scheduleReconnect();
		});
		try {
			port.postMessage({ type: 'hello', ...this.#caller });
		} catch {
			/* the disconnect handler reconnects */
		}
	}

	/**
	 * Connect now if the port is down. A live port is never dropped to make a
	 * new one: the worker reads a surface port closing as Chrome's ✕ and
	 * settles what it owed (RB10).
	 */
	#reconnectNow(): void {
		if (this.#stopped || !this.#caller || this.#port) return;
		if (this.#reconnectTimer) {
			clearTimeout(this.#reconnectTimer);
			this.#reconnectTimer = null;
		}
		this.#connect();
	}

	#scheduleReconnect(): void {
		if (this.#stopped || this.#reconnectTimer || !this.#caller) return;
		const delay = RECONNECT_MS[Math.min(this.#failures, RECONNECT_MS.length - 1)];
		this.#failures += 1;
		this.#reconnectTimer = setTimeout(() => {
			this.#reconnectTimer = null;
			this.#connect();
		}, delay);
	}

	#onMessage(raw: unknown): void {
		if (!raw || typeof raw !== 'object') return;
		const message = raw as { type?: string; [key: string]: unknown };
		switch (message.type) {
			case 'owed':
				if (isRequest(message.request) && this.current?.rid !== message.request.rid) {
					this.current = message.request;
				}
				return;
			case 'withdrawn': {
				const rid = String(message.rid ?? '');
				if (this.current?.rid === rid) this.current = null;
				for (const callback of this.#withdrawn) {
					try {
						callback(rid, String(message.cause ?? ''));
					} catch {
						/* one listener's fault is not the others' */
					}
				}
				return;
			}
			case 'claimResult':
				this.#claims.get(Number(message.nonce))?.(message.live === true);
				return;
			case 'answered':
				this.#answers.get(String(message.rid ?? ''))?.(message.delivered === true);
				return;
			default:
				return;
		}
	}
}

/** The one surface of this document. */
export const panelSurface = new PanelSurface();
