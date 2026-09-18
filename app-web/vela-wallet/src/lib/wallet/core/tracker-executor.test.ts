/**
 * The web tx_tracker executor (issue 188): a confirmation asks the balance core
 * for a forced refresh — before, and independently of, the auto-add, which
 * only fires when the receipt carried logs.
 */
import { describe, expect, it, vi } from 'vitest';
import type { TrackEffect } from './tracker-types';

vi.mock('$lib/services/records', () => ({
	loadTransactions: vi.fn(async () => []),
	updateTransactions: vi.fn(async () => {})
}));
vi.mock('$lib/services/tx-reconciler', () => ({
	pollUserOpStatus: vi.fn(async () => null),
	requestUserOpReceipt: vi.fn(async () => ({ reachedBundler: false, resolution: null }))
}));

import { createTxTrackerExecutor } from './tracker-executor';

describe('notify_confirmed', () => {
	it('asks for a balance refresh on every confirmation; auto-add only with logs', async () => {
		const ports = { feedReconciled: vi.fn(), receiptLogsConfirmed: vi.fn(), confirmed: vi.fn() };
		const executor = createTxTrackerExecutor(ports);
		const effect: TrackEffect = {
			id: 1,
			operation: {
				type: 'notify_confirmed',
				user_op_hash: '0xABC',
				chain_id: 8453,
				tx_hash: '0xdef'
			}
		};
		await expect(executor.execute(effect)).resolves.toEqual({ type: 'notified' });
		expect(ports.confirmed).toHaveBeenCalledTimes(1);
		expect(ports.confirmed).toHaveBeenCalledWith(8453);
		// No receipt was polled for this hash, so there are no logs to admit
		// tokens from — and no sender is guessed.
		expect(ports.receiptLogsConfirmed).not.toHaveBeenCalled();
	});
});
