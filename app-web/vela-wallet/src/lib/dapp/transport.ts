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
	return runtime() !== null && location.protocol === 'chrome-extension:';
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
	answer: { result?: unknown; error?: { code: number; message: string } },
	opHash?: { chainId: number }
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
