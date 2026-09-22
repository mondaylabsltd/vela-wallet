/**
 * The two ways this wallet reaches a Clear Signer page (spec 075, contract §2).
 *
 * 071 opened the page for ONE signature and closed it. 075 makes the page a
 * passkey route, and a route carries a whole flow: create → member proof,
 * sign in → recover ×2. So a channel is a SESSION — it takes request after
 * request in order and ends with `bye` — and there are two of them:
 *
 * - **this device**: `postMessage` to a window this wallet opened. The only
 *   channel where the browser itself vouches for both origins, which is why
 *   the page lets a create through on it (PROTOCOL.md §10.2).
 * - **another device**: the relay (`contracts/relay.md`). The wallet draws a
 *   QR of the pairing link, the page joins the room, the two ends run the
 *   session in `relay/secure-session.ts`, and the person confirms the
 *   six-digit code on THIS side before anything is sent. The relay only ever
 *   sees ciphertext.
 *
 * What a channel decides is only what a transport must: who may speak, which
 * answer belongs to which request, and how a wait ends when no answer comes.
 * Whether an answer is a signature this wallet may use, or a registration it
 * may keep, is the core's — `clearSignerVerify` / `clearSignerVerifyCeremony`,
 * in the callers.
 */

import {
	clearSignerRelayLink,
	clearSignerRelayRoom,
	clearSignerRelayRoomUrl
} from '$lib/core/kernels';
import {
	completeSession,
	keyFingerprint,
	requesterHello,
	requesterKey,
	type RequesterKey,
	type SecureSession
} from './relay/secure-session';

/** One request's wait, as on every shell (contract §2–4). */
export const CLEAR_SIGNER_TIMEOUT_MS = 5 * 60_000;

/** How often a popup is checked for having been closed. */
const CLOSED_POLL_MS = 500;

/** One window, reused: "open the page again" brings back the same one. */
const WINDOW_NAME = 'vela-clear-signer';
const WINDOW_FEATURES = 'popup,width=460,height=760';

/** What this wallet calls itself in the relay hello (relay.md §2.2). */
const APP_NAME = 'vela-web/1';

/**
 * `<base>sign.html?ch=<channel>` — built the way the core's `sign_page` builds
 * every channel's address: a base that already names a page keeps it, one
 * ending in `/` gets `sign.html`, anything else `/sign.html`.
 */
export function signerPageUrl(base: string, channel = 'post'): string {
	const trimmed = base.trim();
	const page = trimmed.endsWith('.html')
		? trimmed
		: trimmed.endsWith('/')
			? `${trimmed}sign.html`
			: `${trimmed}/sign.html`;
	return `${page}?ch=${channel}`;
}

/**
 * What the page answered, or why it did not.
 *
 * `payload` is the page's own message — `{result}` for a signature (071),
 * `{registration|assertion, origin}` for a ceremony (075 §10.3) — handed on
 * whole, because the core reads the fields it needs and nothing here is
 * entitled to an opinion about them.
 *
 * `code` is the page's (`user_rejected` → `declined`, anything else →
 * `refused`) or the channel's own: `declined` (the page was closed or the
 * person cancelled), `timeout`, and `relay_down` (the room could not be
 * reached or was lost for good).
 */
export type ClearSignerReply =
	| { kind: 'answer'; payload: Record<string, unknown> }
	| { kind: 'refused'; code: string; detail: string };

/** A session with the page: several requests in order, then `bye`. */
export interface ClearSignerChannel {
	/**
	 * Send one request and wait for its answer. Never rejects; a channel that
	 * has ended answers at once with why.
	 */
	ask(request: { intent: unknown; context: unknown }): Promise<ClearSignerReply>;
	/** Say goodbye and let the page go. Idempotent. */
	end(): void;
	/** The person cancelled: the request in flight is declined and the session ends. */
	cancel(): void;
	/** "Open the page again" — this-device only; the relay has nothing to reopen. */
	reopen(): void;
	/** Ended: nothing more can be asked of it. */
	readonly ended: boolean;
}

/** What the channel needs of the browser — `window`, or a test's stand-in. */
export interface ClearSignerHost {
	open(url: string, target: string, features: string): Window | null;
	addEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
	removeEventListener(type: 'message', listener: (event: MessageEvent) => void): void;
}

export interface PostMessageChannelOptions {
	/** The page the person chose (`SignPrefView.signer_url`), or a key's own. */
	signerUrl: string;
	host?: ClearSignerHost;
	/** Per request, not per session. */
	timeoutMs?: number;
}

