/**
 * The fee port's answer, in the send core's vocabulary (`toSendFeeOutcome`).
 * Every fee failure must arrive as a word the send core can read: one it
 * cannot would fail the whole event, and the form would hang on its estimate.
 * Since PR 2 note 13 the two vocabularies are one, so it is passed through.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { sendEstimateFailureBodyKey } from '$lib/core/kernels';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { SendEstimateFailure } from '$lib/core/generated/SendEstimateFailure';
import { toSendFeeOutcome } from './send-types';

describe('toSendFeeOutcome', () => {
	it('passes every fee failure through as it is (PR 2 note 13)', () => {
		const cases: FeeFailure[] = [
			{ chain_read: { rate_limited: false } },
			{ chain_read: { rate_limited: true } },
			// Issue 483: the account read never left the app — Vela's own fault.
			'internal',
			'would_fail',
			'missing_public_key',
			'fee_token_unavailable',
			'quote_unavailable',
			'calculation_failed',
			'estimate_failed',
			'gas_quote_too_high'
		];
		for (const fee of cases) {
			const kind: SendEstimateFailure = fee;
			expect(toSendFeeOutcome({ type: 'failed', kind: fee }), JSON.stringify(fee)).toEqual({
				type: 'failed',
				kind
			});
		}
	});

	it('the alert is worded by the cause, by the core (`sendEstimateFailureBodyKey`)', () => {
		expect(sendEstimateFailureBodyKey({ chain_read: { rate_limited: false } })).toBe(
			'send.alertEstimateChainDownBody'
		);
		expect(sendEstimateFailureBodyKey('internal')).toBe('componentsUi.gas.reasonInternal');
		for (const other of ['quote_unavailable', 'would_fail', 'timeout', 'other'] as const) {
			expect(sendEstimateFailureBodyKey(other), other).toBe('send.alertEstimateFailedBody');
		}
	});
});
