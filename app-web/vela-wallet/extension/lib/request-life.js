/**
 * The life of one extension request, as pure rules (spec 082 RB1–RB11,
 * data-model §4, contract §14).
 *
 * The service worker owns a request from arrival to answer: only it sees both
 * the page that asked and the surface that answers. It cannot load the core on
 * every wake, so the rules it applies live here — dependency-free, with no
 * extension or DOM API — and `request-life.test.ts` drives every branch. The
 * two numbers that matter to money (the 5-minute limit and the 4900 code) are
 * pinned to the core by `instant.test.ts`.
 *
 * A record is what `background.js` keeps in `storage.session` under
 * `vela.req.<tabId>:<pageRequestId>`:
 *
 *   { v, rid, id, method, params, origin, tabId, windowId, documentId,
 *     sentAt, at, surface, surfaceWindowId, state, claimedAt?, phase?,
 *     opHash?, chainId?, windowOpening?, queued? }
 *
 * `windowOpening` / `queued` are the request WINDOW's queue (spec 094 S7): one
 * window per site, the requests behind it `queued` until it passes to them.
 *
 * `state` goes `created` → `shown` → `claimed`; the record leaves the ledger
 * when it is answered or settled, never otherwise. `phase` is the last live
 * claim's; a `submit` claim may carry the operation's hash and chain
 * (`opHash`, `chainId`, RJ2), sent right before the relay POST.
 */
import { isHash32 } from './op-receipt.js';
import { REQUEST_TTL_MS, SETTLE, maybeSentPayload, settleError } from './protocol.js';

/** The record schema this worker writes. */
export const RECORD_VERSION = 1;

/**
 * The record for a request that just arrived. `at` is the 5-minute clock:
 * the page's own send time, never later than now (a page cannot buy itself
 * time by claiming a send in the future).
 */
export function newRecord({ request, sender, now, surface, surfaceWindowId }) {
	const sentAt =
		typeof request.sentAt === 'number' && Number.isFinite(request.sentAt) ? request.sentAt : now;
	const tabId = sender.tab?.id;
	return {
		v: RECORD_VERSION,
		rid: `${tabId}:${request.id}`,
		id: request.id,
		method: request.method,
		params: request.params,
		origin: sender.origin,
		tabId,
		windowId: sender.tab?.windowId,
		documentId: sender.documentId,
		sentAt,
		at: Math.min(sentAt, now),
		surface,
		surfaceWindowId,
		state: 'created'
	};
}

/** When a record stops being answerable: 5 min after arrival, or after its claim. */
export function deadlineOf(record, ttlMs = REQUEST_TTL_MS) {
	const from =
		record.state === 'claimed' && typeof record.claimedAt === 'number'
			? record.claimedAt
			: record.at;
	return from + ttlMs;
}

/**
 * The request a PANEL in `windowId` should show next: the oldest one owed on
 * a panel of that window, whichever tab asked (RB7 — one queue per window, no
 * active-tab filter, no automatic tab switching; the sheet names the site).
 */
export function nextForWindow(records, windowId) {
	let best = null;
	for (const record of records) {
		if (!record || record.surface !== 'panel') continue;
		if (record.surfaceWindowId !== windowId) continue;
		if (!best || record.at < best.at || (record.at === best.at && record.rid < best.rid)) {
			best = record;
		}
	}
	return best;
}

// ---- one request window per site (spec 094 S7) -------------------------------

/**
 * Does this record hold its site's request window — opened, or being opened?
 * A record whose surface is `window` but that has not reached the window yet
 * holds nothing: the OLDEST request of a site is the one that opens it.
 */
export function holdsWindow(record) {
	return (
		!!record &&
		record.surface === 'window' &&
		record.queued !== true &&
		(record.surfaceWindowId !== undefined || record.windowOpening === true)
	);
}

/**
 * The request holding `origin`'s window, other than `exceptRid`, or `null`.
 * A page that fires twelve requests with no click gets one window, which
 * takes them in turn — not twelve focused popups (089 F03).
 */
export function windowHolder(records, origin, exceptRid) {
	for (const record of records) {
		if (record && record.rid !== exceptRid && record.origin === origin && holdsWindow(record)) {
			return record;
		}
	}
	return null;
}

/** The oldest request queued behind `origin`'s window, or `null`. */
export function nextQueued(records, origin) {
	let best = null;
	for (const record of records) {
		if (!record || record.queued !== true || record.surface !== 'window') continue;
		if (record.origin !== origin) continue;
		if (!best || record.at < best.at || (record.at === best.at && record.rid < best.rid)) {
			best = record;
		}
	}
	return best;
}

/**
 * What a site's queue does when the request holding its window leaves the
 * ledger (`ended`, ended by `cause`; `records` no longer holds it):
 *
 *   - the person closed the window (`surface_closed`) — every request queued
 *     behind it is settled the same way, so a flood does not raise a window
 *     per request after each ✕;
 *   - anything else (an answer, the page leaving, the time running out) — the
 *     oldest queued request takes the window over.
 *
 * Returns `{ settle: rid[], next: record | null }`.
 */
