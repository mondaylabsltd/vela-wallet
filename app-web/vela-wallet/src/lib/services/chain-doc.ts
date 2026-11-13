/**
 * A chain's registry document (`/chains/eip155-{id}.json`), fetched ONCE for
 * every reader of it (perf-7).
 *
 * Two readers ask for the same document: the balance read (its stablecoins,
 * wrapped coin and DEX — `chain-tokens.ts`) and the RPC pool's endpoint
 * collection (its index of public nodes — `chain-registry.ts`). Each fetched
 * it on its own, so every load asked for every chain's document twice; and
 * while the host hung, every balance round (every 20–26 s) asked again, each
 * ask holding its full timeout. Here:
 *
 * - one request per chain at a time — a second reader joins the first;
 * - a definitive answer (the document, or a 404 saying there is none) is kept
 *   {@link DOC_TTL_MS} for both;
 * - an ask that got NO answer (offline, refused, timed out) is not repeated
 *   at once: the chain rests {@link QUIET_BASE_MS}, doubling to
 *   {@link QUIET_CAP_MS}, and an ask inside that rest is answered "unread"
 *   without a request — the truth: nothing was read. Any answer ends the
 *   rest, and so does the browser coming back online. A server that answered
 *   with an error (5xx, 429…) is not resting: it answered, and the next ask
 *   reads again.
 *
 * Only documents are carried here; what they mean is each reader's (and the
 * core's) to say.
 */

import { getEthereumDataURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';

/** Definitive answers are kept this long (the readers' 30 min). */
export const DOC_TTL_MS = 30 * 60 * 1000;
/** The first rest after an ask that got no answer. */
export const QUIET_BASE_MS = 10_000;
/** The longest rest. */
export const QUIET_CAP_MS = 300_000;

/**
 * `doc`: the server answered with a JSON object. `absent`: it answered that
 * there is none (404). `unread`: nothing definitive came back.
 */
export type ChainDoc =
	| { kind: 'doc'; body: Record<string, unknown> }
	| { kind: 'absent' }
	| { kind: 'unread'; cause: unknown };

type Kept = { read: Exclude<ChainDoc, { kind: 'unread' }>; at: number };

const kept = new Map<string, Kept>();
const inFlight = new Map<string, Promise<ChainDoc>>();
const resting = new Map<string, { failures: number; until: number; cause: unknown }>();

if (typeof window !== 'undefined') window.addEventListener('online', () => resting.clear());

/** The document's URL — also its key: a re-pointed endpoint is a new document. */
function docUrl(chainId: number): string {
	return `${getEthereumDataURL()}/chains/eip155-${chainId}.json`;
}

async function fetchDoc(url: string): Promise<{ read: ChainDoc; answered: boolean }> {
	let res: Response;
	try {
		res = await fetchWithTimeout(url, {}, { timeoutMs: NET_TIMEOUTS.ethereumData });
	} catch (cause) {
		// No answer at all: offline, refused, timed out.
		return { read: { kind: 'unread', cause }, answered: false };
	}
	if (res.status === 404) return { read: { kind: 'absent' }, answered: true };
	if (!res.ok) {
		// 5xx, 429, any other refusal: the server said nothing about the chain.
		return {
			read: { kind: 'unread', cause: new Error(`registry document: HTTP ${res.status}`) },
			answered: true
		};
	}
	try {
		const body: unknown = await res.json();
		if (typeof body !== 'object' || body === null || Array.isArray(body)) {
			return {
				read: { kind: 'unread', cause: new Error('registry document: not an object') },
				answered: true
			};
		}
		return { read: { kind: 'doc', body: body as Record<string, unknown> }, answered: true };
	} catch (cause) {
		// A body cut off or not JSON is no answer either.
		return { read: { kind: 'unread', cause }, answered: true };
	}
}

/** The chain's registry document, as one of three outcomes; see the module doc. */
export function readChainDoc(chainId: number, now: () => number = Date.now): Promise<ChainDoc> {
	const url = docUrl(chainId);
	const hit = kept.get(url);
	if (hit && now() - hit.at < DOC_TTL_MS) return Promise.resolve(hit.read);
	const rest = resting.get(url);
	if (rest && now() < rest.until) return Promise.resolve({ kind: 'unread', cause: rest.cause });
	const joined = inFlight.get(url);
	if (joined) return joined;

	const request = fetchDoc(url).then(({ read, answered }) => {
		inFlight.delete(url);
		if (read.kind !== 'unread') kept.set(url, { read, at: now() });
		if (answered) {
			resting.delete(url);
		} else {
			const failures = (resting.get(url)?.failures ?? 0) + 1;
			const quiet = Math.min(QUIET_CAP_MS, QUIET_BASE_MS * 2 ** (failures - 1));
			resting.set(url, {
				failures,
				until: now() + quiet,
				cause: read.kind === 'unread' ? read.cause : null
			});
		}
		return read;
	});
	inFlight.set(url, request);
	return request;
}

/** For tests: forget every document, rest and request. */
export function resetChainDocsForTests(): void {
	kept.clear();
	inFlight.clear();
	resting.clear();
}
