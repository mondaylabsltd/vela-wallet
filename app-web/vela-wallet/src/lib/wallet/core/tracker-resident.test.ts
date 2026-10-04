/**
 * The tracker's clock (spec 079, F13): it used to stop at the end of the 120 s
 * wait window, so an operation past it was asked about only on the next focus
 * or resume — the landing said "submitted" forever and never saw it land. The
 * clock now runs while the core still follows ANY op; how often each is asked
 * about past the window is the core's (`tx_tracker::receipt_interval_ms`).
 */
import { describe, expect, it } from 'vitest';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import { trackerNeedsClock } from './tracker-resident';

function entry(over: Partial<TrackEntryView>): TrackEntryView {
	return {
		user_op_hash: '0x' + 'a1'.repeat(32),
		chain_id: 42161,
		record_ids: ['dapp-1-tx'],
		status: 'pending',
		tx_hash: null,
		polling: true,
		submitted_at_ms: 1_000,
		outcome: 'landing',
		relay_tx_hash: null,
		relay_sent_at_ms: null,
		...over
	};
}

describe('the tracker clock', () => {
	it('ticks inside the wait window', () => {
		expect(trackerNeedsClock({ entries: [entry({})] })).toBe(true);
	});

	it('keeps ticking past the window, whatever the op is waiting on', () => {
		for (const status of ['pending', 'fee_held', 'unreachable', 'accepted_not_landed'] as const) {
			expect(
				trackerNeedsClock({ entries: [entry({ status, outcome: 'still_confirming' })] }),
				status
			).toBe(true);
		}
	});

	it('stops when nothing is followed any more: landed, failed, or abandoned at 24 h', () => {
		expect(
			trackerNeedsClock({
				entries: [
					entry({ status: 'confirmed', polling: false, outcome: 'final', tx_hash: '0xbb' }),
					entry({ status: 'dropped', polling: false, outcome: 'final' }),
					entry({ status: 'accepted_not_landed', polling: false, outcome: 'unknown' })
				]
			})
		).toBe(false);
		expect(trackerNeedsClock({ entries: [] })).toBe(false);
	});

	it('one op still followed among finished ones is enough', () => {
		expect(
			trackerNeedsClock({
				entries: [
					entry({ status: 'confirmed', polling: false, outcome: 'final', tx_hash: '0xbb' }),
					entry({ status: 'accepted_not_landed', outcome: 'still_confirming' })
				]
			})
		).toBe(true);
	});
});
