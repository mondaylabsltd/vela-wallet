/**
 * The service worker: routing, and the one owner of a request's life.
 *
 * Ported in PART from packages/safari-extension/src/background.js @ 52ad8fa9.
 * Every decision about a connection or a signature belongs to the core's own
 * machines (`dapp_permissions`, `sign_request`, `ext_cache`), which run in the
 * wallet, not here. This file carries requests to the wallet and answers back,
 * and it makes one promise of its own:
 *
 *   **a request is answered exactly once, and only while its page is there.**
 *
 * Spec 082 (RB1–RB11) moved the whole life of a request here, because only the
 * worker sees both the page that asked and the surface that answers:
 *
 *   - a request is a record in `storage.session`
 *     (`vela.req.<tabId>:<pageRequestId>`), written the moment it arrives, so
 *     an evicted worker comes back knowing what it owes (G19 — the old start
 *     sweep deleted every record, and a live request with it);
 *   - the page is known by Chrome's `documentId`; content.js holds a
 *     `vela.doc` port while it owes an answer, and that port closing means the
 *     page left (a reload settles the old request at once — G17);
 *   - the surface holds a `vela.surface` port; Chrome's panel ✕ closes it, and
 *     the requests it owed are settled at once (EX6);
 *   - one queue per window, whichever tab asked (G18);
 *   - the surface CLAIMS a request before a grant, before the passkey and
 *     before the relay POST, so nothing is ever signed or sent for a page that
 *     is gone (RB5);
 *   - a request that ends without a decision answers 4900, never 4001 and
 *     never Chrome's own error text (RB6) — except a claimed submit whose
 *     claim carried the operation hash: that one may have been sent, and its
 *     page is told the hash when its surface goes (RJ2), never 4900.
 *
 * The rules are pure functions in `lib/request-life.js`; this file performs
 * them and logs every step through `lib/swlog.js` (RB14).
 *
 * What else this file holds — and why it is not "state" in the sense above:
 *
 *   - **the chain each origin is on** (`vela.chain.<origin>`), the SITE's pick;
 *   - **a read**, forwarded to the node or bundler the catalog names, with the
 *     core's 8 s budget per endpoint and a memory of endpoints that just
 *     failed (RF2); receipt reads for an operation hash this wallet handed out
 *     are translated to the real transaction (RF3);
 *   - **the page events** (`accountsChanged`, `chainChanged`, `disconnect`),
 *     emitted when a grant or a chain pick CHANGES in storage.
 */
import {
	CHAIN_PREFIX,
	CHAINS_KEY,
	DOC_PORT,
	ENDPOINTS_KEY,
	ERR,
	PANEL_HELLO_WAIT_MS,
	PANEL_OPEN_WAIT_MS,
	PERM_PREFIX,
	READ_SLOW_MS,
	READ_TIMEOUT_MS,
	REQUEST_PREFIX,
	SURFACE_KEY,
	SURFACE_PORT,
	BUNDLER_METHODS,
	chainEndpoints,
	chainKnown,
	chainNameOf,
	classifyMethod,
	endpointAnswered,
	endpointFailed,
	isWellFormedRequest,
	orderEndpoints,
	originOfUrl,
	parseChainId,
	readFailureKind,
	resolveGrantedAccounts,
	rpcError,
	switchChainParam,
	toHexChainId,
	unreachableChainMessage
} from './lib/protocol.js';
import {
	affectedBy,
	callerOwns,
	claimVerdict,
	newRecord,
	nextForWindow,
	recoveryPlan,
	settlement,
	surfaceAfterOpen,
	surfaceAnswer,
	withClaim
} from './lib/request-life.js';
import { createSwLog } from './lib/swlog.js';
import {
	expiredOpKeys,
	liveOpEntry,
	opKey,
	opRecord,
	realTxHash,
	receiptLookupHash
} from './lib/op-receipt.js';
import { openDoor } from './lib/locales.js';

/** The snapshot the wallet publishes for exactly this purpose. */
const EXT_CACHE_KEY = 'vela.ext.cache';

/** `storage.session`: the ledger, the endpoint memory and the log. */
const sessionArea = chrome.storage.session;
const swlog = createSwLog({ storage: sessionArea });

