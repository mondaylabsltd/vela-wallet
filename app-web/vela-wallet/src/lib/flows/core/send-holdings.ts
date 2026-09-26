/**
 * `FetchTokens`, answered from the asset list's holdings (spec 078) — and the
 * one failure mode that source has.
 *
 * Send shows the SAME holdings the balance machine does: the list already in
 * memory, not a second walk over every chain. What that list can say:
 *
 * - **tokens** — this account has settled a round, and the round reached
 *   something (or honestly found nothing held: an empty list with every chain
 *   answering is an empty wallet, not a failure);
 * - **walk** — no round has settled for this account yet, or the machine is
 *   reading another account: the executor walks the chains as it always has;
 * - **nothing** — the round reached NOTHING: no tokens and chains that did not
 *   answer, or nothing could be read at all and nothing is known.
 *
 * "Nothing" is not answered at once (the desktop's retry-once rule, now all
 * four shells'): a proxy blip at launch left the picker on "could not load
 * tokens" until the next ten-minute poll. The machine is asked for ONE forced
 * re-read and the NEXT round answers, whatever it holds — the Assets list
 * recovers with it. Only a second round that reaches nothing is the failure,
 * and the core words it (`tokens: null` → "could not load tokens").
 */
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { SendToken } from '$lib/core/generated/SendToken';

/** What the asset list says for one `FetchTokens`. */
export type SendHoldingsAnswer =
	| { kind: 'tokens'; tokens: SendToken[] }
	/** Nothing settled for this account: walk the chains as before. */
	| { kind: 'walk' }
	/** Two rounds reached nothing: `tokens_loaded { tokens: null }`. */
	| { kind: 'unreadable' };

/** The balance machine, as far as Send needs it. */
export interface HoldingsSource {
	/** The machine's view as it stands. */
	view(): BalanceView;
	/** `address` is the active account and has settled at least one round. */
	settled(address: string): boolean;
	/**
	 * Ask for ONE forced re-read, and resolve once the next round for this
	 * account has ended — settled or errored — or the account has changed.
	 */
	reread(address: string): Promise<void>;
	/** A balance row in Send's wire shape — exactly as a walk's answer maps. */
	toSend(token: BalanceToken): SendToken;
}

/** One reading of the machine, without asking it anything. */
export function readHoldings(
	address: string,
	source: Pick<HoldingsSource, 'view' | 'settled' | 'toSend'>
): { kind: 'tokens'; tokens: SendToken[] } | { kind: 'walk' } | { kind: 'nothing' } {
	const view = source.view();
	if (view.address === null || view.address.toLowerCase() !== address.toLowerCase()) {
		return { kind: 'walk' };
	}
	if (source.settled(address)) {
		// Chains that did not answer and not one token: that round saw nothing,
		// which is not the same fact as an empty wallet.
		if (view.tokens.length === 0 && view.failed_chain_ids.length > 0) return { kind: 'nothing' };
		return { kind: 'tokens', tokens: view.tokens.map((token) => source.toSend(token)) };
	}
	// The first fetch threw with no cache to fall back on.
	if (view.unreachable) return { kind: 'nothing' };
	// The first round is still out.
	return { kind: 'walk' };
}

/** The answer to `FetchTokens`, retrying a round that reached nothing once. */
export async function answerHoldings(
	address: string,
	source: HoldingsSource
): Promise<SendHoldingsAnswer> {
	const first = readHoldings(address, source);
	if (first.kind !== 'nothing') return first;
	await source.reread(address);
	const next = readHoldings(address, source);
	return next.kind === 'nothing' ? { kind: 'unreadable' } : next;
}
