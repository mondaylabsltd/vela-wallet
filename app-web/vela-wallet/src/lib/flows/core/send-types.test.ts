/**
 * The fee port's answer, in the send core's vocabulary (`toSendFeeOutcome`).
 * Every fee failure must arrive as a word the send core can read: one it
 * cannot would fail the whole event, and the form would hang on its estimate.
 */
import { describe, expect, it } from 'vitest';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { SendEstimateFailure } from '$lib/core/generated/SendEstimateFailure';
import { toSendFeeOutcome } from './send-types';

describe('toSendFeeOutcome', () => {
	it('says every fee failure in a word the send core knows', () => {
		const cases: [FeeFailure, SendEstimateFailure][] = [
			[{ chain_read: { rate_limited: false } }, 'quote_unavailable'],
			[{ chain_read: { rate_limited: true } }, 'quote_unavailable'],
			// Issue 483: the account read never left the app — Vela's own fault.
			['internal', 'other'],
			['would_fail', 'estimate_failed'],
			['missing_public_key', 'missing_public_key'],
			['fee_token_unavailable', 'fee_token_unavailable'],
			['quote_unavailable', 'quote_unavailable'],
			['calculation_failed', 'calculation_failed'],
			['estimate_failed', 'estimate_failed'],
			['gas_quote_too_high', 'gas_quote_too_high']
		];
		for (const [fee, send] of cases) {
			expect(toSendFeeOutcome({ type: 'failed', kind: fee }), JSON.stringify(fee)).toEqual({
				type: 'failed',
				kind: send
			});
		}
	});
});
