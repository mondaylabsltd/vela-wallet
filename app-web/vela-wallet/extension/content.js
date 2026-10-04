/**
 * The page bridge (spec 027 T320) — isolated world.
 *
 * Ported in PART from packages/safari-extension/src/content.js @ 52ad8fa9:
 * the provider transport and the dead-worker-safe background round-trip. The
 * wallet is a surface this extension owns (spec 027 D34), so this file only
 * carries messages, and it carries exactly two facts the page cannot forge:
 * WHICH tab a request came from, and WHICH origin sent it. Both are the
 * browser's, added on this side of the boundary.
 *
 * Spec 082 (RB3, RB4, RB6, RB11) gave it the page's half of a request's life:
 *
 *   - a sign/connect/add-network request is handed to the worker, which says only
 *     `accepted`; the answer arrives later as a message addressed to THIS
 *     document (`{type:'answer'}`), whenever the person decides;
 *   - while the page owes such an answer it holds a `vela.doc` port, which is
 *     how the worker knows the page is still there; if the worker restarts,
 *     the port closes and is reopened (0 / 250 / 1000 / 3000 ms), which wakes
 *     the new worker — it still holds the request, and it resumes (RB4);
 *   - every request has a deadline, and the page's is strictly after the
 *     worker's: 5 min + 5 s unclaimed, 5 min after the wallet claims it;
 *   - a dropped channel never reaches the page as Chrome's own words ("The
 *     message channel closed before a response was received", G19): a read
 *     is retried once, a read that SENDS something is never retried, and a
 *     sign/connect is retried once and then answered 4900 — it may have
 *     happened (`droppedChannelAnswer`).
 *
 * One answer per id, whichever of these gets there first.
 */
import {
	CHANNEL,
	CONTENT_GRACE_MS,
	DOC_PORT,
	RECONNECT_BACKOFF_MS,
	REQUEST_TTL_MS,
	HELD_BUCKETS,
	classifyMethod,
	droppedChannelAnswer,
	settleError
} from './lib/protocol.js';

