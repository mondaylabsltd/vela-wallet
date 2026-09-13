/**
 * The service worker: routing, and no authoritative state (spec 027 T320/T322).
 *
 * Ported in PART from packages/safari-extension/src/background.js @ 52ad8fa9.
 * Safari's version also HELD policy — per-origin grants in `storage.local`, a
 * read proxy to a public node, a native round-trip to the iOS app. None of that
 * comes across. Every decision this extension makes belongs to the core's own
 * machines (`dapp_permissions`, `dapp_session`, `ext_cache`), which run in the
 * wallet, not here. This file carries requests to the wallet and answers back,
 * and it makes exactly one promise of its own:
 *
 *   **a request is never left unanswered.**
 *
 * MV3 evicts an idle worker, and a page promise that never settles is the worst
 * thing this extension can produce — a dApp spinner that spins forever while
 * the person cannot tell whether their money moved. So a request is written
 * down the moment it arrives, a window closed without a decision answers 4001,
 * and content.js has its own deadline on top (spec 027 D37).
 *
 * What this file does hold — and why it is not "state" in the sense above:
 *
 *   - **the chain each origin is on** (`vela.chain.<origin>`), written on a
 *     `wallet_switchEthereumChain` the wallet's own catalog sanctions. The
 *     wallet has no global network (ext_cache invariant ⑤): the chain is the
 *     SITE's pick, and a site's pick is a fact about the site, not a judgement
 *     about the wallet;
 *   - **a read**, forwarded verbatim to the node or bundler the catalog names
 *     for that chain, and its answer forwarded back verbatim. Nothing here
 *     reads the payload;
 *   - **the page events** (`accountsChanged`, `chainChanged`, `disconnect`),
 *     emitted when a grant or a chain pick CHANGES in storage. The grant is
 *     what the core wrote — on connect, on an account switch, on revoke — so
 *     announcing its change is mirroring a decision, not making one. This is
 *     the same twin discipline as `resolveGrantedAccounts` (protocol.js).
 */
import {
	CHAIN_PREFIX,
	CHAINS_KEY,
	CLOSED_WITHOUT_ANSWER,
	ERR,
	PERM_PREFIX,
	REQUEST_PREFIX,
	SURFACE_KEY,
	BUNDLER_METHODS,
	chainEndpoints,
	chainKnown,
	classifyMethod,
	isWellFormedRequest,
	originOfUrl,
	parseChainId,
	resolveGrantedAccounts,
	rpcError,
	switchChainParam,
	toHexChainId
} from './lib/protocol.js';
import { negotiate, requestPage, walletPage } from './lib/locales.js';

/** The snapshot the wallet publishes for exactly this purpose. */
const EXT_CACHE_KEY = 'vela.ext.cache';

/** How long a node may take before the next one is tried. */
const READ_TIMEOUT_MS = 20_000;

/**
 * Where requests are answered — cached, because the choice has to be made
 * synchronously (see `openSurface`) and storage cannot be read synchronously.
 * Read once at start, kept fresh by the storage listener below.
 */
let surfacePreference = 'panel';
void chrome.storage.local
	.get(SURFACE_KEY)
	.then((all) => {
		if (all[SURFACE_KEY] === 'window') surfacePreference = 'window';
	})
	.catch(() => {});

// ---------------------------------------------------------------------------
// The doorway
// ---------------------------------------------------------------------------

/**
 * There is deliberately NO `default_popup`. An action popup is dismissed the
 * moment it loses focus, and every ceremony this wallet performs — signing IN
 * included — hands focus to the platform authenticator's own prompt. A wallet
 * living in the action popup would close itself in the middle of every passkey
 * it asks for (spec 027 D34). The toolbar button opens a real tab instead, and
 * it reuses the one already open rather than stacking copies.
 */

/** The locale the wallet's own pages should open in. */
function uiLocale() {
	return negotiate(chrome.i18n?.getUILanguage?.());
}