export function queueAfter(records, ended, cause) {
	if (!ended || ended.surface !== 'window' || ended.queued === true)
		return { settle: [], next: null };
	const rest = records.filter((record) => record && record.rid !== ended.rid);
	if (cause === 'surface_closed') {
		return {
			settle: rest
				.filter(
					(record) =>
						record.queued === true && record.surface === 'window' && record.origin === ended.origin
				)
				.map((record) => record.rid),
			next: null
		};
	}
	return { settle: [], next: nextQueued(rest, ended.origin) };
}

/**
 * Is this request still live enough to act on (RB5)? Asked at the three
 * points a request becomes harder to take back: `approve` (before a connect
 * grant is written), `sign` (before the passkey), `submit` (after the passkey,
 * before the relay POST).
 *
 * Live only if the record exists, the caller is the surface it was given to,
 * its page is still attached, and — for approve/sign — it is under the limit;
 * a submit needs an earlier claim instead (the signature exists; its age no
 * longer matters, the page's presence does).
 *
 * Returns `{ live: true }` or `{ live: false, cause }`; `cause` is a settle
 * cause where one applies, else `gone` / `wrong_surface` / `not_claimed`.
 */
export function claimVerdict(record, { now, ttlMs = REQUEST_TTL_MS, docAttached, caller, phase }) {
	if (!record) return { live: false, cause: 'gone' };
	if (!callerOwns(record, caller)) return { live: false, cause: 'wrong_surface' };
	if (!docAttached) return { live: false, cause: 'page_left' };
	if (phase === 'submit') {
		return record.state === 'claimed' ? { live: true } : { live: false, cause: 'not_claimed' };
	}
	if (phase !== 'approve' && phase !== 'sign') return { live: false, cause: 'gone' };
	if (now - record.at >= ttlMs) return { live: false, cause: 'expired' };
	return { live: true };
}

/**
 * The record after a LIVE claim (RB5): `claimed` from now, at `phase`. A
 * `submit` claim that carries the operation hash and its chain keeps both
 * (RJ2) — a hash that is not a 32-byte hex, or a chain that is not a positive
 * integer, is not kept. Pure: returns a new record.
 *
 * @param {object} record
 * @param {{ phase: string, now: number, opHash?: unknown, chainId?: unknown }} claim
 */
export function withClaim(record, { phase, now, opHash, chainId }) {
	const next = { ...record, state: 'claimed', claimedAt: now, phase };
	if (phase === 'submit' && isHash32(opHash)) {
		next.opHash = opHash;
		if (Number.isInteger(chainId) && chainId > 0) next.chainId = chainId;
	}
	return next;
}

/**
 * The operation hash a request may already have been sent under (RJ2), or
 * `null`: only a record claimed for SUBMIT whose claim carried the hash. From
 * that claim on, the bytes may be on their way to the relay, so a request that
 * ends without its surface's answer is answered "not confirmed yet", naming
 * this hash (`maybeSentPayload`; a batch: the hash is its id) — ruling 1 +
 * RA2, "may have been sent", and 083 — and never 4900, which a dApp reads as
 * "not sent" and pays again. Before the hash exists nothing was sent, and 4900
 * is right.
 */
export function maybeSentHash(record) {
	if (!record || record.state !== 'claimed' || record.phase !== 'submit') return null;
	return isHash32(record.opHash) ? record.opHash : null;
}

/**
 * What the page is told when the SURFACE answers (RJ2). A surface answers what
 * it still owes with the close settlement — 4900, "it may have happened" (RB6)
 * — when it is torn down: the panel's page going (`pagehide`), or an in-app
 * navigation off the wallet unmounting the request host while the port stays
 * up. For a claimed submit that carried its operation hash that is surface
 * loss like any other, so the page is told it is not confirmed yet (083; a
 * batch: its id), never 4900 (which a dApp reads as "not sent" and pays
 * again). Every other answer — a result, or a
 * real error such as RJ3's "refused; nothing was sent" — goes as the surface
 * gave it.
 *
 * Returns `{ payload, maybeSent }`: `maybeSent` is the hash when it replaced
 * the settlement, else `null`.
 *
 * @param {object} record
 * @param {{ result?: unknown, error?: { code?: unknown } }} payload
 */
export function surfaceAnswer(record, payload) {
	const hash = maybeSentHash(record);
	if (hash && payload?.error && payload.error.code === SETTLE.surface_closed.code) {
		return { payload: maybeSentPayload(record.method, hash), maybeSent: hash };
	}
	return { payload, maybeSent: null };
}

/** `{rid, cause}` — plus the answer a may-have-been-sent request is owed. */
function ending(record, cause) {
	const hash = maybeSentHash(record);
	return hash
		? { rid: record.rid, cause, answer: maybeSentPayload(record.method, hash), opHash: hash }
		: { rid: record.rid, cause };
}

/**
 * Does `caller` — `{kind:'panel', windowId}` or `{kind:'window', rid}` — own
 * this record? A panel owns its window's panel queue; a dedicated window owns
 * the one request it was opened for.
 */
