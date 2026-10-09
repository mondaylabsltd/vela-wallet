/**
 * Correctness batch item 3: one transaction in flight per account and network.
 *
 * The send and signing machines hold a second confirm while the account's
 * previous operation on that network is in flight. What they are told is the
 * core's reading of the tracker's OWN view (`inFlightOps`) — so this drives the
 * REAL tracker core through the resident, mocking only the network and the
 * store, and pins the wiring end to end: a hand-off that names its signer
 * becomes a held nonce for every listener, it is said once (no flicker), and
 * it lets go when the operation lands. A view re-encoded from a narrower
 * shape would lose `sender` (nothing held) or `stalled` (held forever); the
 * kernel tests below pin both.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { InFlightOp } from '$lib/core/generated/InFlightOp';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';

const net = vi.hoisted(() => ({ landed: null as string | null }));

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/records', () => ({
	loadTransactions: vi.fn(async () => []),
	updateTransactions: vi.fn(async () => {})
}));
vi.mock('$lib/services/tx-reconciler', () => ({
	pollUserOpStatus: vi.fn(async () => null),
	// Accepted and on its way until the test says it landed.
	requestUserOpReceipt: vi.fn(async () =>
		net.landed === null
			? { reachedBundler: true, resolution: null }
			: {
					reachedBundler: true,
					resolution: { status: 'confirmed', txHash: net.landed, logs: [] }
				}
	)
}));
vi.mock('$lib/services/rpc-adapter', () => ({ rpcCall: vi.fn(async () => ({ result: null })) }));
vi.mock('$lib/wallet/core/balance.svelte', () => ({ balance: { refresh: vi.fn() } }));
vi.mock('$lib/wallet/core/feed.svelte', () => ({ feed: { reconciled: vi.fn() } }));
vi.mock('./token-trust-resident', () => ({ notifyReceiptLogsConfirmed: vi.fn() }));

import { inFlightOps } from '$lib/core/kernels';
import {
	dispatchTxTracker,
	outcomeOf,
	subscribeInFlightOps,
	trackSubmitted,
	txTrackerView
} from './tracker-resident';

const SENDER = '0x' + 'aB'.repeat(20);
const OP = '0x' + 'c4'.repeat(32);

afterEach(() => {
	net.landed = null;
});

describe('what the send and signing machines are told is in flight', () => {
	it('a hand-off that names its signer holds that account’s nonce; landing lets it go', async () => {
		const heard: InFlightOp[][] = [];
		const stop = subscribeInFlightOps((ops) => heard.push(ops));
		// Told at once, before anything is in flight.
		expect(heard).toEqual([[]]);

		trackSubmitted(OP, ['send-1'], 100, undefined, false, 9, false, SENDER);
		await vi.waitFor(() => expect(heard.at(-1)).toHaveLength(1));
		expect(heard.at(-1)).toEqual([
			{ sender: SENDER.toLowerCase(), chain_id: 100, user_op_hash: OP }
		]);
		// Every later render with the same list is not repeated: nothing flickers.
		const told = heard.length;
		dispatchTxTracker({ type: 'home_focused' });
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(heard.length).toBe(told);

		// It lands: final — the hold lets go.
		net.landed = '0x' + 'dd'.repeat(32);
		await vi.waitFor(
			() => {
				dispatchTxTracker({ type: 'tick' });
				expect(heard.at(-1)).toEqual([]);
			},
			{ timeout: 8000, interval: 500 }
		);
		stop();
	}, 15_000);

	it('a listener that leaves hears nothing more', async () => {
		const heard: InFlightOp[][] = [];
		const stop = subscribeInFlightOps((ops) => heard.push(ops));
		stop();
		const before = heard.length;
		trackSubmitted('0x' + 'e5'.repeat(32), ['send-2'], 1, undefined, false, 9, false, SENDER);
		await vi.waitFor(() =>
			expect(txTrackerView().entries.some((e) => e.user_op_hash === '0x' + 'e5'.repeat(32))).toBe(
				true
			)
		);
		expect(heard.length).toBe(before);
	});
});

function entry(over: Partial<TrackEntryView>): TrackEntryView {
	return {
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
		sender: SENDER.toLowerCase(),
		...over
	};
}

describe('the core’s reading of the tracker view (`inFlightOps`)', () => {
	it('an accepted op still followed holds its account’s nonce', () => {
		expect(inFlightOps({ entries: [entry({})] })).toEqual([
			{ sender: SENDER.toLowerCase(), chain_id: 100, user_op_hash: OP }
		]);
	});

	it('ten minutes without progress (`stalled`) lets the next one go', () => {
		expect(inFlightOps({ entries: [entry({ stalled: true })] })).toEqual([]);
	});

	it('an entry with no signer holds nothing — why the view must be the core’s own', () => {
		expect(inFlightOps({ entries: [entry({ sender: null })] })).toEqual([]);
	});

	it('a final op holds nothing', () => {
		expect(
			inFlightOps({
				entries: [entry({ status: 'confirmed', polling: false, outcome: 'final', tx_hash: '0x1' })]
			})
		).toEqual([]);
	});
});

describe('a refusal reaches the send receipt with its reason', () => {
	it('the tracker’s `refusal` rides on the failed outcome', () => {
		expect(outcomeOf(entry({ status: 'rejected', refusal: 'nonce_used' }))).toEqual({
			type: 'failed',
			rejected: true,
			not_sent: false,
			refusal: 'nonce_used'
		});
		// An older relay said none: the plain refusal.
		expect(outcomeOf(entry({ status: 'rejected' }))).toEqual({
			type: 'failed',
			rejected: true,
			not_sent: false
		});
	});
});