async function openWallet() {
	const url = chrome.runtime.getURL(walletPage(uiLocale()));
	const [existing] = await chrome.tabs.query({ url: chrome.runtime.getURL('') + '*' });
	if (existing) {
		await chrome.tabs.update(existing.id, { active: true, url });
		await chrome.windows.update(existing.windowId, { focused: true });
		return;
	}
	await chrome.tabs.create({ url });
}

chrome.action.onClicked.addListener(() => {
	openWallet().catch((error) => console.error('[vela] could not open the wallet', error));
});

// ---------------------------------------------------------------------------
// Storage, read defensively
// ---------------------------------------------------------------------------

async function readLocal(keys) {
	try {
		return await chrome.storage.local.get(keys);
	} catch {
		return {};
	}
}

// ---------------------------------------------------------------------------
// Requests in flight
// ---------------------------------------------------------------------------

/**
 * The live half of a pending request: the page's own `sendResponse`, which
 * cannot be persisted and does not survive an eviction. The DURABLE half is in
 * `storage.local` under `REQUEST_PREFIX`, so a worker that comes back can still
 * find out what it owes an answer for.
 *
 * `surface` says where the request is being answered: `'panel'` (the side
 * panel of the asking tab) or `'window'` (a dedicated window). The record
 * itself is kept here too, so the surface can be handed it before the storage
 * write has landed.
 */
const pending = new Map();

async function remember(rid, record) {
	try {
		await chrome.storage.local.set({ [REQUEST_PREFIX + rid]: record });
	} catch {
		/* storage denied — the in-memory half still answers this session */
	}
}

async function forget(rid) {
	try {
		await chrome.storage.local.remove(REQUEST_PREFIX + rid);
	} catch {
		/* nothing to undo */
	}
}

/** Settle a request exactly once, whatever settles it. */
function settle(rid, payload) {
	const entry = pending.get(rid);
	if (!entry) return false;
	pending.delete(rid);
	void forget(rid);
	try {
		entry.reply(payload);
	} catch {
		/* the page is gone; the wallet's own record is the surviving truth */
	}
	if (entry.windowId !== undefined) {
		chrome.windows.remove(entry.windowId).catch(() => {});
	}
	return true;
}

/**
 * The surface that answers — the SIDE PANEL of the asking tab.
 *
 * A panel is what the founder asked for, and it is also the right shape: it
 * belongs to the tab that asked, it cannot be styled, covered or scrolled by
 * the site, and — unlike the action popup — it survives the focus change a
 * passkey prompt causes. `chrome.sidePanel.open` may only be called on a user
 * gesture, and the gesture travels with the page's message: a request a
 * person just clicked for opens the panel, a request a page fired on its own
 * cannot. That one falls back to the dedicated window (D34), which needs no
 * gesture. Either way the request is answered, once.
 *
 * Called SYNCHRONOUSLY from the message listener — the gesture is spent by
 * the first `await`.
 */
function openSurface(rid, tabId) {
	const entry = pending.get(rid);
	const panel = chrome.sidePanel;
	if (surfacePreference !== 'window' && entry && panel && typeof panel.open === 'function') {
		let attempt;
		try {
			attempt = Promise.resolve(panel.open({ tabId }));
		} catch (error) {
			attempt = Promise.reject(error);
		}
		return attempt.then(
			() => {
				entry.surface = 'panel';
			},
			() => openRequestWindow(rid)
		);
	}
	return openRequestWindow(rid);
}

/**
 * The fallback surface: a dedicated window, not the action popup, and not an
 * in-page sheet.
 */
async function openRequestWindow(rid) {
	const entry = pending.get(rid);
	if (!entry) return;
	const created = await chrome.windows.create({
		url: chrome.runtime.getURL(requestPage(uiLocale(), rid)),
		type: 'popup',
		width: 420,
		height: 760,
		focused: true
	});
	entry.surface = 'window';
	entry.windowId = created.id;
}

