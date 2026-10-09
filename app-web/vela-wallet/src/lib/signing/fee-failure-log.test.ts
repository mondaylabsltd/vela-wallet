/**
 * The signing sheet's fee diagnosis (spec 082 G61), now that the retry is the
 * core's (PR 2 note 1): a failed run is said once — however many views repeat
 * it — with whether the core asks again, and a fee that lands after failures
 * says how many runs it took. The schedule itself is pinned against the real
 * core in `flows/core/fee-retry.test.ts`.
 */
import { describe, expect, it } from 'vitest';
import type { FeeFailure } from '$lib/core/generated/FeeFailure';
import type { FeeFailureView } from '$lib/core/generated/FeeFailureView';
import { feeFailureCause, FeeFailureLog, type FeeFailureInput } from './fee-failure-log';

function harness() {
	const lines: string[] = [];
	const counted: string[] = [];
	const log = new FeeFailureLog({
		log: (line) => lines.push(line),
		count: (cause) => counted.push(cause)
	});
	return { log, lines, counted };
}

const view = (failure: FeeFailure, auto_retry: boolean, retrying = false): FeeFailureView => ({
	failure,
	reason_key: null,
	auto_retry,
	retrying,
	figure_key: null,
	footer_key: 'componentsUi.signing.confirmBlock.feeRetrying'
});
const failed = (failure: FeeFailure, auto = true): FeeFailureInput => ({
	busy: false,
	failed: failure,
	failure: view(failure, auto),
	fee: null
});
const retrying = (failure: FeeFailure): FeeFailureInput => ({
	busy: true,
	failed: null,
	failure: view(failure, true, true),
	fee: null
});
const QUOTED = { busy: false, failed: null, failure: null, fee: {} } as unknown as FeeFailureInput;

describe('FeeFailureLog', () => {
	it('says a failed run once, however many views repeat it', () => {
		const h = harness();
		h.log.observe(failed('quote_unavailable'), 100);
		h.log.observe(failed('quote_unavailable'), 100);
		expect(h.lines).toEqual([
			'fee: quote failed chain=100 cause=quote_unavailable run #1, the core asks again'
		]);
		expect(h.counted).toEqual(['quote_unavailable']);
	});

	it('counts each run the core asks again, and the recovery after them', () => {
		const h = harness();
		const down = { chain_read: { rate_limited: true } } as const;
		h.log.observe(failed(down), 1);
		h.log.observe(retrying(down), 1);
		h.log.observe(failed(down), 1);
		h.log.observe(retrying(down), 1);
		h.log.observe(QUOTED, 1);
		expect(h.lines).toEqual([
			'fee: quote failed chain=1 cause=chain_read_rate_limited run #1, the core asks again',
			'fee: quote failed chain=1 cause=chain_read_rate_limited run #2, the core asks again',
			'fee: quote back chain=1 after 2 failed runs'
		]);
		expect(h.counted).toEqual(['chain_read_rate_limited', 'chain_read_rate_limited']);
	});

	it('says when nothing will ask again but a tap', () => {
		const h = harness();
		h.log.observe(failed('missing_public_key', false), 8453);
		expect(h.lines).toEqual([
			'fee: quote failed chain=8453 cause=missing_public_key run #1, no re-quote'
		]);
	});

	it('another request (or none) starts the count again', () => {
		const h = harness();
		h.log.observe(failed('quote_unavailable'), 100);
		h.log.observe(null, null);
		h.log.observe(QUOTED, 100);
		expect(h.lines).toHaveLength(1);
		h.log.observe(failed('internal'), null);
		expect(h.lines[1]).toBe(
			'fee: quote failed chain=unknown cause=internal run #1, the core asks again'
		);
	});

	it('names a failure by its wire word, never a host or a value', () => {
		expect(feeFailureCause('estimate_failed')).toBe('estimate_failed');
		expect(feeFailureCause({ chain_read: { rate_limited: false } })).toBe('chain_read');
		expect(feeFailureCause({ chain_read: { rate_limited: true } })).toBe('chain_read_rate_limited');
	});
});