/** One request being waited on, and how its wait ends. */
interface Pending {
	id: string;
	request: { intent: unknown; context: unknown };
	settle(reply: ClearSignerReply): void;
	timer: ReturnType<typeof setTimeout>;
}

/**
 * The same-device channel: a window this wallet opened, and `postMessage`
 * both ways (071 contract §4, extended to a session by 075 §1.5).
 *
 * ```text
 *   window.open(<page>sign.html?ch=post)
 *   page   → {vela:'ready'}                       from the page's origin, from that window
 *   wallet → {vela:'intent', id, intent, context} to that origin only
 *   page   → {vela:'result', id, …} | {vela:'error', id, code}
 *   wallet → the next intent, on the same page …
 *   wallet → {vela:'bye'}                         the flow is over
 * ```
 */
export function openPostMessageChannel(options: PostMessageChannelOptions): ClearSignerChannel {
	const host: ClearSignerHost = options.host ?? window;
	const url = signerPageUrl(options.signerUrl);
	const origin = new URL(url).origin;
	const timeoutMs = options.timeoutMs ?? CLEAR_SIGNER_TIMEOUT_MS;

	let popup: Window | null = null;
	let pending: Pending | null = null;
	let closedSeen = false;
	let ended = false;
	/** The page's scripts are up: it has said `ready` at least once. */
	let ready = false;

	function post(message: Record<string, unknown>): void {
		try {
			popup?.postMessage(message, origin);
		} catch {
			/* a window that is gone answers nothing */
		}
	}

	function settle(reply: ClearSignerReply): void {
		const waiting = pending;
		if (waiting === null) return;
		pending = null;
		clearTimeout(waiting.timer);
		waiting.settle(reply);
	}

	function onMessage(event: MessageEvent): void {
		if (ended) return;
		// Both, never one: the origin says WHICH SITE is speaking, the source
		// says it is the window this wallet opened — not another tab of the
		// same site, and not a frame it embeds.
		if (event.origin !== origin || popup === null || event.source !== popup) return;
		const data = event.data as { vela?: unknown; id?: unknown; code?: unknown } | null;
		if (!data || typeof data !== 'object') return;
		if (data.vela === 'ready') {
			ready = true;
			// Every `ready` gets the request in flight: a page that reloaded has
			// forgotten it, and one that has nothing to do is simply waiting.
			if (pending !== null) sendIntent(pending);
			return;
		}
		if (data.vela === 'bye') {
			// The page ended the session (its five-minute idle, or a person
			// leaving it). Anything in flight is unanswered.
			finish('declined', 'the page said goodbye');
			return;
		}
		if (pending === null || data.id !== pending.id) return;
		if (data.vela === 'result') {
			settle({ kind: 'answer', payload: data as Record<string, unknown> });
		} else if (data.vela === 'error') {
			// The page's own vocabulary (PROTOCOL.md): `user_rejected` is the
			// person, anything else is the page's rules refusing.
			const code = typeof data.code === 'string' ? data.code : '';
			if (code === '' || code === 'user_rejected')
				settle({ kind: 'refused', code: 'declined', detail: '' });
			else settle({ kind: 'refused', code: 'refused', detail: code });
		}
	}

	function sendIntent(waiting: Pending): void {
		post({
			vela: 'intent',
			id: waiting.id,
			intent: waiting.request.intent,
			context: waiting.request.context
		});
	}

	function open(): void {
		popup = host.open(url, WINDOW_NAME, WINDOW_FEATURES);
		closedSeen = false;
	}

	/** End the session for good; whatever was in flight ends with `code`. */
	function finish(code: string, detail: string): void {
		if (ended) return;
		ended = true;
		settle({ kind: 'refused', code, detail });
		host.removeEventListener('message', onMessage);
		clearInterval(poll);
		try {
			if (popup && !popup.closed) popup.close();
		} catch {
			/* a window we cannot close is the browser's to keep */
		}
	}

	host.addEventListener('message', onMessage);
	const poll = setInterval(() => {
		if (ended) return;
		if (popup === null || !popup.closed) {
			closedSeen = false;
			return;
		}
		// One more tick first: an answer posted as the page closed may still be
		// queued behind this check, and it must win over "closed".
		if (closedSeen) finish('declined', 'the page was closed');
		closedSeen = true;
	}, CLOSED_POLL_MS);
	open();

	return {
		get ended() {
			return ended;
		},
		ask(request) {
			if (ended) return Promise.resolve(refusal('declined', 'the page is gone'));
			if (pending !== null) {
				// One at a time, by construction: every flow that uses a session
				// asks for the next ceremony only after the last one answered.
				return Promise.resolve(refusal('malformed', 'a request is already in flight'));
			}
			return new Promise<ClearSignerReply>((resolve) => {
				const waiting: Pending = {
					id: crypto.randomUUID(),
					request,
					settle: resolve,
					timer: setTimeout(() => settle(refusal('timeout', '')), timeoutMs)
				};
				pending = waiting;
				// A page that is still loading is not listening yet — it will say
				// `ready`, and every `ready` is answered with what is in flight.
				// One already up says it only once, so this request goes now.
				if (ready) sendIntent(waiting);
			});
		},
		end() {
			if (ended) return;
			post({ vela: 'bye', v: 1, reason: 'done' });
			finish('declined', 'the session ended');
		},
		cancel() {
			finish('declined', 'cancelled');
		},
		reopen() {
			if (ended) return;
			if (popup !== null && !popup.closed) popup.focus();
			else open();
		}
	};
}