/** Close the side panel of a tab that has nothing more to show. */
function closePanel(tabId) {
	const panel = chrome.sidePanel;
	if (!panel) return;
	if (typeof panel.close === 'function') {
		Promise.resolve(panel.close({ tabId })).catch(() => {});
		return;
	}
	// Older Chrome: disabling the panel for the tab dismisses it; re-enabling
	// makes the next request able to open it again, without opening it now.
	Promise.resolve(panel.setOptions({ tabId, enabled: false }))
		.then(() => panel.setOptions({ tabId, enabled: true }))
		.catch(() => {});
}

/** The oldest request a panel on `tabId` still owes an answer for. */
function nextForPanel(tabId) {
	let best = null;
	for (const entry of pending.values()) {
		if (entry.surface !== 'panel') continue;
		if (tabId !== undefined && entry.tabId !== tabId) continue;
		if (!best || entry.record.at < best.record.at) best = entry;
	}
	return best ? best.record : null;
}

/**
 * A window that went away still owing an answer.
 *
 * The window settles itself with the CORE's answer on teardown; this is the
 * backstop for one that died before it could. Either way the code is 4900, not
 * 4001 — see `CLOSED_WITHOUT_ANSWER`. An explicit Cancel already answered 4001
 * before closing, so it never reaches here.
 */
chrome.windows.onRemoved.addListener((windowId) => {
	for (const [rid, entry] of pending) {
		if (entry.windowId === windowId) {
			settle(rid, {
				error: rpcError(CLOSED_WITHOUT_ANSWER.code, CLOSED_WITHOUT_ANSWER.message)
			});
		}
	}
});

/**
 * The tab that asked went away, or navigated. Its content script — and the
 * `sendResponse` in hand — died with the document, and a panel bound to the
 * tab has nothing left to answer. Settle with 4900, as a navigation does.
 */
function settleTab(tabId) {
	for (const [rid, entry] of pending) {
		if (entry.tabId === tabId) {
			settle(rid, {
				error: rpcError(CLOSED_WITHOUT_ANSWER.code, CLOSED_WITHOUT_ANSWER.message)
			});
		}
	}
}
chrome.tabs.onRemoved.addListener((tabId) => settleTab(tabId));

/**
 * Drop requests nobody can answer any more.
 *
 * A record outlives the worker on purpose — that is the whole point of writing
 * it down — but it cannot outlive the tab that asked. When this worker starts
 * cold, every record from before it is unanswerable: the page's own
 * `sendResponse` died with the previous worker, and content.js has already
 * settled that page on its deadline. Keeping them would mean a window could
 * later open on a request whose asker is long gone.
 */
async function sweepStaleRequests() {
	try {
		const all = await chrome.storage.local.get(null);
		const stale = Object.keys(all).filter((key) => key.startsWith(REQUEST_PREFIX));
		if (stale.length) await chrome.storage.local.remove(stale);
	} catch {
		/* storage denied — nothing to sweep and nothing to report */
	}
}
void sweepStaleRequests();

// ---------------------------------------------------------------------------
// The instant answers
// ---------------------------------------------------------------------------

/**
 * The chain an origin is on: its own pick, else the chain it connected on,
 * else the snapshot's default. `0` when the wallet has never published.
 */
function chainOf(origin, all) {
	const picked = parseChainId(all[CHAIN_PREFIX + origin]);
	if (picked > 0) return picked;
	const grant = all[PERM_PREFIX + origin];
	const connectedOn = grant && typeof grant === 'object' ? parseChainId(grant.chainId) : 0;
	if (connectedOn > 0) return connectedOn;
	const snapshot = all[EXT_CACHE_KEY];
	return snapshot && typeof snapshot === 'object' ? parseChainId(snapshot.chain_id) : 0;
}

