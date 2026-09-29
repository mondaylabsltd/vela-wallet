// Spec 078 — the send executor's two new duties.
//
//  1. `FetchTokens` is answered from the asset list's holdings (the `holdings`
//     port) when the balance machine has settled a round for the account, so
//     Send and the home screen show one list; the chain walk is the fallback,
//     and what MUST walk — a refresh after `ClearTokenCache`, the boot after a
//     network was added — still does.
//  2. `PrewarmFees` answers at once and fires the fee executor's OWN cached
//     reads per chain (Tempo: the fee recipient instead of the relay's gas
//     quote), swallowing every error.
// And `EstimateFee` hands `auto_fee_token` through to the fee session.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SendToken } from '$lib/core/generated/SendToken';

const seams = vi.hoisted(() => ({
	fetchTokens: vi.fn(),
	clearTokenCache: vi.fn(),
	addNetwork: vi.fn(),
	deployed: vi.fn(),
	gasSignals: vi.fn(),
	bundlerQuote: vi.fn(),
	inBand: vi.fn(),
	accountInfo: vi.fn(),
	trackSubmitted: vi.fn(),
	withdrawTracked: vi.fn(),
	updateTransactions: vi.fn(),
	deleteTransactions: vi.fn()
}));

vi.mock('$lib/core/kernels', () => ({
	fromHex: vi.fn(),
	verifySafeWebAuthn: vi.fn(),
	userOpWriteAheadWaitMs: () => 5_000,
	userOpNotSentDetail: () => 'relay unreachable; nothing was sent'
}));
vi.mock('$lib/wallet/core/tracker-resident', () => ({
	trackSubmitted: seams.trackSubmitted,
	withdrawTracked: seams.withdrawTracked
}));
vi.mock('$lib/services/networks', () => ({
	getAllNetworksSync: () => [],
	networkId: (id: number) => `chain-${id}`,
	nativeSymbol: () => 'ETH',
	chainName: (id: number) => `Chain ${id}`
}));
vi.mock('$lib/onboarding/core/passkey', () => ({ PasskeyError: class extends Error {} }));
vi.mock('$lib/signing/sign-challenge', () => ({
	cancelChallenge: vi.fn(),
	signChallenge: vi.fn()
}));
vi.mock('$lib/services/add-network.svelte', () => ({
	addCustomNetworkByChainId: seams.addNetwork
}));
vi.mock('$lib/services/bundler-service', () => ({
	parseBundlerUnderfunded: vi.fn(),
	probeTreasury: vi.fn(),
	fetchInBandGasQuotes: seams.inBand,
	fetchBundlerAccountInfo: seams.accountInfo
}));
vi.mock('$lib/services/platform', () => ({ hapticError: vi.fn(), hapticSuccess: vi.fn() }));
vi.mock('$lib/services/recipient-identity', () => ({ resolveRecipientIdentity: vi.fn() }));
vi.mock('$lib/services/recipient-risk', () => ({ resolveRecipientRisk: vi.fn() }));
vi.mock('$lib/services/safe-transaction', () => ({
	keySetOf: vi.fn(),
	sendBatchCalls: vi.fn(),
	UserOpFeeHoldError: class extends Error {},
	UserOpRejectedError: class extends Error {},
	accountIsDeployed: seams.deployed,
	fetchRawGasSignals: seams.gasSignals,
	fetchRawBundlerQuote: seams.bundlerQuote
}));
vi.mock('$lib/services/accounts', () => ({
	findAccountByAddress: vi.fn(),
	findAccountByCredentialId: vi.fn()
}));
vi.mock('$lib/services/records', () => ({
	saveTransactions: vi.fn(),
	updateTransactions: seams.updateTransactions,
	deleteTransactions: seams.deleteTransactions
}));
vi.mock('$lib/services/token-metadata', () => ({ resolveTokenMetadata: vi.fn() }));
vi.mock('$lib/services/sim/tx-simulation', () => ({
	serializeAssetSim: vi.fn(),
	simulateAssetChanges: vi.fn()
}));
vi.mock('$lib/services/wallet-api', () => ({
	clearTokenCache: seams.clearTokenCache,
	fetchTokens: seams.fetchTokens
}));

