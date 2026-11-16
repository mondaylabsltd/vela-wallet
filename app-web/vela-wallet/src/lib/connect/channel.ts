/**
 * How a site's requests reach Vela Connect's window, and its answers get back
 * (spec 109): the popup's `postMessage`, or the redirect's URL fragment.
 *
 * The rules that keep this safe live here and nowhere else:
 *
 *   - **popup** — a message counts only if the window's OPENER sent it
 *     (`event.source === window.opener`). The first one fixes the requester:
 *     its `event.origin`, the browser's fact. From then on a message from any
 *     other origin is dropped, and every answer is posted to that origin
 *     alone. The pin survives the trip through creating a wallet
 *     (`sessionStorage`, this window's own), so a window opened by one site
 *     never serves another.
 *   - **redirect** — the requester is the origin of `return`, which is where
 *     the answer goes and the only thing Vela can say about who asked
 *     (`verified: false`: the window tells the person so).
 */
import {
	readInbound,
	readRedirect,
	readyMessage,
	redirectBack,
	responseMessage,
	type ConnectRequest,
	type RedirectRequest,
	type WireAnswer
} from './protocol';

/** A request in the window, with who asked it. */
export interface ChannelRequest extends ConnectRequest {
	/** The window's own id for it: unique while the window lives. */
	rid: string;
	/** The requester — see `Channel.verified` for how sure that is. */
	origin: string;
}

export interface Channel {
	readonly kind: 'popup' | 'redirect';
	/** `true`: the browser vouches for the origin. `false`: a link says it. */
	readonly verified: boolean;
	/** The requester, once known. */
	readonly origin: string | null;
	/** Start taking requests; `onRequest` gets each one once. */
	start(onRequest: (request: ChannelRequest) => void, onInvalid?: () => void): void;
	/** Send `rid`'s answer. `true` when the site can still take it. */
	answer(rid: string, answer: WireAnswer): Promise<boolean>;
	/** Is the site still there to answer? */
	alive(): boolean;
	stop(): void;
}

/** The window's pinned requester (popup), kept across its own navigations. */
export const CONNECT_ORIGIN_KEY = 'vela.connect.origin';
/** A redirect request in flight, kept across the trip through Welcome. */
export const CONNECT_REDIRECT_KEY = 'vela.connect.redirect';
/** Set while this window went to create a wallet or sign in: come back to `/dapp`. */
export const CONNECT_RETURN_KEY = 'vela.connect.return';

function session(): Storage | null {
	try {
		return globalThis.sessionStorage ?? null;
	} catch {
		return null;
	}
}

/** An origin a request may be pinned to: a real one, never `"null"`. */
function pinnable(origin: string): boolean {
	return /^https?:\/\/[^/]+$/.test(origin);
}

// ---------------------------------------------------------------------------
// Popup
// ---------------------------------------------------------------------------

export class PopupChannel implements Channel {
	readonly kind = 'popup';
	readonly verified = true;
	#origin: string | null;
	#seen = new Set<string>();
	#listener: ((event: MessageEvent) => void) | null = null;
	#next = 0;

	constructor(private readonly win: Window = window) {
		const kept = session()?.getItem(CONNECT_ORIGIN_KEY) ?? null;
		this.#origin = kept && pinnable(kept) ? kept : null;
	}

	/** Is there an opener to take requests from at all? */
	static available(win: Window = window): boolean {
		return !!win.opener && win.opener !== win;
	}

	get origin(): string | null {
		return this.#origin;
	}