const NOT_OPENED = () => rpcError(ERR.UNAUTHORIZED, 'Vela has not been opened in this browser yet');

/**
 * What an origin may be told without asking anyone.
 *
 * Both values come from state the CORE authored — the grant it wrote, and the
 * snapshot `ext_cache` published — combined by `resolveGrantedAccounts`, a
 * pinned twin of the core's own rule (see its note in `lib/protocol.js`).
 */
async function answerFromSnapshot(method, origin) {
	const all = await readLocal([PERM_PREFIX + origin, CHAIN_PREFIX + origin, EXT_CACHE_KEY]);
	const grant = all[PERM_PREFIX + origin] ?? null;
	const snapshot = all[EXT_CACHE_KEY] ?? null;
	const addresses = Array.isArray(snapshot?.accounts)
		? snapshot.accounts.map((a) => a?.address).filter((a) => typeof a === 'string')
		: null;
	const accounts = resolveGrantedAccounts(grant, addresses);
	const chainId = chainOf(origin, all);

	switch (method) {
		case 'eth_accounts':
			// `[]` for an ungranted origin is the honest answer, and the one
			// EIP-1193 asks for: a disconnected wallet, with no prompt.
			return { result: accounts };
		case 'eth_chainId':
			return chainId > 0 ? { result: toHexChainId(chainId) } : { error: NOT_OPENED() };
		case 'net_version':
			return chainId > 0 ? { result: String(chainId) } : { error: NOT_OPENED() };
		case 'wallet_getPermissions':
			// EIP-2255's shape, from the same grant.
			return { result: accounts.length ? [{ parentCapability: 'eth_accounts' }] : [] };
		default:
			return { error: rpcError(ERR.METHOD_NOT_FOUND, `Unhandled state method ${method}`) };
	}
}

// ---------------------------------------------------------------------------
// Chain switching
// ---------------------------------------------------------------------------

/**
 * `wallet_switchEthereumChain` (EIP-3326) and `wallet_addEthereumChain`
 * (EIP-3085) for a chain the wallet already has.
 *
 * One address on every chain is this wallet's whole proposition, so a switch
 * asks nobody: the pick is written for the ORIGIN, the page hears
 * `chainChanged` (from the storage listener below), and the request answers
 * `null` as the EIP says. A chain the catalog does not know is 4902 — the
 * code a dApp reads as "offer to add it" — and adding one is the wallet's
 * Settings screen, never a site's request.
 */
async function switchChain(method, params, origin) {
	const chainId = switchChainParam(params);
	if (chainId <= 0) {
		return { error: rpcError(ERR.INVALID_PARAMS, 'Expected [{ chainId }]') };
	}
	const all = await readLocal([
		CHAINS_KEY,
		CHAIN_PREFIX + origin,
		PERM_PREFIX + origin,
		EXT_CACHE_KEY
	]);
	const catalog = all[CHAINS_KEY];
	if (!catalog) return { error: NOT_OPENED() };
	if (!chainKnown(catalog, chainId)) {
		return {
			error: rpcError(
				ERR.CHAIN_NOT_ADDED,
				method === 'wallet_addEthereumChain'
					? `Add chain ${chainId} in Vela's network settings first`
					: `Chain ${chainId} is not in Vela's networks`
			)
		};
	}
	if (chainOf(origin, all) !== chainId) {
		try {
			await chrome.storage.local.set({ [CHAIN_PREFIX + origin]: chainId });
		} catch {
			return { error: rpcError(ERR.INTERNAL, 'Could not record the chain switch') };
		}
	}
	return { result: null };
}

// ---------------------------------------------------------------------------
// Reads — forwarded, never interpreted
// ---------------------------------------------------------------------------

/**
 * A node or bundler read for the origin's chain, on the endpoints the wallet
 * published. A transport failure moves to the next endpoint; a JSON-RPC error
 * IS an answer (a revert is what the page asked to learn) and goes back as is.
 */