import { createSendExecutor, setSendTrackerSink } from './send-executor';
import type { SendShellPorts } from './send-types';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import { sendBatchCalls, type SignFn, type SubmitResult } from '$lib/services/safe-transaction';

const ACCOUNT = '0x' + 'aa'.repeat(20);
const HELD: SendToken = {
	network: 'eth-mainnet',
	chain_id: 1,
	symbol: 'ETH',
	balance: '0.043968123456789012',
	decimals: 18,
	token_address: null,
	price_usd: 2700,
	logo_urls: [],
	spam: false
};

function ports(extra: Partial<SendShellPorts> = {}): SendShellPorts {
	return {
		tokensPartial: vi.fn(),
		tokensFetched: vi.fn(),
		credentialId: () => null,
		credentialLoaded: vi.fn(),
		signingStarted: vi.fn(),
		receiptUpdate: vi.fn(),
		alert: vi.fn(),
		close: vi.fn(),
		feeQuote: vi.fn(async () => ({ type: 'failed' as const, kind: 'estimate_failed' as const })),
		...extra
	};
}

const fetchTokens = { id: 1, operation: { type: 'fetch_tokens' as const, address: ACCOUNT } };

beforeEach(() => {
	for (const seam of Object.values(seams)) seam.mockReset();
	seams.fetchTokens.mockResolvedValue([]);
	for (const read of [seams.deployed, seams.gasSignals, seams.bundlerQuote, seams.inBand]) {
		read.mockResolvedValue(null);
	}
	seams.accountInfo.mockResolvedValue(null);
});

describe('FetchTokens — one list with the asset screen (spec 078)', () => {
	it('answers from the settled holdings, without walking a chain', async () => {
		const holdings = vi.fn(async () => ({ kind: 'tokens' as const, tokens: [HELD] }));
		const executor = createSendExecutor(ports({ holdings }));
		const result = await executor.execute(fetchTokens);
		expect(result).toMatchObject({ type: 'tokens_loaded', tokens: [HELD] });
		expect(holdings).toHaveBeenCalledWith(ACCOUNT);
		expect(seams.fetchTokens).not.toHaveBeenCalled();
	});

	it('walks the chains while the balance machine has nothing settled for the account', async () => {
		const executor = createSendExecutor(ports({ holdings: () => ({ kind: 'walk' }) }));
		await executor.execute(fetchTokens);
		expect(seams.fetchTokens).toHaveBeenCalledTimes(1);
	});

	it('two rounds that reached nothing are "could not load tokens" — not an empty picker', async () => {
		const executor = createSendExecutor(ports({ holdings: async () => ({ kind: 'unreadable' }) }));
		expect(await executor.execute(fetchTokens)).toMatchObject({
			type: 'tokens_loaded',
			tokens: null
		});
		expect(seams.fetchTokens).not.toHaveBeenCalled();
	});

	it('a refresh asked from Send walks, once — the list in memory is what it is refreshing', async () => {
		const executor = createSendExecutor(
			ports({ holdings: () => ({ kind: 'tokens', tokens: [HELD] }) })
		);
		await executor.execute({
			id: 1,
			operation: { type: 'clear_token_cache', address: ACCOUNT }
		});
		await executor.execute(fetchTokens);
		expect(seams.fetchTokens).toHaveBeenCalledTimes(1);
		await executor.execute(fetchTokens);
		expect(seams.fetchTokens).toHaveBeenCalledTimes(1);
	});

	it('the boot after a network was added walks — no held list has seen that chain', async () => {
		seams.addNetwork.mockResolvedValue({ ok: true });
		const executor = createSendExecutor(
			ports({ holdings: () => ({ kind: 'tokens', tokens: [HELD] }) })
		);
		await executor.execute({ id: 1, operation: { type: 'add_network', chain_id: 196 } });
		await executor.execute(fetchTokens);
		expect(seams.fetchTokens).toHaveBeenCalledTimes(1);
	});
});