function refusal(code: string, detail: string): ClearSignerReply {
	return { kind: 'refused', code, detail };
}

// ---------------------------------------------------------------------------
// The relay — another device (contracts/relay.md)
// ---------------------------------------------------------------------------

/** What the relay channel needs of a WebSocket; `window.WebSocket`, or a test's. */
export interface RelaySocket {
	send(data: string | ArrayBufferView): void;
	close(code?: number, reason?: string): void;
	onopen: (() => void) | null;
	onmessage: ((event: { data: unknown }) => void) | null;
	onclose: ((event: { code: number }) => void) | null;
	onerror: (() => void) | null;
	binaryType?: string;
}

export interface RelayChannelOptions {
	/** The relay (`SignPrefView.relay_url`). */
	relayUrl: string;
	/** The page the other device is asked to open. */
	signerUrl: string;
	/** The pairing link is ready to be shown as a QR and copied. */
	onLink(link: string): void;
	/**
	 * The two ends derived a session: show these six digits beside the page's
	 * own and wait for `confirm()`. `null` means the pairing is gone again (the
	 * other device dropped out) and the wallet is waiting for it to come back.
	 */
	onCode(code: string | null): void;
	/** The room could not be reached, or was lost for good. */
	onDown?(reason: string): void;
	sockets?: (url: string) => RelaySocket;
	timeoutMs?: number;
}

/** The relay channel, plus the one thing only a person can do: confirm the code. */
export interface RelayChannel extends ClearSignerChannel {
	/** The person says both screens show the same digits. Nothing is sent before this. */
	confirm(): void;
	/** The link this pairing is for — a QR, and copyable. */
	readonly link: string;
}

/**
 * Pair with a Clear Signer page on another device and carry a session to it.
 *
 * Nothing is sent until `confirm()`: the six-digit code is the only thing
 * standing between this wallet and a page that is not the one the person
 * opened (relay.md §2.4), and for a CREATE that matters most — a substituted
 * page would hand the wallet somebody else's key.
 */
