/**
 * The shared receipt throttle (`requestUserOpReceipt`) — issue 464.
 *
 * Every surface watching a UserOp joins one in-flight request and one 3 s
 * cooldown. The cooldown used to count from the ANSWER: an answer 0.3 s
 * after its tick left the tracker's next 3 s tick 2.7 s out, so the cache
 * answered it and the relay was asked every 6 s. It counts from the ASK now,
 * with the core's tick slack, so a 3 s tick really asks every 3 s.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const relay = vi.hoisted(() => ({
	asks: [] as { resolve: (answer: { result?: unknown; error?: unknown }) => void }[]
}));

vi.mock('./rpc-adapter', () => ({
	USER_OP_STATUS_METHOD: 'pimlico_getUserOperationStatus',
	rpcCall: vi.fn(
		() =>
			new Promise<{ result?: unknown; error?: unknown }>((resolve) => relay.asks.push({ resolve }))
	)
}));

import {
	_resetUserOpReceiptPollCache,
	requestUserOpReceipt,
	USER_OP_RECEIPT_POLL_INTERVAL_MS,
	USER_OP_RECEIPT_TICK_SLACK_MS
} from './tx-reconciler';

const HASH = '0x' + 'ab'.repeat(32);
const CHAIN = 43114;
const T0 = 1_800_000_000_000;

/** Answer the latest ask with "not landed yet", `after` ms after it was made. */
async function answerPending(after: number) {
	vi.setSystemTime(Date.now() + after);
	relay.asks.at(-1)!.resolve({ result: null });
	await vi.runAllTimersAsync();
}

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(T0);
	relay.asks.length = 0;
	_resetUserOpReceiptPollCache();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('the receipt cooldown counts from the ask (issue 464)', () => {
	it('a 3 s tick asks again even when the last answer came back late in the gap', async () => {
		const first = requestUserOpReceipt(HASH, CHAIN);
		expect(relay.asks).toHaveLength(1);
		await answerPending(300);
		expect(await first).toEqual({ resolution: null, reachedBundler: true });

		// The next tick: 3 s after the ASK, 2.7 s after the answer.
		vi.setSystemTime(T0 + USER_OP_RECEIPT_POLL_INTERVAL_MS);
		void requestUserOpReceipt(HASH, CHAIN);
		expect(relay.asks).toHaveLength(2);
	});

	it('a tick that reads a little early (timer jitter) still reaches the relay', async () => {
		void requestUserOpReceipt(HASH, CHAIN);
		await answerPending(400);

		vi.setSystemTime(T0 + USER_OP_RECEIPT_POLL_INTERVAL_MS - 1);
		void requestUserOpReceipt(HASH, CHAIN);
		expect(relay.asks).toHaveLength(2);
	});

	it('inside the cooldown another surface gets the cached answer, with no request', async () => {
		void requestUserOpReceipt(HASH, CHAIN);
		await answerPending(200);

		vi.setSystemTime(T0 + USER_OP_RECEIPT_POLL_INTERVAL_MS - USER_OP_RECEIPT_TICK_SLACK_MS - 1);
		expect(await requestUserOpReceipt(HASH, CHAIN)).toEqual({
			resolution: null,
			reachedBundler: true
		});
		expect(relay.asks).toHaveLength(1);
	});

	it('surfaces asking while a request is out share it — one request per op at a time', async () => {
		const a = requestUserOpReceipt(HASH, CHAIN);
		// Slower than the cooldown: still one request, never a second beside it.
		vi.setSystemTime(T0 + 2 * USER_OP_RECEIPT_POLL_INTERVAL_MS);
		const b = requestUserOpReceipt(HASH, CHAIN);
		expect(relay.asks).toHaveLength(1);
		relay.asks[0].resolve({
			result: { success: true, receipt: { transactionHash: '0x' + 'cd'.repeat(32) } }
		});
		expect(await a).toEqual(await b);
		expect((await a).resolution?.confirmed).toBe(true);
	});

	it('a definitive receipt is kept: nobody asks for it again', async () => {
		void requestUserOpReceipt(HASH, CHAIN);
		relay.asks[0].resolve({
			result: { success: false, receipt: { transactionHash: '0x' + 'ef'.repeat(32) } }
		});
		await vi.runAllTimersAsync();

		vi.setSystemTime(T0 + 10 * USER_OP_RECEIPT_POLL_INTERVAL_MS);
		expect((await requestUserOpReceipt(HASH, CHAIN)).resolution?.failed).toBe(true);
		expect(relay.asks).toHaveLength(1);
	});
});