describe('PrewarmFees — the fee caches read ahead while the person chooses', () => {
	it('answers at once and fires the fee executor’s own reads per chain', async () => {
		const executor = createSendExecutor(ports({ feeTier: () => 'standard' }));
		// A read that never answers must not hold the answer back.
		seams.inBand.mockReturnValue(new Promise(() => {}));
		const result = await executor.execute({
			id: 1,
			operation: { type: 'prewarm_fees', account: ACCOUNT, chain_ids: [8453, 4217] }
		});
		expect(result).toEqual({ type: 'fees_prewarmed' });
		expect(seams.deployed).toHaveBeenCalledWith(ACCOUNT, 8453);
		expect(seams.deployed).toHaveBeenCalledWith(ACCOUNT, 4217);
		expect(seams.gasSignals).toHaveBeenCalledWith(8453, true);
		// Tempo asks no tip, and no relay gas quote — its fee recipient instead.
		expect(seams.gasSignals).toHaveBeenCalledWith(4217, false);
		expect(seams.bundlerQuote).toHaveBeenCalledExactlyOnceWith(8453, 'standard');
		expect(seams.accountInfo).toHaveBeenCalledExactlyOnceWith(4217, ACCOUNT);
		expect(seams.inBand).toHaveBeenCalledWith(8453, ACCOUNT);
		expect(seams.inBand).toHaveBeenCalledWith(4217, ACCOUNT);
	});

	it('swallows every failure, thrown or rejected', async () => {
		seams.deployed.mockRejectedValue(new Error('eth_getCode indeterminate'));
		seams.gasSignals.mockImplementation(() => {
			throw new Error('sync throw');
		});
		const executor = createSendExecutor(ports());
		await expect(
			executor.execute({
				id: 1,
				operation: { type: 'prewarm_fees', account: ACCOUNT, chain_ids: [1] }
			})
		).resolves.toEqual({ type: 'fees_prewarmed' });
		// Without a tier port the factory default is warmed.
		expect(seams.bundlerQuote).toHaveBeenCalledWith(1, 'fast');
	});
});

describe('EstimateFee — the fee coin nobody chose', () => {
	it.each([true, false])('hands auto_fee_token = %s to the fee session', async (auto) => {
		const feeQuote = vi.fn(async () => ({
			type: 'failed' as const,
			kind: 'estimate_failed' as const
		}));
		const executor = createSendExecutor(ports({ feeQuote }));
		await executor.execute({
			id: 1,
			operation: {
				type: 'estimate_fee',
				chain_id: 1,
				account: ACCOUNT,
				tx: null,
				batch: null,
				gas_fee_token: null,
				public_key_hex: null,
				auto_fee_token: auto
			}
		});
		expect(feeQuote).toHaveBeenCalledWith(expect.objectContaining({ autoFeeToken: auto }));
	});
});

/**
 * Spec 082 RJ1 (T229): the wallet's own Send writes ahead too — every
 * recipient's record is on disk before a byte of the op leaves, so a quit
 * mid-submit leaves a pending "may have been sent" row the tracker resolves.
 */
