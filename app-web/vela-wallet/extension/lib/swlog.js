/**
 * The service worker's log (spec 082 RB14, contract §15, FR-018/019).
 *
 * Before 082 the worker wrote one line in its whole life, and an evicted
 * worker took even that with it — so "the panel showed nothing" (G23) could
 * not be told apart from "the request never arrived". Now every step of a
 * request's life is one line:
 *
 *   [vela-sw] <iso> <event> k=v k=v…
 *
 * on the console, in a 200-line ring in `storage.session` (`vela.sw.log`, so
 * it outlives an eviction and dies with the browser), and as a counter per
 * `<event>.<cause>` in `vela.sw.counts` — the only part the bug report sends
 * (`services/bug-report.ts`).
 *
 * What is never written, whatever a caller passes: request params, results,
 * signatures, addresses, hashes, URL paths or queries. A field whose value
 * looks like any of those is dropped, not masked; an endpoint is reduced to
 * its host. `swlog.test.ts` feeds it each of those and reads the ring back.
 */

/** The ring, in storage.session. */
export const SW_LOG_KEY = 'vela.sw.log';
/** The counters, in storage.session: `<event>[.<cause>]` → n. */
export const SW_COUNTS_KEY = 'vela.sw.counts';
/** How many lines the ring keeps. */
export const SW_LOG_CAP = 200;

/** The fixed events (contract §15). Anything else is not logged. */
export const SW_EVENTS = new Set([
	'sw.start',
	'req.arrived',
	'req.surface',
	'req.shown',
	'req.claim',
	'req.answered',
	'req.settled',
	'req.resumed',
	'panel.up',
	'panel.down',
	'read.fail',
	'read.failover',
	'read.slow',
	'read.exhausted'
]);

/** A 0x-blob of 8+ hex digits: an address, a hash, a signature, calldata. */
const HEXISH = /0x[0-9a-f]{8,}/i;
/** What a plain field value may be: a word, a number, a short id. */
const PLAIN = /^[\w.:-]{1,64}$/;
/** A bare host, optionally with a port. */
const HOST = /^[a-z0-9.-]{1,253}(:\d{1,5})?$/i;

/** The host of an endpoint — never its path, query or credentials. */
export function hostOf(value) {
	if (typeof value !== 'string' || !value) return null;
	try {
		const url = new URL(value);
		return url.host || null;
	} catch {
		return HOST.test(value) ? value.toLowerCase() : null;
	}
}

/**
 * The value as it may be written, or `null` when it may not be written at
 * all. `host` (and anything ending in `Host`) is reduced to a host.
 */
export function safeValue(key, value) {
	if (typeof value === 'number') return Number.isFinite(value) ? String(value) : null;
	if (typeof value === 'boolean') return value ? 'true' : 'false';
	if (typeof value !== 'string') return null;
	if (key === 'host' || key.endsWith('Host')) return hostOf(value);
	if (HEXISH.test(value)) return null;
	if (!PLAIN.test(value)) return null;
	return value;
}

/** One line, or `null` for an event that is not on the list. */
export function formatLine(event, fields, now) {
	if (!SW_EVENTS.has(event)) return null;
	const parts = [`[vela-sw] ${new Date(now).toISOString()} ${event}`];
	for (const [key, value] of Object.entries(fields ?? {})) {
		if (!/^[a-zA-Z][\w]{0,31}$/.test(key)) continue;
		const safe = safeValue(key, value);
		if (safe !== null) parts.push(`${key}=${safe}`);
	}
	return parts.join(' ');
}

/** The counter an event bumps: `<event>.<cause>` when it has one. */
export function counterKey(event, fields) {
	const cause = safeValue('cause', fields?.cause ?? fields?.kind ?? fields?.verdict);
	return cause ? `${event}.${cause}` : event;
}

/** `ring` with `line` appended, the oldest dropped past the cap. */
export function appendLine(ring, line, cap = SW_LOG_CAP) {
	const lines = Array.isArray(ring) ? ring.filter((l) => typeof l === 'string') : [];
	lines.push(line);
	return lines.length > cap ? lines.slice(lines.length - cap) : lines;
}

/**
 * The logger. `storage` is `chrome.storage.session` (or a test double with
 * `get`/`set`); `sink` is where the console line goes.
 *
 * Writes are chained so two lines logged in one tick cannot overwrite each
 * other's ring; a storage failure loses the stored copy, never the console
 * line, and never throws into the caller.
 *
 * @param {{
 *   storage?: { get(keys: string[]): Promise<Record<string, unknown>>, set(items: Record<string, unknown>): Promise<void> },
 *   sink?: { info(line: string): unknown },
 *   now?: () => number
 * }} [options]
 */
export function createSwLog({ storage, sink = console, now = () => Date.now() } = {}) {
	let chain = Promise.resolve();
	function log(event, fields = {}) {
		const line = formatLine(event, fields, now());
		if (line === null) return chain;
		try {
			sink.info(line);
		} catch {
			/* a console that throws is not this file's business */
		}
		if (!storage) return chain;
		const key = counterKey(event, fields);
		chain = chain
			.then(async () => {
				const all = await storage.get([SW_LOG_KEY, SW_COUNTS_KEY]);
				const counts =
					all?.[SW_COUNTS_KEY] && typeof all[SW_COUNTS_KEY] === 'object'
						? { ...all[SW_COUNTS_KEY] }
						: {};
				counts[key] = (Number.isFinite(counts[key]) ? counts[key] : 0) + 1;
				await storage.set({
					[SW_LOG_KEY]: appendLine(all?.[SW_LOG_KEY], line),
					[SW_COUNTS_KEY]: counts
				});
			})
			.catch(() => {});
		return chain;
	}
	return { log, flush: () => chain };
}
