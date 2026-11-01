/**
 * The extension's request channel, wallet side (spec 027 T321).
 *
 * This is the FIRST real transport on the seam 026 built. `sign_request` holds
 * a registry and answers each request to the transport that OWNS it; until now
 * the only occupant was a test requester on the wallet's own page. Nothing
 * above this file changes: the signing sheet, the four machines and the submit
 * spine are 026's, unchanged.
 *
 * Ported from src/services/extension-bridge-transport.ts @ 52ad8fa9, against
 * the `DAppTransport` shape in src/services/dapp-transport.ts. The Expo version
 * speaks to a Safari extension over `nativeMessaging`; this one speaks to its
 * own service worker, which is a shorter and more honest path — there is no
 * second process holding a copy of the answer.
 *
 * The promises it keeps are the channel contract's:
 *   - one answer, once (a settled request cannot be re-answered);
 *   - silence is never a clean refusal (a window closed without a decision
 *     answers 4900 — the request may have landed; only Cancel is 4001);
 *   - the wallet's own record outlives the page's answer (026's persist-at-
 *     submit ordering is untouched, so an operation that was submitted is in
 *     the activity feed whether or not the dApp ever heard back).
 */

import { panelSurface } from './panel-surface.svelte';

/** What the service worker wrote down when the request arrived. */
export interface ExtensionRequest {
	/** The background's id for this request: `<tabId>:<page id>`. */
	rid: string;
	/** The id the PAGE used. Echoed back so the page can correlate. */
	id: string;
	method: string;
	params: unknown[];
	/** The browser's fact about who asked — never the page's claim. */
	origin: string;
	tabId: number;
	at: number;
	/**
	 * r10 follow-up: a network-space switch moved this site since its last
	 * request, to `chainId` — the sheet this request opens says so once
	 * (`connect.browser.spaceMoved`).
	 */
	spaceNote?: { chainId: number };
}

/** EIP-1193's refusal, and the one a dismissed window produces. */
export const USER_REJECTED = 4001;

interface RuntimeMessage {
	type: string;
	[key: string]: unknown;
}

interface ChromeRuntimeLike {
	sendMessage(message: RuntimeMessage): Promise<unknown>;
}

/** The extension runtime, when this page is running inside one. */
function runtime(): ChromeRuntimeLike | null {
	const api = (globalThis as { chrome?: { runtime?: unknown } }).chrome?.runtime;
	if (!api || typeof (api as ChromeRuntimeLike).sendMessage !== 'function') return null;
	return api as ChromeRuntimeLike;
}

/** True when this document is a page of the packaged extension. */
export function inExtension(): boolean {
	return runtime() !== null && location.protocol === __VELA_EXTENSION_PROTOCOL__;
}

/**
 * The request this window was opened to answer, or `null` when there is none —
 * which is the normal state of every page except the request window.
 */
export async function readRequest(rid: string): Promise<ExtensionRequest | null> {
	const api = runtime();
	if (!api || !rid) return null;
	try {
		const detail = await api.sendMessage({ type: 'requestDetail', rid });
		return isRequest(detail) ? detail : null;
	} catch {
		// The worker was evicted and could not be woken. The window has nothing
		// to show, and closing it answers 4900 (the core's `browser_closed`) —
		// never 4001, which a dApp reads as "nothing happened, safe to retry".
		return null;
	}
}

/**
 * What an answer that names an operation rides with, for the worker
 * (`extension/lib/op-receipt.js`): the chain it went to — and, for a batch a
 * multisig answered with (its id the proposal's safeTxHash), the multisig,
 * the proposal's number and the block it was proposed at, which is where
 * `wallet_getCallsStatus` reads it from.
 */
export interface AnsweredOp {
	chainId: number;
	multisig?: { safe: string; nonce: string; fromBlock?: string };
}

/**
 * Hand the answer back — through the surface's port when it is up (spec 082
 * contract §14), else the worker's message fallback.
 *
 * Resolves `true` only when the PAGE took it. `false` is not a detail to
 * ignore (before 082 it was, G17): the asking page is gone, the dApp heard
 * nothing, and the surface must stop showing a request nobody is waiting on.
 * Settling twice is the worker's job to refuse.
 */
export async function answerRequest(
	rid: string,
	answer: { result?: unknown; error?: { code: number; message: string; data?: unknown } },
	opHash?: AnsweredOp
): Promise<boolean> {
	if (panelSurface.caller) {
		const delivered = await panelSurface.answer(rid, answer, opHash);
		if (!delivered) console.info(`[dapp] answer not delivered rid=${rid}`);
		return delivered;
	}
	const api = runtime();
	if (!api) return false;
	try {
		const reply = (await api.sendMessage({
			type: 'requestAnswer',
			rid,
			...answer,
			...(opHash ? { opHash } : {})
		})) as {
			delivered?: boolean;
		} | null;
		const delivered = reply?.delivered === true;
		if (!delivered) console.info(`[dapp] answer not delivered rid=${rid}`);
		return delivered;
	} catch {
		return false;
	}
}

/** Refuse a request in the standard shape. */
/**
 * r11-2: a request this surface TOOK — marked owed, reported shown to the
 * worker — whose sheet could not be built (a read before the sheet threw).
 * Left as it was, the request stayed owed with no sheet: the panel drew the
 * plain wallet, the site's promise never settled, and every later request
 * of the window waited behind it. Nothing was signed or sent (the throw came
 * before any sheet), so the site hears an internal error it may retry —
 * never "the user rejected it", which nobody did.
 */
export const NOT_SHOWN_ERROR = {
	code: -32603,
	message: 'Vela could not show this request. Nothing was signed — try again.'
} as const;

export function answerNotShown(rid: string): Promise<boolean> {
	return answerRequest(rid, { error: { ...NOT_SHOWN_ERROR } }).catch(() => false);
}

export function rejectRequest(
	rid: string,
	message = 'User rejected the request'
): Promise<boolean> {
	return answerRequest(rid, { error: { code: USER_REJECTED, message } });
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