async function forwardRead(method, params, origin) {
	const all = await readLocal([
		CHAINS_KEY,
		CHAIN_PREFIX + origin,
		PERM_PREFIX + origin,
		EXT_CACHE_KEY
	]);
	const catalog = all[CHAINS_KEY];
	if (!catalog) return { error: NOT_OPENED() };
	const chainId = chainOf(origin, all);
	const endpoints = chainEndpoints(catalog, chainId, BUNDLER_METHODS.has(method));
	if (endpoints.length === 0) {
		return { error: rpcError(ERR.CHAIN_NOT_ADDED, `Vela has no endpoint for chain ${chainId}`) };
	}

	let failure = 'no endpoint answered';
	for (const url of endpoints) {
		try {
			const response = await fetch(url, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
				signal: AbortSignal.timeout(READ_TIMEOUT_MS)
			});
			if (!response.ok) {
				failure = `HTTP ${response.status}`;
				continue;
			}
			const body = await response.json();
			if (body && body.error && typeof body.error === 'object') {
				const { code, message, data } = body.error;
				return {
					error: rpcError(
						typeof code === 'number' ? code : ERR.INTERNAL,
						typeof message === 'string' ? message : 'RPC error',
						data
					)
				};
			}
			return { result: body && 'result' in body ? body.result : null };
		} catch (error) {
			failure = String(error?.message ?? error);
		}
	}
	return {
		error: rpcError(ERR.INTERNAL, `Vela could not reach a node for chain ${chainId}: ${failure}`)
	};
}

// ---------------------------------------------------------------------------
// The events a connected page hears
// ---------------------------------------------------------------------------

/** Every tab whose document is `origin`, told `event`. */
async function broadcast(origin, event, data) {
	let tabs;
	try {
		tabs = await chrome.tabs.query({});
	} catch {
		return;
	}
	for (const tab of tabs) {
		if (tab.id === undefined || originOfUrl(tab.url) !== origin) continue;
		chrome.tabs.sendMessage(tab.id, { type: 'evt', event, data }).catch(() => {
			/* no content script in that tab (not yet loaded, or a page we cannot run in) */
		});
	}
}

/**
 * A grant or a chain pick changed in storage — announce it.
 *
 * The grant is the core's: written on connect, re-pinned by `account_switched`
 * when the wallet switches accounts, removed by a revoke. What each change
 * means to the page is the core's `DpermPageEvent` vocabulary, mirrored here:
 * a new or re-pinned address → `accountsChanged([address])`; a removed grant →
 * `accountsChanged([])` then `disconnect`; a chain pick → `chainChanged`.
 */
chrome.storage.onChanged.addListener((changes, area) => {
	if (area !== 'local') return;
	for (const [key, change] of Object.entries(changes)) {
		if (key === SURFACE_KEY) {
			surfacePreference = change.newValue === 'window' ? 'window' : 'panel';
		} else if (key.startsWith(PERM_PREFIX)) {
			const origin = key.slice(PERM_PREFIX.length);
			const before = change.oldValue?.address;
			const after = change.newValue?.address;
			if (typeof after === 'string') {
				if (typeof before !== 'string' || before.toLowerCase() !== after.toLowerCase()) {
					void broadcast(origin, 'accountsChanged', [after]);
				}
			} else if (typeof before === 'string') {
				void broadcast(origin, 'accountsChanged', []);
				void broadcast(origin, 'disconnect', null);
			}
		} else if (key.startsWith(CHAIN_PREFIX)) {
			const origin = key.slice(CHAIN_PREFIX.length);
			const chainId = parseChainId(change.newValue);
			if (chainId > 0) void broadcast(origin, 'chainChanged', toHexChainId(chainId));
		}
	}
});

// ---------------------------------------------------------------------------
// The router
// ---------------------------------------------------------------------------

