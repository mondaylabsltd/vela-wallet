/**
 * The executor's fee, priced the way every other send of the wallet is: one
 * `fee_policy` quote of the executor's real operation (its calls, the fee leg
 * the core appends, the relay's own simulation), at the relay's published
 * per-gas price on the gas it bills. The amount quoted is the amount shown
 * and the amount signed (`quotedFee`) — never a second, send-time price.
 *
 * It used to be that second price: the executor signed with no quote, and the
 * in-band path's programmatic fallback charged 3 × the operation's padded gas
 * LIMITS × the fast tier's price — the pre-CONTRACT-v2 rule the core retired
 * for sends (`fee_policy::IN_BAND_PRICE`: "not 3 × the operation's padded
 * LIMITS on top of a relay markup"), which is why an execution cost many
 * times its bundle on chain.
 */
import { DEFAULT_TIER, FeeQuote } from '$lib/flows/core/fee-quote.svelte';
import { findAccountByAddress } from '$lib/services/accounts';
import { estimateRevertsFor } from '$lib/services/estimate-verdict';
import type { QuotedInBandFee } from '$lib/services/safe-transaction';
import type { Call } from './kernel';

export type ExecutionQuote =
	| {
			kind: 'ok';
			fee: Required<Pick<QuotedInBandFee, 'amount' | 'recipient'>> & Pick<QuotedInBandFee, 'tier'>;
			/** The gas the relay bills the operation against (the fee's basis). */
			totalGas: bigint;
			/** Whether the executor's own account exists on the chain already. */
			executorDeployed: boolean;
	  }
	/** The relay simulated it and it fails; its reason, as the core cleaned it. */
	| { kind: 'reverts'; reason: string | null }
	/** No price: the relay or the chain did not answer. */
	| { kind: 'failed' };

/** `0x…` or decimal → decimal, as `FeeCall.value` travels. */
function decimal(value: string): string {
	try {
		return BigInt(value || '0').toString();
	} catch {
		return '0';
	}
}

/** Price the executor's calls on `chainId`, paid in the chain's coin. */
export async function quoteExecutionFee(
	chainId: number,
	executor: string,
	calls: Call[]
): Promise<ExecutionQuote> {
	const stored = findAccountByAddress(executor);
	const quote = new FeeQuote();
	try {
		const outcome = await quote.requestQuote({
			chainId,
			account: executor,
			calls: calls.map((c) => ({ to: c.to, value: decimal(c.value), data: c.data || '0x' })),
			feeToken: null,
			publicKeyHex: stored?.publicKeyHex,
			tier: DEFAULT_TIER
		});
		if (outcome.kind === 'ok') {
			const view = outcome.estimate;
			const amount = BigInt(
				view.fee_asset.type === 'erc20' ? view.fee_asset.amount : view.total_wei
			);
			if (view.fee_recipient && amount > 0n) {
				return {
					kind: 'ok',
					fee: {
						amount,
						recipient: view.fee_recipient,
						...(view.tier === 'rapid' ? {} : { tier: view.tier })
					},
					totalGas: BigInt(view.total_gas),
					executorDeployed: view.deployed
				};
			}
		}
		// The relay's own simulation said it reverts: an answer, not an outage.
		const reverts = estimateRevertsFor(chainId, executor);
		if (reverts) return { kind: 'reverts', reason: reverts.reason };
		return { kind: 'failed' };
	} catch {
		return { kind: 'failed' };
	} finally {
		quote.dispose();
	}
}
