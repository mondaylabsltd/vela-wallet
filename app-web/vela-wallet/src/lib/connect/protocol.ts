/**
 * Vela Connect v1 — the wire (spec 109, `docs/vela-connect/PROTOCOL.md`).
 *
 * Pure: no DOM, no storage, no wasm. What a message from a site IS, and what
 * an answer to it looks like, for both ways a site opens Vela — a popup
 * (`postMessage`) and a redirect (the URL fragment). Nothing here decides
 * whether a request may be granted or signed; the core does (`ConnectHost`).
 *
 * Everything a site sends is untrusted: a message is read field by field and
 * anything it does not say the way the protocol says it is refused, never
 * guessed at.
 */

/** `vela` in every message. */
export const PROTOCOL_VERSION = 1;

/** At most this much JSON in one request's params (the extension's own cap). */
export const MAX_REQUEST_BYTES = 512 * 1024;

/** A request id: the site's own, echoed back. */
const ID = /^[\x21-\x7e]{1,64}$/;

/** A redirect's `state`. */
const STATE = /^[A-Za-z0-9_-]{16,256}$/;

/** One request, as the core and the signing machine take it. */
export interface ConnectRequest {
	/** The site's id, echoed in the answer. */
	id: string;
	method: string;
	params: unknown[];
}

/** EIP-1193's error object. */
export interface WireError {
	code: number;
	message: string;
	data?: unknown;
}

export interface WireAnswer {
	result?: unknown;
	error?: WireError;
}

/** -32600: not a request this protocol understands. */
export const INVALID_REQUEST = -32600;

export type Inbound =
	| { kind: 'request'; request: ConnectRequest }
	/** Answerable, but not a request we take: the id is good, the rest is not. */
	| { kind: 'invalid'; id: string; error: WireError }
	/** Not ours, or not answerable: dropped without a word. */
	| { kind: 'ignore' };

