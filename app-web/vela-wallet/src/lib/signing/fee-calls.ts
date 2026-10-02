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
import { dappRequestCalls } from '$lib/core/kernels';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { SignMethodKind } from '$lib/core/generated/SignMethodKind';

/**
 * The calls to price, or `null` when there is nothing on chain.
 *
 * `kind` is the core's classification (`SignMethodKind`), so a message never
 * asks for a quote and a batch is priced as its own list of calls. The calls
 * themselves are the core's ONE reading (`tx_request::calls_of`, spec 096 F1)
 * — the very calls the submit sends. This used to read `value` its own way:
 * `"1000"` as decimal here and as hex at the submit, so the fee shown could
 * price a different amount than the one signed. A call with no recipient (a
 * deployment, which this wallet does not send) or an unreadable amount is
 * nothing to price: a wrong fee beside a real transaction is worse than none.
 */
export function feeCallsOf(kind: SignMethodKind, paramsJson: string): FeeCall[] | null {
	const method =
		kind === 'batch' ? 'wallet_sendCalls' : kind === 'transaction' ? 'eth_sendTransaction' : null;
	if (method === null) return null;
	return dappRequestCalls(method, paramsJson);
}