export async function openRelayChannel(options: RelayChannelOptions): Promise<RelayChannel> {
	const timeoutMs = options.timeoutMs ?? CLEAR_SIGNER_TIMEOUT_MS;
	const random = new Uint8Array(16);
	crypto.getRandomValues(random);
	const room = clearSignerRelayRoom(random);
	if (room === undefined) throw new Error('the relay room could not be built');
	const key: RequesterKey = await requesterKey();
	const rk = await keyFingerprint(key.publicKey);
	const link = clearSignerRelayLink(options.signerUrl, options.relayUrl, room, rk);

	let session: SecureSession | null = null;
	let confirmed = false;
	let pending: Pending | null = null;
	let ended = false;
	/** `n` on the wire: this side's last, and the highest seen from the page. */
	let outgoing = 0;
	let lastSeen = 0;

	const socket: RelaySocket = (options.sockets ?? defaultSocket)(
		clearSignerRelayRoomUrl(options.relayUrl, room)
	);
	socket.binaryType = 'arraybuffer';

	function settle(reply: ClearSignerReply): void {
		const waiting = pending;
		if (waiting === null) return;
		pending = null;
		clearTimeout(waiting.timer);
		waiting.settle(reply);
	}

	function finish(code: string, detail: string): void {
		if (ended) return;
		ended = true;
		session = null;
		settle({ kind: 'refused', code, detail });
		try {
			socket.close(1000, 'bye');
		} catch {
			/* already gone */
		}
	}

	async function send(message: Record<string, unknown>): Promise<void> {
		const live = session;
		if (live === null) throw new Error('the pairing is not up');
		outgoing = Math.max(outgoing, lastSeen) + 1;
		const sealed = await live.seal(
			new TextEncoder().encode(JSON.stringify({ v: 1, ...message, n: outgoing }))
		);
		socket.send(sealed);
	}

	/** The request in flight goes out as soon as there is a confirmed session. */
	function flush(): void {
		if (!confirmed || session === null || pending === null) return;
		const { id, request } = pending;
		void send({ t: 'intent', id, intent: request.intent, context: request.context }).catch(() =>
			finish('relay_down', 'the request could not be sent')
		);
	}

	async function onHello(hello: Record<string, unknown>): Promise<void> {
		try {
			const nonce = new Uint8Array(16);
			crypto.getRandomValues(nonce);
			socket.send(JSON.stringify(requesterHello(key, nonce, APP_NAME)));
			const next = await completeSession(key, nonce, hello);
			if (ended) return;
			session = next;
			// Every pairing is confirmed on its own: a page that dropped out and
			// came back is a new session, a new code, and a new look from the
			// person (relay.md §2.4).
			confirmed = false;
			outgoing = 0;
			lastSeen = 0;
			options.onCode(next.code);
		} catch {
			finish('relay_down', 'the pairing could not be established');
		}
	}

	async function onSealed(data: ArrayBuffer): Promise<void> {
		const live = session;
		if (live === null) return;
		let message: Record<string, unknown>;
		try {
			const plain = await live.open(new Uint8Array(data));
			message = JSON.parse(new TextDecoder().decode(plain)) as Record<string, unknown>;
		} catch {
			// Replayed, reordered or unreadable: dropped, never delivered.
			return;
		}
		if (live !== session || ended) return;
		const n = message.n;
		if (typeof n !== 'number' || n <= lastSeen) return;
		lastSeen = n;
		if (message.t === 'bye') {
			finish('declined', 'the page said goodbye');
			return;
		}
		if (pending === null || message.id !== pending.id) return;
		if (message.t === 'result') settle({ kind: 'answer', payload: message });
		else if (message.t === 'error') {
			const code = typeof message.code === 'string' ? message.code : '';
			if (code === '' || code === 'user_rejected') settle(refusal('declined', ''));
			else settle(refusal('refused', code));
		}
	}

	socket.onmessage = (event) => {
		if (ended) return;
		const data = event.data;
		if (typeof data === 'string') {
			let frame: Record<string, unknown> | null;
			try {
				frame = JSON.parse(data) as Record<string, unknown>;
			} catch {
				return;
			}
			if (!frame || typeof frame !== 'object') return;
			// The relay's own frames carry a `relay` key; an end's never do.
			if (Object.prototype.hasOwnProperty.call(frame, 'relay')) {
				if (frame.relay === 'left') {
					// The other device is gone; the room waits for it. Whatever was
					// in flight can no longer be answered there, so it is re-sent
					// when the pairing comes back — after a fresh code.
					session = null;
					confirmed = false;
					options.onCode(null);
				}
				return;
			}
			if (frame.t === 'hello' && session === null) void onHello(frame);
			return;
		}
		if (data instanceof ArrayBuffer) void onSealed(data);
	};
	socket.onclose = (event) => {
		// 4408 expired/idle, 4409 role taken, 1009 too big: all of them mean
		// this pairing is over, and the person is told so rather than left
		// looking at a code nobody will match.
		finish('relay_down', `the relay closed (${event.code})`);
	};
	socket.onerror = () => {
		/* `onclose` follows with the code */
	};
	options.onLink(link);

	return {
		link,
		get ended() {
			return ended;
		},
		ask(request) {
			if (ended) return Promise.resolve(refusal('relay_down', 'the pairing is over'));
			if (pending !== null) {
				return Promise.resolve(refusal('malformed', 'a request is already in flight'));
			}
			return new Promise<ClearSignerReply>((resolve) => {
				pending = {
					id: crypto.randomUUID(),
					request,
					settle: resolve,
					timer: setTimeout(() => settle(refusal('timeout', '')), timeoutMs)
				};
				flush();
			});
		},
		confirm() {
			if (ended || session === null || confirmed) return;
			confirmed = true;
			flush();
		},
		end() {
			if (ended) return;
			const said = session !== null && confirmed ? send({ t: 'bye', reason: 'done' }) : null;
			if (said === null) finish('declined', 'the session ended');
			else
				void said.then(
					() => finish('declined', 'the session ended'),
					() => finish('declined', 'the session ended')
				);
		},
		cancel() {
			finish('declined', 'cancelled');
		},
		reopen() {
			/* nothing to reopen: the page lives on another device */
		}
	};
}

function defaultSocket(url: string): RelaySocket {
	return new WebSocket(url) as unknown as RelaySocket;
}
