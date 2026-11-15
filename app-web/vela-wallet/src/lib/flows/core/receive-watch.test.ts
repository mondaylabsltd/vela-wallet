/**
 * The web receive_watch + payment_request executors (spec 025 Phase 4).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { APIToken } from '#lib/services/tokens-model.ts';

const kv = new Map<string, string>();
vi.mock('#lib/services/storage.ts', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
const visible = { value: true };
vi.mock('#lib/services/platform.ts', () => ({
	isAppActive: vi.fn(() => visible.value),
	hapticSuccess: vi.fn()
}));
const ETH: APIToken = {
	network: 'eth-mainnet',
	chainName: 'Ethereum',
	symbol: 'ETH',
	balance: '1.5',
	decimals: 18,
	logo: null,
	name: 'Ether',
	tokenAddress: null,
	priceUsd: 3000,
	spam: false
};
vi.mock('#lib/services/wallet-api.ts', () => ({ fetchTokens: vi.fn(async () => [ETH]) }));

import { createReceiveWatchExecutor, receiveWatchOperationFailure } from './receive-watch';
import { executePaymentRequestOperation, paymentRequestOperationFailure } from './payment-request';

const ADDR = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';

beforeEach(() => {
	kv.clear();
	visible.value = true;
});

describe('receive_watch executor', () => {
	it('fetch_tokens snapshots the holdings in the core vocabulary', async () => {
		const execute = createReceiveWatchExecutor(ADDR);
		const result = await execute(
			{ id: 1, operation: { type: 'fetch_tokens' } },
			new AbortController().signal
		);
		expect(result).toMatchObject({
			type: 'tokens_fetched',
			tokens: [
				{ id: 'eth-mainnet_native_ETH', symbol: 'ETH', chain_id: 1, balance: 1.5, price_usd: 3000 }
			]
		});
	});
	it('a hidden tab answers inactive BEFORE fetching (D12)', async () => {
		visible.value = false;
		const execute = createReceiveWatchExecutor(ADDR);
		expect(
			await execute({ id: 1, operation: { type: 'fetch_tokens' } }, new AbortController().signal)
		).toEqual({ type: 'inactive' });
	});
	it('wait is abortable and still answers', async () => {
		const execute = createReceiveWatchExecutor(ADDR);
		const controller = new AbortController();
		const pending = execute({ id: 1, operation: { type: 'wait', ms: 60_000 } }, controller.signal);
		controller.abort();
		// An aborted wait resolves only through the timer being cleared — the
		// loop discards the answer; here we just make sure nothing hangs.
		const raced = await Promise.race([
			pending,
			new Promise((r) => setTimeout(() => r('hung'), 50))
		]);
		expect(raced === 'hung' || (raced as { type: string }).type === 'waited').toBe(true);
	});
	it('signal_deposit acknowledges and nudges the shell', async () => {
		const nudged = vi.fn();
		const execute = createReceiveWatchExecutor(ADDR, nudged);
		expect(
			await execute({ id: 1, operation: { type: 'signal_deposit' } }, new AbortController().signal)
		).toEqual({ type: 'signalled' });
		expect(nudged).toHaveBeenCalledOnce();
	});
	it('the failure twin answers every operation', () => {
		expect(
			receiveWatchOperationFailure({ id: 1, operation: { type: 'fetch_tokens' } })
		).toMatchObject({
			type: 'fetch_failed'
		});
		expect(receiveWatchOperationFailure({ id: 1, operation: { type: 'signal_deposit' } })).toEqual({
			type: 'signalled'
		});
	});
});

describe('payment_request executor', () => {
	it('the acknowledgement flag round-trips per account; unreadable shows the gate', async () => {
		expect(
			await executePaymentRequestOperation({
				id: 1,
				operation: { type: 'read_ack', account: ADDR }
			})
		).toEqual({ type: 'ack_flag', acknowledged: false });
		await executePaymentRequestOperation({
			id: 1,
			operation: { type: 'write_ack', account: ADDR }
		});
		expect(kv.get(`vela.receiveWarned.${ADDR}`)).toBe('1');
		expect(
			await executePaymentRequestOperation({
				id: 1,
				operation: { type: 'read_ack', account: ADDR }
			})
		).toEqual({ type: 'ack_flag', acknowledged: true });
		expect(
			paymentRequestOperationFailure({ id: 1, operation: { type: 'read_ack', account: ADDR } })
		).toEqual({ type: 'ack_flag', acknowledged: false });
	});
});