function route(request, sender, reply) {
	// The two facts the page cannot forge, taken from the browser rather than
	// from the message: who asked, and from where.
	const origin = sender.origin ?? (sender.url ? new URL(sender.url).origin : null);
	const tabId = sender.tab?.id;
	if (!origin || tabId === undefined) {
		reply({ error: rpcError(ERR.INTERNAL, 'Request did not come from a page') });
		return;
	}
	if (!isWellFormedRequest(request)) {
		// Malformed, or larger than a request has any business being. Refused
		// before it reaches a screen that would have to render it.
		reply({ error: rpcError(ERR.INVALID_PARAMS, 'Malformed request') });
		return;
	}

	const rid = `${tabId}:${request.id}`;
	if (pending.has(rid)) {
		// One answer, once. A repeated id is a page confusing itself.
		reply({ error: rpcError(ERR.INVALID_PARAMS, 'Duplicate request id') });
		return;
	}

	const bucket = classifyMethod(request.method);
	switch (bucket) {
		case 'unsupported':
			reply({
				error: rpcError(ERR.UNSUPPORTED_METHOD, `Vela does not support ${request.method}`)
			});
			return;
		case 'state':
			// Answered from what the WALLET published, never from a judgement made
			// here. A page that is already connected asks these on every load;
			// opening a surface for them would be absurd.
			void answerFromSnapshot(request.method, origin).then(reply);
			return;
		case 'switch':
		case 'addChain':
			void switchChain(request.method, request.params, origin).then(reply);
			return;
		case 'watchAsset':
			// EIP-747: `false` is "not added". Tokens are added in the wallet, where
			// the person can see what they are adding.
			reply({ result: false });
			return;
		case 'read':
			void forwardRead(request.method, request.params, origin).then(reply);
			return;
		case 'sign':
		case 'connect':
			break;
		default:
			reply({ error: rpcError(ERR.UNSUPPORTED_METHOD, `Vela does not support ${request.method}`) });
			return;
	}

	const record = {
		rid,
		id: request.id,
		method: request.method,
		params: request.params,
		origin,
		tabId,
		at: Date.now()
	};
	pending.set(rid, { reply, tabId, origin, record, surface: undefined, windowId: undefined });
	// The surface FIRST, synchronously: the user gesture that lets the side
	// panel open does not survive an `await`.
	const opened = openSurface(rid, tabId);
	void remember(rid, record);
	opened.catch((error) => {
		settle(rid, { error: rpcError(ERR.INTERNAL, String(error?.message ?? error)) });
	});
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
	if (!message || typeof message !== 'object') return;

	if (message.type === 'rpc') {
		route(message, sender, sendResponse);
		return true; // the answer comes later
	}

	// ---- the wallet's own half ---------------------------------------------

	if (message.type === 'requestDetail') {
		// A window opened for one request, by id. The in-memory record first —
		// the storage write may still be in flight when the page asks.
		const live = pending.get(message.rid);
		if (live) {
			sendResponse(live.record);
			return false;
		}
		chrome.storage.local
			.get(REQUEST_PREFIX + message.rid)
			.then((all) => sendResponse(all[REQUEST_PREFIX + message.rid] ?? null))
			.catch(() => sendResponse(null));
		return true;
	}

	if (message.type === 'requestCurrent') {
		// The side panel of a tab, asking what it owes. Nothing → `null`, and
		// the panel closes itself.
		sendResponse(nextForPanel(typeof message.tabId === 'number' ? message.tabId : undefined));
		return false;
	}

	if (message.type === 'panelDone') {
		if (typeof message.tabId === 'number' && !nextForPanel(message.tabId)) {
			closePanel(message.tabId);
		}
		sendResponse({ ok: true });
		return false;
	}

	if (message.type === 'requestAnswer') {
		const delivered = settle(message.rid, { result: message.result, error: message.error });
		sendResponse({ delivered });
		return false;
	}

	return undefined;
});
