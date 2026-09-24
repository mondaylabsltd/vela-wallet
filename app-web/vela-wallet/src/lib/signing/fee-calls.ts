/**
 * The REAL calls of a signing request, as the fee machine prices them.
 *
 * Lifted out of `SigningHost.svelte` (B-3/B-4, 2026-09-23). It lived inside the
 * component, wrapped in one `try`, and what it returned decided TWO things a
 * person can see:
 *
 * - whether the sheet asks for a fee quote at all, and
 * - whether the speed control is enabled (the host's predicate is
 *   `quotedFor !== ''`, and `quotedFor` is only set once this returns calls).
 *
 * So `null` meant "no network fee AND no speed switch", silently, and there was
 * nowhere to test it from. The owner met exactly that on a Uniswap swap.
 *
 * `null` now means one thing only: **there is nothing on chain to price** — a
 * message, a request with no calls, a call with no recipient. Anything that is
 * a transaction and has a recipient comes back as calls.
 */
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { SignMethodKind } from '$lib/core/generated/SignMethodKind';

/**
 * A JSON-RPC quantity as wei.
 *
 * `undefined`, `""` and **`"0x"`** are all zero. That last one is why this
 * function exists: `BigInt("0x")` throws `SyntaxError`, and `"0x"` is what
 * several dApp libraries send for "no ether with this call". The throw took the
 * whole request's fee and speed control with it.
 *
 * `null` for anything that is genuinely not a number, so a caller can tell
 * "zero" from "unreadable" instead of pricing garbage as free.
 */
export function weiOf(value: unknown): string | null {
	if (value === undefined || value === null) return '0';
	if (typeof value === 'number') {
		return Number.isSafeInteger(value) && value >= 0 ? String(value) : null;
	}
	if (typeof value === 'bigint') return value >= 0n ? value.toString() : null;
	if (typeof value !== 'string') return null;
	const text = value.trim();
	if (text === '' || text === '0x' || text === '0X') return '0';
	try {
		const parsed = BigInt(text);
		return parsed >= 0n ? parsed.toString() : null;
	} catch {
		return null;
	}
}

/** One call as a requester wrote it, before anything is trusted about it. */
type RawCall = { to?: unknown; value?: unknown; data?: unknown };

/**
 * The calls to price, or `null` when there is nothing on chain.
 *
 * `kind` is the core's classification (`SignMethodKind`), so a message never
 * asks for a quote and a batch is priced as its own list of calls.
 */
export function feeCallsOf(kind: SignMethodKind, paramsJson: string): FeeCall[] | null {
	let params: unknown;
	try {
		params = JSON.parse(paramsJson);
	} catch {
		return null;
	}
	if (!Array.isArray(params)) return null;
	const first = params[0] as RawCall & { calls?: unknown };
	const raw: RawCall[] =
		kind === 'batch'
			? Array.isArray(first?.calls)
				? (first.calls as RawCall[])
				: []
			: kind === 'transaction'
				? first === undefined || first === null
					? []
					: [first]
				: [];

	const calls: FeeCall[] = [];
	for (const call of raw) {
		const to = typeof call?.to === 'string' ? call.to : '';
		// No recipient is a contract DEPLOYMENT, which this wallet does not
		// send — and pricing it as a call to nowhere would be a made-up number.
		if (to === '') return null;
		const value = weiOf(call?.value);
		// An unreadable amount must not be quietly priced as zero: the fee
		// depends on it, and a wrong fee shown beside a real transaction is
		// worse than no fee shown.
		if (value === null) return null;
		calls.push({ to, value, data: typeof call?.data === 'string' ? call.data : '0x' });
	}
	return calls.length > 0 ? calls : null;
}
