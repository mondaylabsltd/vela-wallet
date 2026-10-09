/**
 * PR 2 note 12, through the resident: the extension's request window follows
 * the tracker the wallet runs — it sweeps nothing, polls nothing, and still
 * holds a second confirm while the account's operation is in flight (the
 * wallet's view, read by the core's own `inFlightOps`), and its hand-offs go
 * to the wallet's tracker. The share protocol has its own suite
 * (`tracker-share.test.ts`); this pins the resident's wiring of it.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';
import type { InFlightOp } from '$lib/core/generated/InFlightOp';
import type { TrackEvent } from '$lib/core/generated/TrackEvent';
import type { TrackView } from '$lib/core/generated/TrackView';

const seams = vi.hoisted(() => ({
	loadTransactions: vi.fn(async () => []),
	receipt: vi.fn(async () => ({ reachedBundler: true, resolution: null })),
	status: vi.fn(async () => null)
}));

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/records', () => ({
	loadTransactions: seams.loadTransactions,
	updateTransactions: vi.fn(async () => {})
}));
vi.mock('$lib/services/tx-reconciler', () => ({
	pollUserOpStatus: seams.status,
	requestUserOpReceipt: seams.receipt
}));
vi.mock('$lib/services/rpc-adapter', () => ({ rpcCall: vi.fn(async () => ({ result: null })) }));
vi.mock('$lib/wallet/core/balance.svelte', () => ({ balance: { refresh: vi.fn() } }));
vi.mock('$lib/wallet/core/feed.svelte', () => ({ feed: { reconciled: vi.fn() } }));
vi.mock('./token-trust-resident', () => ({ notifyReceiptLogsConfirmed: vi.fn() }));

import {
	followSharedTracker,
	subscribeInFlightOps,
	trackSubmitted,
	txTrackerView
} from './tracker-resident';
import {
	startVoice,
	TRACKER_LOCK,
	type ShareChannel,
	type ShareDeps,
	type ShareLocks,
	type ShareMessage
} from './tracker-share';

/** One channel per document, delivering to the others. */
const channels = new Set<ShareChannel>();
function open(): ShareChannel {
	const channel: ShareChannel = {
		onmessage: null,
		postMessage(message: ShareMessage) {
			const copy = structuredClone(message);
			for (const other of channels) {
				if (other !== channel) queueMicrotask(() => other.onmessage?.({ data: copy }));
			}
		},
		close() {
			channels.delete(channel);
		}
	};
	channels.add(channel);
	return channel;
}
/** The wallet holds the shared lock for its life; nothing else is asked here. */
let sharedHeld = 0;
const locks: ShareLocks = {
	request: (_name, { mode }, callback) => {
		if (mode === 'shared') {
			sharedHeld += 1;
			return callback();
		}
		// The window's takeover waits for a wallet that never goes, in this test.
		return new Promise(() => {});
	},
	query: async () => ({
		held: Array.from({ length: sharedHeld }, () => ({ name: TRACKER_LOCK, mode: 'shared' }))
	})
};
const deps = (id: string): ShareDeps => ({ channel: open, locks: () => locks, id: () => id });

const SENDER = ('0x' + 'aB'.repeat(20)).toLowerCase();
const OP = '0x' + 'c4'.repeat(32);
const WALLET_VIEW = {
	entries: [
		{
			user_op_hash: OP,
			chain_id: 100,
			record_ids: ['send-1'],
			status: 'pending',
			tx_hash: null,
			polling: true,
			submitted_at_ms: 1_000,
			outcome: 'landing',
			relay_tx_hash: null,
			relay_sent_at_ms: null,
			sender: SENDER
		}
	]
} as TrackView;

describe('the request window follows the wallet’s tracker (PR 2 note 12)', () => {
	it('sweeps nothing, polls nothing, holds by the wallet’s view, hands its ops over', async () => {
		// The side panel is up, running the one tracker.
		const taken: TrackEvent[] = [];
		startVoice(deps('panel'), { current: () => WALLET_VIEW, dispatch: (e) => taken.push(e) });

		// The request window opens.
		followSharedTracker(deps('window'));
		const heard: InFlightOp[][] = [];
		subscribeInFlightOps((ops) => heard.push(ops));

		// The panel's operation holds this window's confirm too.
		await vi.waitFor(() =>
			expect(heard.at(-1)).toEqual([{ sender: SENDER, chain_id: 100, user_op_hash: OP }])
		);
		expect(txTrackerView()).toEqual(WALLET_VIEW);

		// This window's own approval: handed to the panel's tracker.
		const mine = '0x' + 'e5'.repeat(32);
		trackSubmitted(mine, ['dapp-1-tx'], 100, undefined, false, 9, true, SENDER);
		await vi.waitFor(() => expect(taken.map((e) => e.type)).toEqual(['submitted']));
		expect(taken[0]).toMatchObject({ user_op_hash: mine, admitted: true, sender: SENDER });

		// No second sweep of the stored records, no second poll of any op.
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(seams.loadTransactions).not.toHaveBeenCalled();
		expect(seams.receipt).not.toHaveBeenCalled();
		expect(seams.status).not.toHaveBeenCalled();
	});
});