(() => {
	const ORIGIN = window.location.origin;

	/** Answer the MAIN world. Targeted at our own origin — never a wildcard. */
	function respond(id, result) {
		window.postMessage({ ch: CHANNEL, dir: 'res', id, result }, ORIGIN);
	}
	function respondErr(id, error) {
		window.postMessage({ ch: CHANNEL, dir: 'res', id, error }, ORIGIN);
	}
	function emitEvt(event, data) {
		window.postMessage({ ch: CHANNEL, dir: 'evt', event, data }, ORIGIN);
	}

	/** Is this script still attached to a live extension (not reloaded/updated)? */
	function extensionAlive() {
		try {
			return typeof chrome !== 'undefined' && !!chrome.runtime?.id;
		} catch {
			return false;
		}
	}

	// ---- what this page still owes an answer for -----------------------------

	/** page request id → { bucket, method, timer } — sign/connect only. */
	const owed = new Map();
	/** Ids answered already, so a late second answer is refused (one answer). */
	const answered = new Set();

	/** Answer the page once; `false` when this id is not (or no longer) owed. */
	function answerOwed(id, payload) {
		const entry = owed.get(id);
		if (!entry) return false;
		clearTimeout(entry.timer);
		owed.delete(id);
		answered.add(id);
		if (payload.error) respondErr(id, payload.error);
		else respond(id, payload.result === undefined ? null : payload.result);
		if (owed.size === 0) closePort();
		return true;
	}

	function arm(id, ms) {
		const entry = owed.get(id);
		if (!entry) return;
		clearTimeout(entry.timer);
		entry.timer = setTimeout(() => expire(id), ms);
	}

	/** This page's deadline passed: answer it, and tell the worker to let go. */
	function expire(id) {
		if (!answerOwed(id, { error: settleError('expired') })) return;
		if (!extensionAlive()) return;
		try {
			const sent = chrome.runtime.sendMessage({ type: 'abandon', id });
			if (sent && typeof sent.catch === 'function') sent.catch(() => {});
		} catch {
			/* the worker will settle it on its own limit */
		}
	}

	/** Every owed request, answered with `cause` (the extension went away). */
	function answerAll(cause) {
		for (const id of [...owed.keys()]) answerOwed(id, { error: settleError(cause) });
	}

	// ---- the page's port -----------------------------------------------------

	let port = null;
	let failures = 0;
	let reconnectTimer = null;

	function connect() {
		if (port || owed.size === 0) return;
		if (!extensionAlive()) {
			answerAll('updated');
			return;
		}
		let opened;
		try {
			opened = chrome.runtime.connect({ name: DOC_PORT });
		} catch {
			scheduleReconnect();
			return;
		}
		port = opened;
		const upAt = Date.now();
		opened.onDisconnect.addListener(() => {
			void chrome.runtime.lastError;
			if (port !== opened) return;
			port = null;
			// A port that stayed up a while was a worker restart, not a failure
			// to reach one: start the backoff again from zero.
			if (Date.now() - upAt > 5_000) failures = 0;
			scheduleReconnect();
		});
	}

	function scheduleReconnect() {
		if (owed.size === 0 || reconnectTimer) return;
		if (!extensionAlive()) {
			answerAll('updated');
			return;
		}
		const delay = RECONNECT_BACKOFF_MS[Math.min(failures, RECONNECT_BACKOFF_MS.length - 1)];
		failures += 1;
		reconnectTimer = setTimeout(() => {
			reconnectTimer = null;
			connect();
		}, delay);
	}

	/** Nothing owed: say so first, so the worker does not read the close as the page leaving. */
	function closePort() {
		clearTimeout(reconnectTimer);
		reconnectTimer = null;
		failures = 0;
		const closing = port;
		port = null;
		if (!closing) return;
		try {
			closing.postMessage({ type: 'idle' });
			closing.disconnect();
		} catch {
			/* already gone */
		}
	}

	// ---- one round-trip to the worker ----------------------------------------

	/**
	 * `{ ok: true, value }`, or `{ ok: false }` when the channel dropped — the
	 * worker was evicted mid-call (a rejection, or a reply of `undefined`).
	 * Chrome's error text is dropped here and never travels further.
	 */
	async function send(message, ms) {
		let timer;
		try {
			const reply = await Promise.race([
				chrome.runtime.sendMessage(message),
				new Promise((resolve) => {
					timer = setTimeout(() => resolve({ __timeout: true }), ms);
				})
			]);
			if (reply && reply.__timeout) return { ok: true, timeout: true };
			if (reply === undefined) return { ok: false };
			return { ok: true, value: reply };
		} catch {
			return { ok: false };
		} finally {
			clearTimeout(timer);
		}
	}

	/** A sign or connect request: accepted now, answered by message later. */
	async function askOwed(d, bucket) {
		if (owed.has(d.id) || answered.has(d.id)) return;
		const sentAt = Date.now();
		owed.set(d.id, { bucket, method: d.method, timer: null });
		arm(d.id, REQUEST_TTL_MS + CONTENT_GRACE_MS);
		connect();
		const message = { type: 'rpc', id: d.id, method: d.method, params: d.params ?? [], sentAt };
		for (let attempt = 0; ; attempt += 1) {
			if (!extensionAlive()) {
				answerOwed(d.id, { error: settleError('updated') });
				return;
			}
			const reply = await send(message, REQUEST_TTL_MS + CONTENT_GRACE_MS);
			if (!owed.has(d.id)) return; // answered meanwhile
			if (reply.ok) {
				if (reply.timeout || reply.value?.accepted) return; // the answer comes by message
				if (reply.value?.error) answerOwed(d.id, { error: reply.value.error });
				else answerOwed(d.id, { result: reply.value?.result ?? null });
				return;
			}
			const next = droppedChannelAnswer(bucket, d.method, attempt);
			if (next.retry) continue;
			answerOwed(d.id, { error: next.error });
			return;
		}
	}

	/** Anything else: the worker's reply IS the answer. */
	async function askRead(d, bucket) {
		const message = { type: 'rpc', id: d.id, method: d.method, params: d.params ?? [] };
		for (let attempt = 0; ; attempt += 1) {
			if (!extensionAlive()) {
				respondErr(d.id, settleError('updated'));
				return;
			}
			const reply = await send(message, REQUEST_TTL_MS);
			if (reply.ok) {
				if (reply.timeout) respondErr(d.id, settleError('expired'));
				else if (reply.value?.error) respondErr(d.id, reply.value.error);
				else respond(d.id, reply.value ? reply.value.result : null);
				return;
			}
			const next = droppedChannelAnswer(bucket, d.method, attempt);
			if (next.retry) continue;
			respondErr(d.id, next.error);
			return;
		}
	}

	// ---- page → worker -------------------------------------------------------

	window.addEventListener('message', (ev) => {
		// Same-window only, on our channel. The page shares this world's DOM but
		// not this world's scope; tagging keeps us clear of other providers and of
		// nested-iframe traffic.
		if (ev.source !== window) return;
		const d = ev.data;
		if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
		// The page supplies id, method and params; the worker re-checks their
		// shape. It does NOT supply the origin: `sender.origin` on the other side
		// is the browser's own fact, and that is the one a grant is keyed on.
		const bucket = typeof d.method === 'string' ? classifyMethod(d.method) : 'unsupported';
		if (typeof d.id === 'string' && HELD_BUCKETS.has(bucket)) {
			void askOwed(d, bucket);
		} else {
			void askRead(d, bucket);
		}
	});

	// ---- worker → page -------------------------------------------------------

	chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
		if (!message || typeof message !== 'object') return undefined;
		switch (message.type) {
			case 'evt':
				emitEvt(message.event, message.data);
				return undefined;
			case 'answer': {
				// Replied SYNCHRONOUSLY: the worker reads `ok` as "the page has it".
				const ok = answerOwed(
					message.id,
					message.error ? { error: message.error } : { result: message.result }
				);
				sendResponse({ ok });
				return undefined;
			}
			case 'claimed': {
				// The wallet is acting on it (a grant, a passkey, a submit): the
				// page waits 5 min from now, never less than the worker does.
				const owns = owed.has(message.id);
				if (owns) arm(message.id, REQUEST_TTL_MS);
				sendResponse({ ok: owns });
				return undefined;
			}
			case 'alive': {
				const ids = Array.isArray(message.ids) ? message.ids : [];
				sendResponse({ alive: ids.filter((id) => owed.has(id)) });
				return undefined;
			}
			default:
				return undefined;
		}
	});
})();