export function callerOwns(record, caller) {
	if (!record || !caller) return false;
	if (caller.kind === 'panel') {
		return record.surface === 'panel' && record.surfaceWindowId === caller.windowId;
	}
	if (caller.kind === 'window') {
		return record.surface === 'window' && record.rid === caller.rid;
	}
	return false;
}

/**
 * What a worker that just started does with each record it finds (RB4:
 * resume, don't settle). `facts`:
 *   - `now`, `ttlMs`;
 *   - `panelWindows`: the window ids that have a side panel open
 *     (`runtime.getContexts SIDE_PANEL`), or `null` when that could not be
 *     read — then nothing is settled on that account;
 *   - `openWindows`: the window ids that exist, or `null` when unknown.
 *
 * Returns `[{ rid, action: 'settle', cause, answer? } | { rid, action: 'probe' }]`:
 * a probe asks the page, by `documentId`, whether it still owns the id
 * (`alive`) and keeps the record if it does. A settle carries `answer` and
 * `opHash` for a claimed submit that may have been sent (RJ2, 083): the page
 * is told `maybeSentPayload` — "not confirmed yet" — not 4900.
 */
export function recoveryPlan(records, { now, ttlMs = REQUEST_TTL_MS, panelWindows, openWindows }) {
	const plan = [];
	const settle = (record, cause) => plan.push({ action: 'settle', ...ending(record, cause) });
	for (const record of records) {
		if (!record || typeof record.rid !== 'string') continue;
		if (now >= deadlineOf(record, ttlMs)) {
			settle(record, 'expired');
			continue;
		}
		if (record.surface === 'panel' && panelWindows && !panelWindows.has(record.surfaceWindowId)) {
			settle(record, 'surface_closed');
			continue;
		}
		// Queued behind its site's window (spec 094 S7): still waiting for one —
		// kept if its page is, and given a window once the worker is up.
		if (record.surface === 'window' && record.queued === true) {
			plan.push({ rid: record.rid, action: 'probe' });
			continue;
		}
		if (
			record.surface === 'window' &&
			openWindows &&
			(record.surfaceWindowId === undefined || !openWindows.has(record.surfaceWindowId))
		) {
			settle(record, 'surface_closed');
			continue;
		}
		plan.push({ rid: record.rid, action: 'probe' });
	}
	return plan;
}

/**
 * The records one browser event ends, and why (data-model §4's trigger table):
 *
 *   - `{type:'doc_closed', documentId}` — the page's `vela.doc` port closed
 *     while the worker lives: the page left (reload, navigation, bfcache);
 *   - `{type:'tab_removed', tabId}` / `{type:'tab_replaced', tabId}` — the
 *     backstop for the same;
 *   - `{type:'surface_closed', caller}` — the panel's or window's
 *     `vela.surface` port closed (Chrome's ✕, a panel reload): every request
 *     that surface owed (RB10);
 *   - `{type:'window_removed', windowId}` — a window closed: its dedicated
 *     request window's request, and every request its panel owed.
 *
 * Returns `[{ rid, cause, answer? }]`. When the SURFACE went (not the page), a
 * claimed submit that may have been sent carries `answer` (`maybeSentPayload`) and `opHash` —
 * RJ2 supersedes RB10 for those records only.
 */
export function affectedBy(records, event) {
	const out = [];
	for (const record of records) {
		if (!record) continue;
		let cause = null;
		switch (event.type) {
			case 'doc_closed':
				if (record.documentId !== undefined && record.documentId === event.documentId) {
					cause = 'page_left';
				}
				break;
			case 'tab_removed':
			case 'tab_replaced':
				if (record.tabId === event.tabId) cause = 'page_left';
				break;
			case 'surface_closed':
				if (callerOwns(record, event.caller)) cause = 'surface_closed';
				break;
			case 'window_removed':
				if (record.surfaceWindowId === event.windowId) cause = 'surface_closed';
				break;
			default:
				break;
		}
		if (!cause) continue;
		out.push(cause === 'surface_closed' ? ending(record, cause) : { rid: record.rid, cause });
	}
	return out;
}

/**
 * Where a request is shown after the side panel was asked to open (RB8).
 *
 * `opened`: `'ok'` (the panel opened), `'failed'` (the open threw or was
 * refused — no user gesture) or `'timeout'` (it neither opened nor failed in
 * `PANEL_OPEN_WAIT_MS`). `panelOpen`: a side panel is already open in that
 * window. A saved `window` preference wins over everything.
 *
 * Returns `'panel'` or `'window'`.
 */
export function surfaceAfterOpen({ opened, panelOpen, preference }) {
	if (preference === 'window') return 'window';
	if (opened === 'ok') return 'panel';
	// The open failed or hung, but a panel is already up in that window: keep
	// it there — the storage write wakes it — instead of popping a window
	// beside a panel the person is looking at (EX2).
	return panelOpen ? 'panel' : 'window';
}

/**
 * What the page is told when its request ends without a decision (RB6):
 * always 4900 — "it may have happened" — with the cause's plain English.
 */
export function settlement(cause) {
	return settleError(cause in SETTLE ? cause : 'surface_closed');
}