/**
 * Where requests are answered — cached, because the choice has to be made
 * synchronously (the side panel may only be opened inside the page's user
 * gesture, which the first `await` spends). Read once at start, kept fresh by
 * the storage listener below.
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
 * moment it loses focus, and every ceremony this wallet performs hands focus
 * to the platform authenticator's own prompt (spec 027 D34). The toolbar
 * button opens a real tab instead, reusing the one already open.
 *
 * Through the doorway, never at a locale page: which locale is the page's to
 * pick, from the language the person pinned — which a worker cannot read
 * (`localStorage`), so picking here meant Chrome's language (issue 317).
 */
async function openWallet() {
	const url = chrome.runtime.getURL(openDoor());
	const [existing] = await chrome.tabs.query({ url: chrome.runtime.getURL('') + '*' });
	if (existing) {
		await chrome.tabs.update(existing.id, { active: true, url });
		await chrome.windows.update(existing.windowId, { focused: true });
		return;
	}
	await chrome.tabs.create({ url });
}

chrome.action?.onClicked?.addListener(() => {
	openWallet().catch(() => console.error('[vela] could not open the wallet'));
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
// The ledger (RB1)
// ---------------------------------------------------------------------------

/**
 * The in-memory mirror of the records in `storage.session`. The stored copy
 * is the truth across an eviction; this one is what the synchronous paths
 * read. Filled by `recover()` before anything reads it (`loaded`).
 */
const records = new Map();
/** documentId → the page's `vela.doc` port. */
const docPorts = new Map();
/** surface key (`panel:<windowId>` / `window:<rid>`) → `{port, caller}`. */
const surfaces = new Map();

const keyOf = (rid) => REQUEST_PREFIX + rid;
const surfaceKey = (caller) =>
	caller.kind === 'panel' ? `panel:${caller.windowId}` : `window:${caller.rid}`;

async function persist(record) {
	try {
		await sessionArea.set({ [keyOf(record.rid)]: record });
	} catch {
		/* storage denied — the in-memory record still answers this worker's life */
	}
}

async function unpersist(rid) {
	try {
		await sessionArea.remove(keyOf(rid));
	} catch {
		/* nothing to undo */
	}
}

/** What a surface is handed: the request, without the worker's bookkeeping. */
function publicRecord(record) {
	return {
		rid: record.rid,
		id: record.id,
		method: record.method,
		params: record.params,
		origin: record.origin,
		tabId: record.tabId,
		at: record.at
	};
}

/**
 * Tell the PAGE, by its document — never a later document in the same tab.
 * `true` when content.js took it (`{ok:true}`); a rejection means the page is
 * gone.
 */
async function toPage(record, message) {
	try {
		const options = record.documentId ? { documentId: record.documentId } : undefined;
		const reply = await chrome.tabs.sendMessage(record.tabId, message, options);
		return reply;
	} catch {
		return null;
	}
}

async function deliver(record, payload) {
	const message = { type: 'answer', id: record.id };
	if (payload.error) message.error = payload.error;
	else message.result = payload.result === undefined ? null : payload.result;
	const reply = await toPage(record, message);
	return reply?.ok === true;
}

/** Does the page still own `record.id`? (`alive`, by documentId.) */
async function probeAlive(record) {
	const reply = await toPage(record, { type: 'alive', ids: [record.id] });
	return Array.isArray(reply?.alive) && reply.alive.includes(record.id);
}

function postTo(caller, message) {
	const entry = surfaces.get(surfaceKey(caller));
	if (!entry) return false;
	try {
		entry.port.postMessage(message);
		return true;
	} catch {
		return false;
	}
}

/** The surface that owns `record`, as a caller. */
function ownerOf(record) {
	return record.surface === 'panel'
		? { kind: 'panel', windowId: record.surfaceWindowId }
		: { kind: 'window', rid: record.rid };
}

/** Hand the next owed request to a surface, if that surface is listening. */
function pushOwed(caller) {
	const all = [...records.values()];
	const next =
		caller.kind === 'panel'
			? nextForWindow(all, caller.windowId)
			: (records.get(caller.rid) ?? null);
	if (next && callerOwns(next, caller)) {
		postTo(caller, { type: 'owed', request: publicRecord(next) });
	}
}

/**
 * End a request without a decision, exactly once: the page is told 4900 with
 * the cause's plain words (RB6), the surface is told `withdrawn` (it closes
 * the card or sheet without words, RB15), and the next owed request moves up.
 */
async function settle(rid, cause) {
	const record = records.get(rid);
	if (!record) return false;
	records.delete(rid);
	await unpersist(rid);
	const owner = ownerOf(record);
	postTo(owner, { type: 'withdrawn', rid, cause });
	void swlog.log('req.settled', { cause, tab: record.tabId });
	void deliver(record, { error: settlement(cause) });
	if (record.surface === 'window' && record.surfaceWindowId !== undefined) {
		chrome.windows.remove(record.surfaceWindowId).catch(() => {});
	}
	if (owner.kind === 'panel') pushOwed(owner);
	return true;
}

/**
 * End a request whose surface went after it was claimed for submit with its
 * operation hash (RJ2): it may have been sent, so the page is told so, once —
 * "not confirmed yet" (083, owner ruling 2026-10-01; a batch: its id) — and
 * its receipt reads for the hash are translated from now on (RF3). The
 * wallet's own record of it, written before the POST, is what the tracker
 * resumes on the next boot.
 */
async function answerMaybeSent(rid, cause, payload, opHash) {
	const record = records.get(rid);
	if (!record) return false;
	records.delete(rid);
	await unpersist(rid);
	const owner = ownerOf(record);
	postTo(owner, { type: 'withdrawn', rid, cause });
	const delivered = await deliver(record, payload);
	void swlog.log('req.answered', {
		cause,
		maybe_sent: 1,
		delivered,
		outcome: payload.error ? 'not_confirmed' : 'ok',
		tab: record.tabId
	});
	void rememberOp(opHash, { chainId: record.chainId });
	if (record.surface === 'window' && record.surfaceWindowId !== undefined) {
		chrome.windows.remove(record.surfaceWindowId).catch(() => {});
	}
	if (owner.kind === 'panel') pushOwed(owner);
	return delivered;
}

/**
 * One request ended by a browser event or a recovery step: `{rid, cause}` is
 * settled 4900; `{rid, cause, answer, opHash}` — a claimed submit that may
 * have been sent — is answered `answer` (RJ2, `maybeSentPayload`).
 */
function end(step) {
	return step.answer
		? answerMaybeSent(step.rid, step.cause, step.answer, step.opHash)
		: settle(step.rid, step.cause);
}

/**
 * The surface's answer, delivered to the page by its document. A surface's own
 * close settlement (4900) for a claimed submit that carried its hash is surface
 * loss: the page is told the hash instead (RJ2, `surfaceAnswer`).
 */
async function answer(rid, given, opHash, caller) {
	const record = records.get(rid);
	if (!record || (caller && !callerOwns(record, caller))) return false;
	const { payload, maybeSent } = surfaceAnswer(record, given);
	records.delete(rid);
	await unpersist(rid);
	const delivered = await deliver(record, payload);
	void swlog.log('req.answered', {
		...(maybeSent ? { cause: 'surface_settled', maybe_sent: 1 } : {}),
		delivered,
		outcome: payload.error ? 'error' : 'ok',
		tab: record.tabId
	});
	if (maybeSent) void rememberOp(maybeSent, { chainId: record.chainId });
	else if (!payload.error) void rememberOp(payload.result, opHash);
	if (record.surface === 'window' && record.surfaceWindowId !== undefined) {
		chrome.windows.remove(record.surfaceWindowId).catch(() => {});
	}
	const owner = ownerOf(record);
	if (owner.kind === 'panel') pushOwed(owner);
	return delivered;
}

// ---------------------------------------------------------------------------
// Where a request is shown (RB8)
// ---------------------------------------------------------------------------

/**
 * Ask Chrome to open the side panel — SYNCHRONOUSLY, inside the page's user
 * gesture. Returns a promise of `'ok' | 'failed'`; the caller races it with
 * `PANEL_OPEN_WAIT_MS`.
 */
function openPanelNow(tabId) {
	try {
		return Promise.resolve(chrome.sidePanel.open({ tabId })).then(
			() => 'ok',
			() => 'failed'
		);
	} catch {
		return Promise.resolve('failed');
	}
}

function within(promise, ms, fallback) {
	return Promise.race([promise, new Promise((resolve) => setTimeout(() => resolve(fallback), ms))]);
}

/**
 * The windows that have a side panel up, or `null` when that cannot be told.
 *
 * Chrome reports the manifest's global side panel with `windowId: -1`
 * (measured on Chrome for Testing 151), so a panel context that names no
 * window cannot rule any window out: that is `null` — the record is probed,
 * never settled on a guess. No side panel at all is an empty set. A panel
 * whose port is connected is up, whatever Chrome reports.
 */
async function panelWindows() {
	let contexts;
	try {
		contexts = await chrome.runtime.getContexts({ contextTypes: ['SIDE_PANEL'] });
	} catch {
		return null;
	}
	const ids = contexts.map((c) => c.windowId);
	if (ids.some((id) => typeof id !== 'number' || id < 0)) return null;
	const up = new Set(ids);
	for (const { caller } of surfaces.values()) {
		if (caller.kind === 'panel') up.add(caller.windowId);
	}
	return up;
}

async function openWindows() {
	try {
		const all = await chrome.windows.getAll();
		return new Set(all.map((w) => w.id));
	} catch {
		return null;
	}
}

/**
 * The fallback surface: a dedicated window, not the action popup — opened at
 * the doorway, which picks the person's language (issue 317).
 */
async function openRequestWindow(record) {
	try {
		const created = await chrome.windows.create({
			url: chrome.runtime.getURL(openDoor(record.rid)),
			type: 'popup',
			width: 420,
			height: 760,
			focused: true
		});
		const live = records.get(record.rid);
		if (!live) {
			// Answered or settled while the window was opening.
			chrome.windows.remove(created.id).catch(() => {});
			return;
		}
		live.surface = 'window';
		live.surfaceWindowId = created.id;
		await persist(live);
		void swlog.log('req.surface', { surface: 'window', tab: live.tabId });
	} catch {
		await settle(record.rid, 'surface_closed');
	}
}

/** surface key → the callbacks waiting for that surface's `hello`. */
const helloWaiters = new Map();

/**
 * Does the surface `key` say hello within `ms`? `true` at once when its port
 * is already up.
 */
function panelHello(key, ms) {
	if (surfaces.has(key)) return Promise.resolve(true);
	return new Promise((resolve) => {
		const waiting = helloWaiters.get(key) ?? new Set();
		helloWaiters.set(key, waiting);
		const done = (up) => {
			clearTimeout(timer);
			waiting.delete(done);
			if (waiting.size === 0 && helloWaiters.get(key) === waiting) helloWaiters.delete(key);
			resolve(up);
		};
		const timer = setTimeout(() => done(surfaces.has(key)), ms);
		waiting.add(done);
	});
}

/** A request that arrived: into the ledger, onto its surface. */
async function admit(record, panelAttempt) {
	await loaded;
	const existing = records.get(record.rid);
	if (existing) {
		// The same document asking again (content.js retries a dropped send):
		// already written down, and answered once, later.
		if (existing.documentId === record.documentId) return;
		// Another document in the same tab reusing an id: the old page is gone.
		await settle(existing.rid, 'page_left');
	}
	records.set(record.rid, record);
	await persist(record);
	void swlog.log('req.arrived', {
		kind: classifyMethod(record.method),
		tab: record.tabId,
		host: record.origin
	});

	if (record.surface === 'window') {
		await openRequestWindow(record);
		return;
	}
	void swlog.log('req.surface', { surface: 'panel', tab: record.tabId });
	const owner = ownerOf(record);
	pushOwed(owner);

	const opened = await within(panelAttempt, PANEL_OPEN_WAIT_MS, 'timeout');
	if (opened === 'ok' || !records.has(record.rid)) return;
	const key = surfaceKey(owner);
	let panelOpen = surfaces.has(key);
	if (!panelOpen) {
		const up = await panelWindows();
		// `null`: Chrome cannot say which window its side panel is in (the
		// global panel reports -1). An idle panel holds no port (RJ20, G63) but
		// wakes on the record just written for its window — so it is given a
		// moment to say hello before a window opens beside it (RB8, EX2).
		panelOpen = up ? up.has(record.surfaceWindowId) : await panelHello(key, PANEL_HELLO_WAIT_MS);
	}
	const where = surfaceAfterOpen({ opened, panelOpen, preference: surfacePreference });
	if (where === 'window' && records.has(record.rid)) await openRequestWindow(record);
}

// ---------------------------------------------------------------------------
// Recovery at start (RB4: resume, don't settle)
// ---------------------------------------------------------------------------

async function recover() {
	// Records from before 082 lived in storage.local; they are nobody's now.
	try {
		const local = await chrome.storage.local.get(null);
		const leftovers = Object.keys(local).filter((key) => key.startsWith(REQUEST_PREFIX));
		const expired = expiredOpKeys(local, Date.now());
		if (leftovers.length || expired.length) {
			await chrome.storage.local.remove([...leftovers, ...expired]);
		}
	} catch {
		/* storage denied — nothing to clear */
	}
	let stored;
	try {
		stored = await sessionArea.get(null);
	} catch {
		stored = {};
	}
	for (const [key, value] of Object.entries(stored)) {
		if (!key.startsWith(REQUEST_PREFIX) || !value || typeof value !== 'object') continue;
		if (typeof value.rid === 'string' && !records.has(value.rid)) records.set(value.rid, value);
	}
}

async function resume() {
	const found = records.size;
	const plan = recoveryPlan([...records.values()], {
		now: Date.now(),
		panelWindows: await panelWindows(),
		openWindows: await openWindows()
	});
	let kept = 0;
	await Promise.all(
		plan.map(async (step) => {
			const record = records.get(step.rid);
			if (!record) return;
			if (step.action === 'settle') {
				await end(step);
				return;
			}
			if (await probeAlive(record)) {
				kept += 1;
				void swlog.log('req.resumed', { tab: record.tabId, state: record.state });
				pushOwed(ownerOf(record));
			} else {
				await settle(step.rid, 'page_left');
			}
		})
	);
	void swlog.log('sw.start', { records: found, recovered: kept });
}

/** Resolves once the ledger is in memory; every request path waits for it. */
const loaded = recover();
/** Resolves once every record found at start was resumed or settled. */
export const ready = loaded.then(resume).catch(() => {});

// ---------------------------------------------------------------------------
// The page's port and the surface's port
// ---------------------------------------------------------------------------

chrome.runtime.onConnect.addListener((port) => {
	if (port.name === DOC_PORT) onDocPort(port);
	else if (port.name === SURFACE_PORT) onSurfacePort(port);
});

/**
 * content.js holds this port only while its page owes a sign/connect answer.
 * Closing it with an `idle` first is content.js saying "nothing owed"; closing
 * it without one, while this worker lives, is the page leaving (RB3).
 */
function onDocPort(port) {
	const documentId = port.sender?.documentId;
	if (!documentId) return;
	docPorts.set(documentId, port);
	let idle = false;
	port.onMessage.addListener((message) => {
		if (message?.type === 'idle') idle = true;
	});
	port.onDisconnect.addListener(() => {
		if (docPorts.get(documentId) === port) docPorts.delete(documentId);
		if (idle) return;
		void loaded.then(() => {
			// A new port for the same document (content.js reconnected) means
			// the page is still there.
			if (docPorts.has(documentId)) return;
			for (const { rid, cause } of affectedBy([...records.values()], {
				type: 'doc_closed',
				documentId
			})) {
				void settle(rid, cause);
			}
		});
	});
}

/**
 * The panel (or a request window) — `hello`, then `shown` / `claim` /
 * `answer` / `ping`; this side pushes `owed` and `withdrawn` (contract §14).
 */
function onSurfacePort(port) {
	const pageUrl = port.sender?.url ?? '';
	if (!pageUrl.startsWith(chrome.runtime.getURL(''))) {
		port.disconnect();
		return;
	}
	let caller = null;
	port.onMessage.addListener((message) => {
		if (!message || typeof message !== 'object') return;
		void loaded.then(() => onSurfaceMessage(port, message, caller, (c) => (caller = c)));
	});
	port.onDisconnect.addListener(() => {
		if (!caller) return;
		const key = surfaceKey(caller);
		if (surfaces.get(key)?.port !== port) return;
		surfaces.delete(key);
		if (caller.kind === 'panel') void swlog.log('panel.down', { window: caller.windowId });
		void loaded.then(() => {
			if (surfaces.has(key)) return;
			for (const step of affectedBy([...records.values()], { type: 'surface_closed', caller })) {
				void end(step);
			}
		});
	});
}

async function onSurfaceMessage(port, message, caller, setCaller) {
	switch (message.type) {
		case 'hello': {
			const next =
				message.kind === 'panel' && typeof message.windowId === 'number'
					? { kind: 'panel', windowId: message.windowId }
					: message.kind === 'window' && typeof message.rid === 'string'
						? { kind: 'window', rid: message.rid }
						: null;
			if (!next) return;
			setCaller(next);
			surfaces.set(surfaceKey(next), { port, caller: next });
			if (next.kind === 'panel') void swlog.log('panel.up', { window: next.windowId });
			pushOwed(next);
			for (const done of [...(helloWaiters.get(surfaceKey(next)) ?? [])]) done(true);
			return;
		}
		case 'shown': {
			const record = records.get(message.rid);
			if (!record || !caller || !callerOwns(record, caller)) return;
			if (record.state === 'created') {
				record.state = 'shown';
				await persist(record);
			}
			void swlog.log('req.shown', { tab: record.tabId });
			return;
		}
		case 'claim': {
			const live = await claim(message.rid, message.phase, caller, {
				opHash: message.opHash,
				chainId: message.chainId
			});
			try {
				port.postMessage({ type: 'claimResult', nonce: message.nonce, ...live });
			} catch {
				/* the surface went away; its disconnect settles what it owed */
			}
			return;
		}
		case 'answer': {
			const delivered = await answer(
				message.rid,
				{ result: message.result, error: message.error },
				message.opHash,
				caller
			);
			try {
				port.postMessage({ type: 'answered', rid: message.rid, delivered });
			} catch {
				/* the surface went away after answering — nothing is owed */
			}
			return;
		}
		case 'ping':
		default:
			return;
	}
}

/**
 * RB5: is the request live enough to act on? A live claim moves it to
 * `claimed` and tells the page, which extends its own deadline; a request
 * past its limit, or whose page is gone, is settled right here. A live
 * `submit` claim keeps the operation hash and chain it carries (RJ2).
 */
async function claim(rid, phase, caller, detail = {}) {
	const record = records.get(rid);
	let docAttached = record ? docPorts.has(record.documentId) : false;
	if (record && !docAttached) docAttached = await probeAlive(record);
	const verdict = claimVerdict(record, { now: Date.now(), docAttached, caller, phase });
	void swlog.log('req.claim', {
		phase,
		live: verdict.live,
		cause: verdict.cause,
		tab: record?.tabId
	});
	if (verdict.live) {
		const claimed = withClaim(record, { phase, now: Date.now(), ...detail });
		records.set(rid, claimed);
		await persist(claimed);
		void toPage(claimed, { type: 'claimed', id: claimed.id });
		return { live: true };
	}
	if (record && (verdict.cause === 'expired' || verdict.cause === 'page_left')) {
		await settle(rid, verdict.cause);
	}
	return { live: false, cause: verdict.cause };
}

// ---------------------------------------------------------------------------
// The browser's own backstops
// ---------------------------------------------------------------------------

function settleAll(event) {
	void loaded.then(() => {
		for (const step of affectedBy([...records.values()], event)) void end(step);
	});
}

chrome.tabs.onRemoved.addListener((tabId) => settleAll({ type: 'tab_removed', tabId }));
chrome.tabs.onReplaced?.addListener((_added, removedTabId) =>
	settleAll({ type: 'tab_replaced', tabId: removedTabId })
);
chrome.windows.onRemoved.addListener((windowId) => settleAll({ type: 'window_removed', windowId }));

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
 * What an origin may be told without asking anyone — from state the CORE
 * authored, combined by `resolveGrantedAccounts` (a pinned twin of the core's
 * rule, see `lib/protocol.js`).
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
			return { result: accounts };
		case 'eth_coinbase':
			return { result: accounts[0] ?? null };
		case 'eth_chainId':
			return chainId > 0 ? { result: toHexChainId(chainId) } : { error: NOT_OPENED() };
		case 'net_version':
			return chainId > 0 ? { result: String(chainId) } : { error: NOT_OPENED() };
		case 'wallet_getPermissions':
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
 * (EIP-3085) for a chain the wallet already has: the pick is written for the
 * ORIGIN and the page hears `chainChanged` from the storage listener. A chain
 * the catalog does not know is 4902.
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
// Reads — forwarded, never interpreted (RF2, RF3)
// ---------------------------------------------------------------------------

/**
 * The endpoints that just failed, `{<url>: {failures, until}}` — kept in
 * memory and written through to `storage.session`, so a woken worker does not
 * spend another 8 s on a node it already knows is down.
 */
let endpointHealth = null;

async function health() {
	if (endpointHealth) return endpointHealth;
	try {
		const all = await sessionArea.get(ENDPOINTS_KEY);
		const stored = all?.[ENDPOINTS_KEY];
		endpointHealth = stored && typeof stored === 'object' ? stored : {};
	} catch {
		endpointHealth = {};
	}
	return endpointHealth;
}

function saveHealth(next) {
	endpointHealth = next;
	sessionArea.set({ [ENDPOINTS_KEY]: next }).catch(() => {});
}

/**
 * One read on `chainId`'s endpoints: the live ones only, or — when every one
 * is cooling down — the one back soonest (`orderEndpoints`, RJ20). A transport
 * failure moves to the next endpoint and cools the failed one
 * (`30 s · 2^(n−1)`, at most 300 s), logged by its kind (an 8 s timer abort
 * is `timeout`, G64); a JSON-RPC error IS an answer (a revert is what the page
 * asked to learn) and goes back as is. When nothing answered, the page gets
 * the chain by name and no engine text (G33).
 */
async function readChain(method, params, catalog, chainId) {
	const endpoints = chainEndpoints(catalog, chainId, BUNDLER_METHODS.has(method));
	if (endpoints.length === 0) {
		return { error: rpcError(ERR.CHAIN_NOT_ADDED, `Vela has no endpoint for chain ${chainId}`) };
	}
	const order = orderEndpoints(endpoints, await health(), Date.now());
	let tried = 0;
	for (const url of order) {
		tried += 1;
		const started = Date.now();
		const timer = AbortSignal.timeout(READ_TIMEOUT_MS);
		let status;
		try {
			const response = await fetch(url, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
				signal: timer
			});
			if (!response.ok) {
				status = response.status;
				throw new Error('http');
			}
			const body = await response.json();
			const took = Date.now() - started;
			saveHealth(endpointAnswered(await health(), url));
			if (tried > 1) void swlog.log('read.failover', { chain: chainId, host: url, tried });
			if (took >= READ_SLOW_MS)
				void swlog.log('read.slow', { chain: chainId, host: url, ms: took });
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
			const kind = readFailureKind(error, status, timer.aborted);
			saveHealth(endpointFailed(await health(), url, Date.now()));
			void swlog.log('read.fail', { chain: chainId, host: url, kind });
		}
	}
	void swlog.log('read.exhausted', { chain: chainId, tried });
	return {
		error: rpcError(ERR.INTERNAL, unreachableChainMessage(chainNameOf(catalog, chainId), chainId))
	};
}

/** Remember an operation hash the page was answered with (RF3). */
async function rememberOp(result, opHash) {
	const entry = opRecord(result, opHash, Date.now());
	if (!entry) return;
	try {
		await chrome.storage.local.set({ [entry.key]: entry.value });
	} catch {
		/* not remembered: the page's receipt reads for it stay untranslated */
	}
}

/**
 * A receipt read for an operation hash this wallet handed out: the bundler's
 * receipt names the real transaction, and the page's own method is forwarded
 * for THAT hash on the chain the operation went to. Not landed yet → `null`,
 * so the page keeps polling as it would for an unmined transaction.
 */
async function translatedReceipt(method, chainId, opHash, catalog) {
	const found = await readChain('eth_getUserOperationReceipt', [opHash], catalog, chainId);
	if (found.error) return { result: null };
	const real = realTxHash(found.result);
	if (!real) return { result: null };
	return readChain(method, [real], catalog, chainId);
}

async function forwardRead(method, params, origin) {
	const all = await readLocal([
		CHAINS_KEY,
		CHAIN_PREFIX + origin,
		PERM_PREFIX + origin,
		EXT_CACHE_KEY
	]);
	const catalog = all[CHAINS_KEY];
	if (!catalog) return { error: NOT_OPENED() };
	const asked = receiptLookupHash(method, params);
	if (asked) {
		const stored = await readLocal(opKey(asked));
		const entry = liveOpEntry(stored[opKey(asked)], Date.now());
		if (entry) return translatedReceipt(method, entry.chainId, asked, catalog);
	}
	return readChain(method, params, catalog, chainOf(origin, all));
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
 * A grant or a chain pick changed in storage — announce it. The grant is the
 * core's; what each change means to the page is the core's `DpermPageEvent`
 * vocabulary, mirrored here.
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
		reply({ error: rpcError(ERR.INVALID_PARAMS, 'Malformed request') });
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
			void answerFromSnapshot(request.method, origin).then(reply);
			return;
		case 'switch':
		case 'addChain':
			void switchChain(request.method, request.params, origin).then(reply);
			return;
		case 'revoke':
			void chrome.storage.local
				.remove(PERM_PREFIX + origin)
				.then(() => reply({ result: null }))
				.catch(() => reply({ error: rpcError(ERR.INTERNAL, 'Could not record the disconnect') }));
			return;
		case 'watchAsset':
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

	// The surface FIRST, synchronously: the user gesture that lets the side
	// panel open does not survive an `await`. `surface: 'panel'` is recorded
	// at once (RB8) — the panel may ask what it owes before `open` resolves.
	const usePanel = surfacePreference !== 'window' && typeof chrome.sidePanel?.open === 'function';
	const panelAttempt = usePanel ? openPanelNow(tabId) : Promise.resolve('failed');
	const record = newRecord({
		request,
		sender: { ...sender, origin },
		now: Date.now(),
		surface: usePanel ? 'panel' : 'window',
		surfaceWindowId: usePanel ? sender.tab?.windowId : undefined
	});
	// The page learns only that the request was taken; the answer comes by
	// message, by document, whenever the person decides (contract §14).
	reply({ accepted: true });
	void admit(record, panelAttempt);
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
	if (!message || typeof message !== 'object') return;

	if (message.type === 'rpc') {
		route(message, sender, sendResponse);
		return true; // a read's answer comes later
	}

	if (message.type === 'abandon') {
		// content.js's own deadline passed (RB11): the page has answered itself.
		const tabId = sender.tab?.id;
		if (tabId === undefined || typeof message.id !== 'string') return;
		void loaded.then(() => {
			const rid = `${tabId}:${message.id}`;
			const record = records.get(rid);
			if (record && (!record.documentId || record.documentId === sender.documentId)) {
				void settle(rid, 'expired');
			}
		});
		return;
	}

	// ---- the wallet's own half (extension pages only) ----------------------

	const fromWallet =
		!sender.tab?.url?.startsWith('http') &&
		typeof sender.url === 'string' &&
		sender.url.startsWith(chrome.runtime.getURL(''));
	if (!fromWallet) return;

	if (message.type === 'requestDetail') {
		// The fallback for a window that could not hold its port.
		void loaded.then(() => {
			const record = records.get(message.rid);
			sendResponse(record ? publicRecord(record) : null);
		});
		return true;
	}

	if (message.type === 'requestAnswer') {
		// The fallback for a surface whose port was down at the moment it answered.
		void loaded
			.then(() =>
				answer(message.rid, { result: message.result, error: message.error }, message.opHash)
			)
			.then((delivered) => sendResponse({ delivered }));
		return true;
	}

	return undefined;
});
