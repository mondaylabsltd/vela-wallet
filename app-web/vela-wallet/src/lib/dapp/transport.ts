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
 *   - silence is refusal (a window closed without a decision answers 4001);
 *   - the wallet's own record outlives the page's answer (026's persist-at-
 *     submit ordering is untouched, so an operation that was submitted is in
 *     the activity feed whether or not the dApp ever heard back).
 */

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
		// to show, and closing it answers 4001 — which is the honest outcome.
		return null;
	}
}

/**
 * The tab this side panel belongs to, if this page is one.
 *
 * A side panel is per tab: the worker opened it for the tab that asked, and
 * `tabs.query` for the active tab of the panel's own window is that tab. Off
 * a panel (a window, the hosted site) there is no answer, and the worker then
 * hands the panel whatever is owed on a panel anywhere — one tab at a time is
 * the normal case, so both roads lead to the same request.
 */
export async function panelTabId(): Promise<number | undefined> {
	const tabs = (globalThis as { chrome?: { tabs?: unknown } }).chrome?.tabs as
		| { query(info: { active: boolean; currentWindow: boolean }): Promise<{ id?: number }[]> }
		| undefined;
	if (!tabs || typeof tabs.query !== 'function') return undefined;
	try {
		const [tab] = await tabs.query({ active: true, currentWindow: true });
		return typeof tab?.id === 'number' ? tab.id : undefined;
	} catch {
		return undefined;
	}
}

/** What the side panel of `tabId` owes an answer for right now, oldest first. */
export async function currentPanelRequest(
	tabId: number | undefined
): Promise<ExtensionRequest | null> {
	const api = runtime();
	if (!api) return null;
	try {
		const detail = await api.sendMessage({ type: 'requestCurrent', tabId });
		return isRequest(detail) ? detail : null;
	} catch {
		return null;
	}
}

/** The panel has nothing more to show: the worker may dismiss it. */
export async function panelDone(tabId: number | undefined): Promise<void> {
	const api = runtime();
	if (!api) return;
	try {
		await api.sendMessage({ type: 'panelDone', tabId });
	} catch {
		/* the page closes itself regardless */
	}
}

/** Hand the answer back. Settling twice is the background's job to refuse. */
export async function answerRequest(
	rid: string,
	answer: { result?: unknown; error?: { code: number; message: string } }
): Promise<boolean> {
	const api = runtime();
	if (!api) return false;
	try {
		const reply = (await api.sendMessage({ type: 'requestAnswer', rid, ...answer })) as {
			delivered?: boolean;
		} | null;
		return reply?.delivered === true;
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
