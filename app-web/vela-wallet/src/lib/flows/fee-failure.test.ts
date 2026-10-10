/**
 * PR 2 polish: a failed fee, as every surface reads it — for its own chain
 * only (`FeeFailureView::is_for_chain`), and with a tap that does exactly
 * what its figure says (`FeeFailureView.tap`).
 */
import { describe, expect, it } from 'vitest';
import type { FeeFailureView } from '$lib/core/generated/FeeFailureView';
import {
	FEE_FIGURE_KEYS,
	failedFeeAction,
	failedFeeTappable,
	failureForChain,
	feeRowTap
} from './fee-failure';
import { WALLET_FLOW_KEYS } from './messages';

const FAILURE: FeeFailureView = {
	failure: 'missing_public_key',
	reason_key: null,
	auto_retry: false,
	retrying: false,
	figure_key: 'componentsUi.gas.estimateFailed',
	footer_key: 'componentsUi.signing.confirmBlock.feeFailed',
	tap: 'retry',
	chain_id: 1,
	fee_token: null
};

describe('a failure is for its own chain (is_for_chain)', () => {
	it('drawn for its chain, dropped for another, and for a form that names none', () => {
		expect(failureForChain(FAILURE, 1)).toBe(FAILURE);
		expect(failureForChain(FAILURE, 137)).toBeNull();
		expect(failureForChain(FAILURE, null)).toBeNull();
	});

	it('a failure built without a run (no chain) is every surface’s', () => {
		const untagged = { ...FAILURE, chain_id: null };
		expect(failureForChain(untagged, 137)).toBe(untagged);
		expect(failureForChain(untagged, null)).toBe(untagged);
	});

	it('no failure is no failure', () => {
		expect(failureForChain(null, 1)).toBeNull();
		expect(failureForChain(undefined, 1)).toBeNull();
	});
});

describe('the tap does what the figure says (FeeFailureView.tap)', () => {
	it('retry asks again — nothing while a re-ask is out', () => {
		expect(failedFeeAction(FAILURE, false)).toBe('requote');
		expect(failedFeeAction(FAILURE, true)).toBe('none');
		expect(failedFeeTappable(FAILURE)).toBe(true);
	});

	it('choose_coin opens the coins, busy or not', () => {
		const coins = { ...FAILURE, failure: 'would_fail' as const, tap: 'choose_coin' as const };
		expect(failedFeeAction(coins, false)).toBe('open_coins');
		expect(failedFeeAction(coins, true)).toBe('open_coins');
		expect(failedFeeTappable(coins)).toBe(true);
		expect(feeRowTap({ failure: coins, busy: false }, 2)).toBe('toggle_coins');
	});

	it('nothing is no control', () => {
		const none = { ...FAILURE, failure: 'would_fail' as const, tap: 'nothing' as const };
		expect(failedFeeAction(none, false)).toBe('none');
		expect(failedFeeTappable(none)).toBe(false);
		expect(feeRowTap({ failure: none, busy: false }, 2)).toBe('none');
	});

	it('every figure key the core can name is one the send screens carry', () => {
		for (const key of FEE_FIGURE_KEYS) {
			expect(WALLET_FLOW_KEYS as readonly string[]).toContain(key);
		}
	});
});