function isObject(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Bytes of `value` as JSON, or `Infinity` for something that is not JSON. */
function jsonBytes(value: unknown): number {
	try {
		return new TextEncoder().encode(JSON.stringify(value) ?? '').length;
	} catch {
		return Infinity;
	}
}

/**
 * Read one popup message (`MessageEvent.data`). Only `{vela, type:"request"}`
 * is ours: any other message — the page's own traffic, another wallet's —
 * is ignored, not answered.
 */
export function readInbound(data: unknown): Inbound {
	if (!isObject(data) || data.type !== 'request' || !('vela' in data)) return { kind: 'ignore' };
	const id = data.id;
	if (typeof id !== 'string' || !ID.test(id)) return { kind: 'ignore' };
	if (data.vela !== PROTOCOL_VERSION) {
		return invalid(id, `Unsupported Vela Connect version ${String(data.vela)}; this is v1`);
	}
	return readRequestBody(id, data.method, data.params);
}

function invalid(id: string, message: string): Inbound {
	return { kind: 'invalid', id, error: { code: INVALID_REQUEST, message } };
}

function readRequestBody(id: string, method: unknown, params: unknown): Inbound {
	if (typeof method !== 'string' || method.length === 0 || method.length > 128) {
		return invalid(id, 'Expected a method');
	}
	if (params !== undefined && !Array.isArray(params)) {
		return invalid(id, 'Expected params to be an array');
	}
	const list = (params ?? []) as unknown[];
	if (jsonBytes(list) > MAX_REQUEST_BYTES) return invalid(id, 'The request is too large');
	return { kind: 'request', request: { id, method, params: list } };
}

/** What Vela posts to its opener when it can take requests. */
export function readyMessage(): { vela: number; type: 'ready' } {
	return { vela: PROTOCOL_VERSION, type: 'ready' };
}

/** The answer to request `id`, as posted. */
export function responseMessage(
	id: string,
	answer: WireAnswer
): { vela: number; type: 'response'; id: string; result?: unknown; error?: WireError } {
	if (answer.error) {
		const error: WireError = { code: answer.error.code, message: answer.error.message };
		if (answer.error.data !== undefined) error.data = answer.error.data;
		return { vela: PROTOCOL_VERSION, type: 'response', id, error };
	}
	return {
		vela: PROTOCOL_VERSION,
		type: 'response',
		id,
		result: answer.result === undefined ? null : answer.result
	};
}

// ---------------------------------------------------------------------------
// base64url
// ---------------------------------------------------------------------------

export function base64UrlEncodeJson(value: unknown): string {
	const bytes = new TextEncoder().encode(JSON.stringify(value));
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function base64UrlDecodeJson(text: string): unknown {
	if (!/^[A-Za-z0-9_-]*$/.test(text)) throw new Error('not base64url');
	const padded = text.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded + '='.repeat((4 - (padded.length % 4)) % 4));
	const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
	return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
}

// ---------------------------------------------------------------------------
// Redirect mode
// ---------------------------------------------------------------------------

/** A redirect request, read. */
export interface RedirectRequest {
	request: ConnectRequest;
	/** Where the answer goes. */
	returnUrl: string;
	/** `returnUrl`'s origin: the requester, as far as anyone can tell. */
	origin: string;
	state: string;
}

export type RedirectReading =
	| { ok: true; redirect: RedirectRequest }
	/** No redirect request in the fragment at all: the popup's page. */
	| { ok: false; reason: 'none' }
	/** A redirect request that cannot be taken; nothing is sent anywhere. */
	| { ok: false; reason: 'invalid' };

/** The id a redirect request is answered under (it carries none of its own). */
export const REDIRECT_REQUEST_ID = 'redirect';

/**
 * Whether `url` may be returned to: `https:`, or `http:` on this machine
 * (`localhost`, `127.0.0.1`, `[::1]`) for development; never with a user
 * name or password in it. Its origin, or `null`.
 */
export function returnOrigin(url: string): string | null {
	let parsed: URL;
	try {
		parsed = new URL(url);
	} catch {
		return null;
	}
	if (parsed.username || parsed.password) return null;
	const local =
		parsed.hostname === 'localhost' ||
		parsed.hostname === '127.0.0.1' ||
		parsed.hostname === '[::1]';
	if (parsed.protocol === 'https:' || (parsed.protocol === 'http:' && local)) {
		return parsed.origin;
	}
	return null;
}

/** Read `location.hash` for a redirect request (`#request=…&return=…&state=…`). */
export function readRedirect(hash: string): RedirectReading {
	const fragment = hash.startsWith('#') ? hash.slice(1) : hash;
	if (!fragment) return { ok: false, reason: 'none' };
	const params = new URLSearchParams(fragment);
	if (!params.has('request') && !params.has('return')) return { ok: false, reason: 'none' };
	const rawRequest = params.get('request') ?? '';
	const returnUrl = params.get('return') ?? '';
	const state = params.get('state') ?? '';
	const origin = returnOrigin(returnUrl);
	if (!origin || !STATE.test(state)) return { ok: false, reason: 'invalid' };
	let body: unknown;
	try {
		body = base64UrlDecodeJson(rawRequest);
	} catch {
		return { ok: false, reason: 'invalid' };
	}
	if (!isObject(body)) return { ok: false, reason: 'invalid' };
	const read = readRequestBody(REDIRECT_REQUEST_ID, body.method, body.params);
	if (read.kind !== 'request') return { ok: false, reason: 'invalid' };
	return { ok: true, redirect: { request: read.request, returnUrl, origin, state } };
}

/** `returnUrl` with the answer as its fragment (any fragment it had replaced). */
export function redirectBack(returnUrl: string, state: string, answer: WireAnswer): string {
	const url = new URL(returnUrl);
	const { vela, result, error } = responseMessage(REDIRECT_REQUEST_ID, answer);
	const body = error ? { vela, state, error } : { vela, state, result };
	url.hash = `vela=${base64UrlEncodeJson(body)}`;
	return url.toString();
}

/**
 * Why redirect mode does not take `method`, or `null` when it does (spec 109,
 * security review 2026-11-17). A redirect's requester is only what its link
 * says, so:
 *
 *   - **no message signatures**: a signature is a credential (Sign-In with
 *     Ethereum above all), and a redirect carries it in a URL fragment, which
 *     a site's own open redirect passes on to anyone. Popup mode, where the
 *     browser vouches for the requester, signs messages;
 *   - **nothing that changes a site's standing without the person**: a link
 *     must not disconnect a site, or move it to another network, by being
 *     opened.
 */
export function redirectRefusal(method: string): string | null {
	if (
		method === 'personal_sign' ||
		method === 'eth_sign' ||
		method.startsWith('eth_signTypedData')
	) {
		return 'Message signatures need popup mode: a redirect cannot show who is asking';
	}
	if (
		method === 'wallet_revokePermissions' ||
		method === 'wallet_switchEthereumChain' ||
		method === 'wallet_addEthereumChain'
	) {
		return `${method} needs popup mode: a link cannot change a site's connection`;
	}
	return null;
}
