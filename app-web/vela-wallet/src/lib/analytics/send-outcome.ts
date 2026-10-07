/**
 * How a send ended, for usage statistics — read from the transaction
 * tracker, which keeps following an operation after the send screen closes.
 *
 * Only the operations THIS document handed over from the send flow are
 * watched (`watchSendOutcome`, called from the send's tracker sink): a dApp's
 * transaction, or one a reload's recovery sweep picks up, is never counted as
 * a send, and a reload does not count one twice. The hash stays in memory;
 * only the chain id and the verdict's kind are sent.
 */
import { subscribeTxTracker } from '$lib/wallet/core/tracker-resident';
import type { TrackStatus } from '$lib/core/generated/TrackStatus';
import type { AnalyticsReason } from './catalog';
import { track } from './index';

const FAILED: Partial<Record<TrackStatus, AnalyticsReason>> = {
	dropped: 'dropped',
	rejected: 'rejected',
	not_sent: 'not_sent'
};

const watched = new Map<string, number>();
let stop: (() => void) | null = null;

export function watchSendOutcome(userOpHash: string, chainId: number): void {
	if (!userOpHash) return;
	watched.set(userOpHash.toLowerCase(), chainId);
	stop ??= subscribeTxTracker((view) => {
		for (const entry of view.entries) {
			const key = entry.user_op_hash.toLowerCase();
			const chain = watched.get(key);
			if (chain === undefined) continue;
			if (entry.status === 'confirmed') {
				watched.delete(key);
				track('send_confirmed', { chain });
			} else if (FAILED[entry.status] !== undefined) {
				watched.delete(key);
				track('send_failed', { chain, reason: FAILED[entry.status] });
			}
		}
		if (watched.size === 0) {
			stop?.();
			stop = null;
		}
	});
}