describe('the wallet’s own Send writes ahead (spec 082 RJ1)', () => {
	const OP = '0x' + '7d'.repeat(32);
	const submit = {
		id: 1,
		operation: {
			type: 'submit_user_op' as const,
			chain_id: 100,
			account: ACCOUNT,
			public_key_hex: '04' + '11'.repeat(64),
			calls: [],
			max_fee_per_gas: null,
			gas_fee_token: null,
			quoted_fee: null
		}
	};

	beforeEach(() => {
		seams.updateTransactions.mockResolvedValue(undefined);
		seams.deleteTransactions.mockResolvedValue(undefined);
	});

	/** The submit as `submitUserOp` runs it: the gate before the first POST, then the POST. */
	function relayPosts(posted: () => void) {
		vi.mocked(sendBatchCalls).mockImplementation(async (...args: unknown[]) => {
			const signFn = args[4] as SignFn;
			await signFn.beforePost?.({ userOpHash: OP, submitBlock: 9, chainId: 100 });
			posted();
			return {
				userOpHash: OP,
				maybeSent: false,
				submitBlock: 9,
				waitForTxHash: async () => '0x'
			} as SubmitResult;
		});
	}

	it('nothing is POSTed before the records are on disk; ClearToPost lets it go', async () => {
		const dispatched: SendEvent[] = [];
		const posted = vi.fn();
		relayPosts(posted);
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: (event) => dispatched.push(event)
		});
		const submitting = executor.execute(submit);
		await vi.waitFor(() => expect(dispatched).toHaveLength(1));
		expect(dispatched[0]).toMatchObject({ type: 'op_signed', user_op_hash: OP, submit_block: 9 });
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(posted).not.toHaveBeenCalled();
		expect(
			await executor.execute({ id: 2, operation: { type: 'clear_to_post', user_op_hash: OP } })
		).toEqual({ type: 'post_cleared' });
		await expect(submitting).resolves.toMatchObject({
			type: 'submitted',
			user_op_hash: OP,
			maybe_sent: false
		});
		expect(posted).toHaveBeenCalledTimes(1);
	});

	it('no clearance in time → zero POSTs, and the send fails as not sent', async () => {
		vi.useFakeTimers();
		try {
			const posted = vi.fn();
			relayPosts(posted);
			const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
				dispatch: () => {}
			});
			const submitting = executor.execute(submit);
			await vi.advanceTimersByTimeAsync(5_000);
			await expect(submitting).resolves.toEqual({
				type: 'submit_failed',
				failure: { type: 'other', message: 'relay unreachable; nothing was sent' }
			});
			expect(posted).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
	});

	it('the relay took it: its records drop "may have been sent", and the tracker hears `admitted`', async () => {
		setSendTrackerSink(vi.fn());
		try {
			const executor = createSendExecutor(ports());
			expect(
				await executor.execute({
					id: 1,
					operation: { type: 'mark_admitted', record_ids: [`${OP}-0`, `${OP}-1`] }
				})
			).toEqual({ type: 'records_persisted' });
			expect(seams.updateTransactions).toHaveBeenCalledWith([`${OP}-0`, `${OP}-1`], {
				maybeSent: false
			});
			await executor.execute({
				id: 2,
				operation: {
					type: 'track_submitted',
					user_op_hash: OP,
					record_ids: [`${OP}-0`, `${OP}-1`],
					chain_id: 100,
					maybe_sent: false,
					submit_block: 9,
					admitted: true
				}
			});
			// Straight to the tracker, with `admitted` — the page's sink forwards none.
			expect(seams.trackSubmitted).toHaveBeenCalledWith(
				OP,
				[`${OP}-0`, `${OP}-1`],
				100,
				undefined,
				false,
				9,
				true
			);
		} finally {
			setSendTrackerSink(null);
		}
	});

	it('proven never sent: the records go in one write, and the tracker forgets them', async () => {
		const executor = createSendExecutor(ports());
		await executor.execute({
			id: 1,
			operation: { type: 'delete_tx_records', ids: [`${OP}-0`, `${OP}-1`] }
		});
		expect(seams.deleteTransactions).toHaveBeenCalledWith([`${OP}-0`, `${OP}-1`]);
		expect(
			await executor.execute({
				id: 2,
				operation: { type: 'track_withdrawn', user_op_hash: OP, record_ids: [`${OP}-0`] }
			})
		).toEqual({ type: 'track_handed_off' });
		expect(seams.withdrawTracked).toHaveBeenCalledWith(OP, [`${OP}-0`]);
	});
});
