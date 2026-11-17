/**
 * The batches Vela Connect answered a site with (spec 109) — so its
 * `wallet_getCallsStatus` can be answered: a batch id is the operation's hash,
 * and only the wallet knows which chain it went to.
 *
 * Kept for a day per batch, under the site that sent it — a site asks about
 * its own batches, never another's (5730, as for an id nobody knows). The
 * extension's worker keeps the same (`vela.op.<hash>`, RF3); this is the web's.
 */

export const CONNECT_OPS_KEY = 'vela.connect.ops';
export const CONNECT_OP_TTL_MS = 24 * 60 * 60_000;
/** Enough for anyone's day; the oldest goes first. */
const MAX_OPS = 200;

export interface ConnectOp {
	chainId: number;
	origin: string;
	at: number;
}

type OpTable = Record<string, ConnectOp>;

function storage(): Storage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

function read(now: number): OpTable {
	let raw: unknown;
	try {
		raw = JSON.parse(storage()?.getItem(CONNECT_OPS_KEY) ?? '{}');
	} catch {
		return {};
	}
	if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};
	const table: OpTable = {};
	for (const [id, value] of Object.entries(raw as Record<string, unknown>)) {
		const op = value as Partial<ConnectOp> | null;
		if (
			op &&
			typeof op.chainId === 'number' &&
			op.chainId > 0 &&
			typeof op.origin === 'string' &&
			typeof op.at === 'number' &&
			now - op.at < CONNECT_OP_TTL_MS
		) {
			table[id] = { chainId: op.chainId, origin: op.origin, at: op.at };
		}
	}
	return table;
}

/** Remember batch `id`, sent for `origin` on `chainId`. */
export function rememberOp(id: string, chainId: number, origin: string, now = Date.now()): void {
	const store = storage();
	if (!store || !id || !(chainId > 0)) return;
	const table = read(now);
	table[id.toLowerCase()] = { chainId, origin, at: now };
	const kept = Object.entries(table)
		.sort(([, a], [, b]) => b.at - a.at)
		.slice(0, MAX_OPS);
	try {
		store.setItem(CONNECT_OPS_KEY, JSON.stringify(Object.fromEntries(kept)));
	} catch {
		/* not remembered: the site's status reads say it is unknown */
	}
}

/** Batch `id` as `origin` sent it, or `null` (unknown, someone else's, or too old). */
export function findOp(id: string, origin: string, now = Date.now()): ConnectOp | null {
	const op = read(now)[id.toLowerCase()];
	return op && op.origin === origin ? op : null;
}

/**
 * The batch id an answer to `wallet_sendCalls` names — a bare id (EIP-5792
 * 1.0) or `{ id }` (2.0.0) — or `null`.
 */
export function batchIdOf(result: unknown): string | null {
	if (typeof result === 'string') return result;
	if (result && typeof result === 'object' && typeof (result as { id?: unknown }).id === 'string') {
		return (result as { id: string }).id;
	}
	return null;
}