	start(onRequest: (request: ChannelRequest) => void, onInvalid?: () => void): void {
		const listener = (event: MessageEvent) => {
			const opener = this.win.opener as Window | null;
			if (!opener || event.source !== opener) return;
			if (!pinnable(event.origin)) return;
			if (this.#origin !== null && event.origin !== this.#origin) return;
			const read = readInbound(event.data);
			if (read.kind === 'ignore') return;
			this.#pin(event.origin);
			if (read.kind === 'invalid') {
				opener.postMessage(responseMessage(read.id, { error: read.error }), event.origin);
				onInvalid?.();
				return;
			}
			const { request } = read;
			// One answer per id: an id already taken in this document is the
			// site sending it twice, not a second request.
			if (this.#seen.has(request.id)) return;
			this.#seen.add(request.id);
			onRequest({ ...request, rid: `c${this.#next++}:${request.id}`, origin: event.origin });
		};
		this.#listener = listener;
		this.win.addEventListener('message', listener);
		// Nothing secret in it, and the opener's origin is not known yet.
		(this.win.opener as Window | null)?.postMessage(readyMessage(), '*');
	}

	#pin(origin: string): void {
		if (this.#origin !== null) return;
		this.#origin = origin;
		try {
			session()?.setItem(CONNECT_ORIGIN_KEY, origin);
		} catch {
			/* kept for this document only */
		}
	}

	async answer(rid: string, answer: WireAnswer): Promise<boolean> {
		const opener = this.win.opener as Window | null;
		if (!opener || opener.closed || this.#origin === null) return false;
		const id = rid.slice(rid.indexOf(':') + 1);
		try {
			opener.postMessage(responseMessage(id, answer), this.#origin);
			return true;
		} catch {
			return false;
		}
	}

	alive(): boolean {
		const opener = this.win.opener as Window | null;
		return !!opener && !opener.closed;
	}

	stop(): void {
		if (this.#listener) this.win.removeEventListener('message', this.#listener);
		this.#listener = null;
	}
}

// ---------------------------------------------------------------------------
// Redirect
// ---------------------------------------------------------------------------

export class RedirectChannel implements Channel {
	readonly kind = 'redirect';
	readonly verified = false;
	#done = false;

	private constructor(
		private readonly redirect: RedirectRequest,
		private readonly navigate: (url: string) => void
	) {}

	/**
	 * The redirect request in `hash`, else the one this window kept (it went
	 * to create a wallet and came back); `'invalid'` for a fragment that names
	 * one and cannot be read; `null` for none.
	 */
	static open(
		hash: string,
		navigate: (url: string) => void = (url) => location.replace(url),
		now = Date.now()
	): RedirectChannel | 'invalid' | null {
		let read = readRedirect(hash);
		if (read.ok) {
			// Kept as the site wrote it, and read again by the same rules when
			// the window comes back from creating a wallet — for as long as that
			// takes, not for a link left behind and found later.
			try {
				session()?.setItem(CONNECT_REDIRECT_KEY, JSON.stringify({ hash, at: now }));
			} catch {
				/* kept in the fragment only */
			}
		} else if (read.reason === 'none') {
			read = readRedirect(RedirectChannel.#kept(now));
		} else {
			RedirectChannel.forget();
		}
		if (read.ok) return new RedirectChannel(read.redirect, navigate);
		return read.reason === 'invalid' ? 'invalid' : null;
	}

	/** The kept fragment, while it is fresh; `''` otherwise (and it is dropped). */
	static #kept(now: number): string {
		try {
			const kept = JSON.parse(session()?.getItem(CONNECT_REDIRECT_KEY) ?? 'null') as {
				hash?: unknown;
				at?: unknown;
			} | null;
			if (
				kept &&
				typeof kept.hash === 'string' &&
				typeof kept.at === 'number' &&
				now - kept.at < CONNECT_RETURN_MS &&
				now >= kept.at - 60_000
			) {
				return kept.hash;
			}
		} catch {
			/* unreadable: none */
		}
		RedirectChannel.forget();
		return '';
	}

	/** Drop a kept redirect request. */
	static forget(): void {
		try {
			session()?.removeItem(CONNECT_REDIRECT_KEY);
		} catch {
			/* nothing kept */
		}
	}

	get origin(): string {
		return this.redirect.origin;
	}

	start(onRequest: (request: ChannelRequest) => void): void {
		const { request, origin } = this.redirect;
		onRequest({ ...request, rid: `r0:${request.id}`, origin });
	}

	async answer(_rid: string, answer: WireAnswer): Promise<boolean> {
		if (this.#done) return false;
		this.#done = true;
		RedirectChannel.forget();
		this.navigate(redirectBack(this.redirect.returnUrl, this.redirect.state, answer));
		return true;
	}

	/** The site is wherever `return` is: always there to go back to. */
	alive(): boolean {
		return !this.#done;
	}

	stop(): void {}
}

/**
 * This window went to create a wallet or sign in for a site's request:
 * remember to come back to `/dapp` once the wallet opens.
 */
export function markConnectReturn(): void {
	try {
		session()?.setItem(CONNECT_RETURN_KEY, String(Date.now()));
	} catch {
		/* the person comes back by the site asking again */
	}
}

/** Within this long of leaving, the wallet sends the window back to `/dapp`. */
export const CONNECT_RETURN_MS = 30 * 60_000;

/**
 * Taken once by the wallet page when it opens: `true` when this window left
 * a site's request to create a wallet or sign in, not long ago.
 */
export function takeConnectReturn(now = Date.now()): boolean {
	const store = session();
	if (!store) return false;
	let at: number;
	try {
		at = Number(store.getItem(CONNECT_RETURN_KEY));
		store.removeItem(CONNECT_RETURN_KEY);
	} catch {
		return false;
	}
	return Number.isFinite(at) && at > 0 && now - at < CONNECT_RETURN_MS && now >= at - 60_000;
}
