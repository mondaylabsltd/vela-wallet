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
	accountInfo: vi.fn()
}));

vi.mock('$lib/core/kernels', () => ({ fromHex: vi.fn(), verifySafeWebAuthn: vi.fn() }));
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
	updateTransactions: vi.fn()
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

import { createSendExecutor } from './send-executor';
import type { SendShellPorts } from './send-types';

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
