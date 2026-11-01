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
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import type { APIToken } from '$lib/services/tokens-model';

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
	noteSubmitFacts: vi.fn(),
	withdrawTracked: vi.fn(),
	trackerView: vi.fn(() => ({ entries: [] as unknown[] })),
	outcomeOf: vi.fn<(entry: unknown) => unknown>(() => null),
	updateTransactions: vi.fn(),
	deleteTransactions: vi.fn(),
	saveTransactions: vi.fn()
}));

/** r5reg-3: what the pool says of the chains now. */
const pool = vi.hoisted(() => ({ rateLimited: new Set<number>(), failed: new Set<number>() }));
vi.mock('$lib/services/rpc-pool', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/rpc-pool')>()),
	getRateLimitedChains: () => pool.rateLimited,
	getFailedRpcChains: () => pool.failed
}));

/** The tracker's subscribers — the send lock's hold listens there (round 3). */
const trackerListeners = vi.hoisted(() => new Set<() => void>());

vi.mock('$lib/core/kernels', () => ({
	fromHex: vi.fn(),
	verifySafeWebAuthn: vi.fn(),
	userOpWriteAheadWaitMs: () => 5_000,
	userOpNotSentDetail: () => 'relay unreachable; nothing was sent'
}));
vi.mock('$lib/wallet/core/tracker-resident', () => ({
	trackSubmitted: seams.trackSubmitted,
	noteSubmitFacts: seams.noteSubmitFacts,
	subscribeTxTracker: (listener: () => void) => {
		trackerListeners.add(listener);
		return () => trackerListeners.delete(listener);
	},
	withdrawTracked: seams.withdrawTracked,
	txTrackerView: seams.trackerView,
	outcomeOf: seams.outcomeOf
}));
vi.mock('$lib/services/networks', () => ({
	getAllNetworksSync: () => [],
	spaceNetworksSync: () => [],
	networkId: (id: number) => `chain-${id}`,
	nativeSymbol: () => 'ETH',
	chainName: (id: number) => `Chain ${id}`
}));
vi.mock('$lib/onboarding/core/passkey', () => ({ PasskeyError: class extends Error {} }));
vi.mock('$lib/signing/sign-challenge', () => ({
	cancelChallenge: vi.fn(),
	signChallenge: vi.fn(),
	VenueBlockedError: class VenueBlockedError extends Error {
		constructor(readonly block: unknown) {
			super('venue blocked');
		}
	},
	PageDeclinedError: class PageDeclinedError extends Error {},
	PageSignFailedError: class PageSignFailedError extends Error {
		constructor(readonly page: string) {
			super('the signing page did not sign');
		}
	}
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
	// The submit's own refusal, as `submitUserOp` throws it.
	UserOpNotSentError: class extends Error {
		constructor(
			readonly rejection: unknown,
			message: string
		) {
			super(message);
		}
	},
	keySetOf: vi.fn(),
	sendBatchCalls: vi.fn(),
	accountIsDeployed: seams.deployed,
	fetchRawGasSignals: seams.gasSignals,
	fetchRawBundlerQuote: seams.bundlerQuote
}));
vi.mock('$lib/services/accounts', () => ({
	findAccountByAddress: vi.fn(),
	findAccountByCredentialId: vi.fn()
}));
vi.mock('$lib/services/records', () => ({
	saveTransactions: seams.saveTransactions,
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
import { createSendSession } from './send-session';
import { sendTokenId, type SendShellPorts } from './send-types';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import { sendBatchCalls, type SignFn, type SubmitResult } from '$lib/services/safe-transaction';
import { PageSignFailedError, VenueBlockedError } from '$lib/signing/sign-challenge';

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

// Each submit drives the core's machines through the wasm; on a loaded
// machine one can run past vitest's 5 s default. Give them room, so a slow
// run is slow — not a timed-out test whose submit lives on into the next.
vi.setConfig({ testTimeout: 30_000 });

beforeEach(() => {
	for (const seam of Object.values(seams)) seam.mockReset();
	// A test that stopped early must not hand its queued answers to the next.
	vi.mocked(sendBatchCalls).mockReset();
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

	it('a network that could not be checked is not told to the machine as incompatible', async () => {
		const executor = createSendExecutor(ports({}));
		const add = { id: 1, operation: { type: 'add_network', chain_id: 196 } } as const;
		seams.addNetwork.mockResolvedValue({ ok: false, reason: 'unverified' });
		expect(await executor.execute(add)).toEqual({
			type: 'network_added',
			outcome: { type: 'error' }
		});
		// A refusal still is one, and an unknown chain still is that.
		seams.addNetwork.mockResolvedValue({ ok: false, reason: 'not-compatible' });
		expect(await executor.execute(add)).toEqual({
			type: 'network_added',
			outcome: { type: 'not_compatible', detail: null }
		});
		seams.addNetwork.mockResolvedValue({ ok: false, reason: 'not-found' });
		expect(await executor.execute(add)).toEqual({
			type: 'network_added',
			outcome: { type: 'not_found' }
		});
	});
});

describe('a picker opened while the home’s round is still out', () => {
	const XDAI: APIToken = {
		network: 'gnosis-mainnet',
		chainName: 'Gnosis',
		symbol: 'xDAI',
		balance: '2',
		decimals: 18,
		logo: null,
		name: 'xDAI',
		tokenAddress: null,
		priceUsd: 1,
		spam: false
	};

	/** Gnosis has answered; another network is still out (the walk never ends here). */
	function roundStillOut() {
		seams.fetchTokens.mockImplementation(
			(_address: string, options: { onProgress?: (tokens: APIToken[]) => void }) => {
				options.onProgress?.([XDAI]);
				return new Promise(() => {});
			}
		);
	}

	it('hands each chunk of the walk to its own session', async () => {
		roundStillOut();
		const dispatch = vi.fn();
		const executor = createSendExecutor(ports({ holdings: () => ({ kind: 'walk' }) }), {
			dispatch
		});
		void executor.execute(fetchTokens);
		await vi.waitFor(() =>
			expect(dispatch).toHaveBeenCalledWith({
				type: 'tokens_partial',
				tokens: [expect.objectContaining({ chain_id: 100, symbol: 'xDAI', balance: '2' })]
			})
		);
	});

	it('lists what has answered, and its row opens the send form — not the asset page', async () => {
		// The desktop test's path (e2e/onchain/multisig.onchain.ts, "the desktop"):
		// the home streams its rows (home-20) and Send opens before the round has
		// settled, so `FetchTokens` walks. The picker must show the row the home
		// shows, or the tap meant for it lands on the home's row beside it.
		roundStillOut();
		let view: SendView | null = null;
		const session = createSendSession({
			onView: (next) => (view = next),
			onError: (error) => {
				throw error;
			},
			ports: ports({ holdings: () => ({ kind: 'walk' }) })
		});
		session.start({
			type: 'open',
			account: {
				id: '',
				address: ACCOUNT,
				name: 'Desk fund',
				multisig: { threshold: 2, owners: 2 }
			},
			params: {
				preselected_symbol: null,
				preselected_network: null,
				prefilled_recipient: null,
				prefilled_chain_id: null,
				prefilled_token_address: null,
				prefilled_amount_base: null,
				locked: false,
				preselected_multi: null
			},
			display: { code: 'USD', rate: 1, fiat_decimals: 2 }
		});
		await vi.waitFor(() => expect(view?.tokens).toHaveLength(1));
		const shown = view as unknown as SendView;
		expect(shown.stage).toBe('select_token');
		expect(shown.loading).toBe(false);
		session.dispatch({ type: 'select_token', token_id: sendTokenId(shown.tokens[0]) });
		expect((view as unknown as SendView).stage).toBe('enter_details');
		expect((view as unknown as SendView).selected_token).toMatchObject({ symbol: 'xDAI' });
		session.dispose();
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
		expect(seams.bundlerQuote).toHaveBeenCalledWith(1, 'standard');
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
	const SENDER = '0x' + 'a1'.repeat(20);
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
			quoted_fee: null,
			review: { chain_id: 100, account: ACCOUNT, transfers: [], fee: null }
		}
	};

	beforeEach(() => {
		seams.updateTransactions.mockResolvedValue(undefined);
		seams.deleteTransactions.mockResolvedValue(undefined);
		seams.trackerView.mockReturnValue({ entries: [] });
		seams.outcomeOf.mockReturnValue(null);
		seams.saveTransactions.mockResolvedValue(undefined);
	});

	/** The core's `PersistTxRecords` for the write-ahead of `hash`: every line, may have been sent. */
	function persistAhead(hash: string) {
		return {
			id: 90,
			operation: {
				type: 'persist_tx_records' as const,
				records: [0, 1].map((i) => ({
					id: `${hash}-${i}`,
					user_op_hash: hash,
					tx_hash: '',
					from: ACCOUNT,
					to: '0x' + '22'.repeat(20),
					to_name: null,
					value: '1',
					symbol: 'xDAI',
					decimals: 18,
					logo_urls: [],
					chain_id: 100,
					timestamp_s: 1,
					usd: null,
					maybe_sent: true,
					submit_block: 9
				}))
			}
		};
	}

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

	it('hands the quote’s measured gas down to the signing, and none when there is none', async () => {
		relayPosts(() => {});
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		const measured = {
			...submit,
			operation: {
				...submit.operation,
				prepared: {
					calls: [],
					deployed: true,
					verification_gas_limit: '100000',
					call_gas_limit: '310000',
					pre_verification_gas: '40000',
					settlement_gas: null
				}
			}
		};
		void executor.execute(measured);
		await vi.waitFor(() => expect(vi.mocked(sendBatchCalls)).toHaveBeenCalledTimes(1));
		expect(vi.mocked(sendBatchCalls).mock.calls.at(-1)?.[9]).toEqual({
			deployed: true,
			verificationGasLimit: 100_000n,
			callGasLimit: 310_000n,
			preVerificationGas: 40_000n,
			settlementGas: null
		});
		void executor.execute({ ...submit, id: 3 });
		await vi.waitFor(() => expect(vi.mocked(sendBatchCalls)).toHaveBeenCalledTimes(2));
		expect(vi.mocked(sendBatchCalls).mock.calls.at(-1)?.[9]).toBeNull();
	});

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
		// The review the person confirmed goes down to the signing, where the
		// operation is attested against it.
		expect(vi.mocked(sendBatchCalls).mock.calls.at(-1)?.[8]).toEqual(submit.operation.review);
		await executor.execute(persistAhead(OP));
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

	/**
	 * The core clears the POST on the store's acknowledgement, and the web
	 * acknowledges a write the store refused (so the machine moves on). A
	 * refused write-ahead must still let nothing out (DESK_B review (2)).
	 */
	/**
	 * The feed's shell contract: after every write of the send's records the
	 * Activity feed reads the store again — r10's due scan and `sent` read
	 * nothing, so a pending send stood absent from Activity (sweep.e2e).
	 */
	it('every write of the send’s records has the feed read them again', async () => {
		const recordsWritten = vi.fn();
		const executor = createSendExecutor(ports({ recordsWritten }));
		await executor.execute(persistAhead(OP));
		expect(recordsWritten).toHaveBeenCalledTimes(1);
		await executor.execute({
			id: 91,
			operation: { type: 'mark_admitted', record_ids: [`${OP}-0`] }
		});
		expect(recordsWritten).toHaveBeenCalledTimes(2);
		await executor.execute({ id: 92, operation: { type: 'delete_tx_records', ids: [`${OP}-0`] } });
		expect(recordsWritten).toHaveBeenCalledTimes(3);
		// A write the store refused put nothing on disk: nothing to read.
		seams.saveTransactions.mockRejectedValueOnce(new Error('QuotaExceededError'));
		await executor.execute(persistAhead(OP));
		expect(recordsWritten).toHaveBeenCalledTimes(3);
	});

	it('records the store refused let nothing out — the send fails as not sent', async () => {
		const posted = vi.fn();
		relayPosts(posted);
		seams.saveTransactions.mockRejectedValueOnce(new Error('QuotaExceededError'));
		const dispatched: SendEvent[] = [];
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: (event) => dispatched.push(event)
		});
		const submitting = executor.execute(submit);
		await vi.waitFor(() => expect(dispatched).toHaveLength(1));
		expect(await executor.execute(persistAhead(OP))).toEqual({ type: 'records_persisted' });
		await executor.execute({ id: 2, operation: { type: 'clear_to_post', user_op_hash: OP } });
		await expect(submitting).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'other', message: 'relay unreachable; nothing was sent' }
		});
		expect(posted).not.toHaveBeenCalled();
	});

	/**
	 * Spec 102 (P2b-W1): an account that cannot sign on the web is refused
	 * before any ceremony, and the core hears WHY — it puts the reason on the
	 * confirm screen in the person's words (`SendView.tx_venue_block`), never
	 * an English sentence of ours and never "try again".
	 */
	it('an account that cannot sign on the web fails as venue_blocked, with the core’s block', async () => {
		const posted = vi.fn();
		vi.mocked(sendBatchCalls).mockImplementationOnce(async (...args: unknown[]) => {
			const signFn = args[4] as SignFn;
			await signFn(new Uint8Array(32), {} as never);
			posted();
			throw new Error('unreachable');
		});
		const { signChallenge } = await import('$lib/signing/sign-challenge');
		vi.mocked(signChallenge).mockRejectedValueOnce(new VenueBlockedError({ type: 'not_on_web' }));
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'venue_blocked', block: { type: 'not_on_web' } }
		});
		expect(posted).not.toHaveBeenCalled();
	});

	/**
	 * Round 7 (r7-2, r7-7): the signing page did not sign — not answering, an
	 * answer that is not this send's — and the core hears which, to say it on
	 * the confirm; never a generic "couldn't be submitted".
	 */
	it('a page that did not sign fails as page_failed, with the page’s reason', async () => {
		vi.mocked(sendBatchCalls).mockImplementationOnce(async (...args: unknown[]) => {
			const signFn = args[4] as SignFn;
			await signFn(new Uint8Array(32), {} as never);
			throw new Error('unreachable');
		});
		const { signChallenge } = await import('$lib/signing/sign-challenge');
		vi.mocked(signChallenge).mockRejectedValueOnce(
			new (PageSignFailedError as unknown as new (page: string) => Error)('mismatch')
		);
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'page_failed', failure: 'mismatch' }
		});
	});

	/**
	 * Correctness batch item 3: another operation of this account still holds
	 * the nonce — the relay's `nonce_in_flight`, or an older relay's
	 * `[existingHash:…]` marker, both read by the core as `NonceHeld`. Not a
	 * failure of the network: the core says "waiting for your last
	 * transaction" and Try again waits for it like any held confirm.
	 */
	it('a submit held behind the account’s previous op fails as previous_pending', async () => {
		const { UserOpNotSentError } = await import('$lib/services/safe-transaction');
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new UserOpNotSentError(
				{ nonce_held: { user_op_hash: '0x' + 'ee'.repeat(32) } },
				'Waiting for an earlier transaction'
			)
		);
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'previous_pending' }
		});
		// Any other refusal is still the relay's words.
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new UserOpNotSentError({ other: 'AA21 didn’t pay prefund' }, 'AA21 didn’t pay prefund')
		);
		await expect(executor.execute(submit)).resolves.toMatchObject({
			type: 'submit_failed',
			failure: { type: 'other' }
		});
	});

	/**
	 * send-15: the nonce this op signed was spent — another transaction of the
	 * account (another tab's) landed first, and the relay answers AA25. Nothing
	 * was sent: told as that, never "couldn't be submitted".
	 */
	it('a spent nonce fails as nonce_used', async () => {
		const { UserOpNotSentError } = await import('$lib/services/safe-transaction');
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		// The core reads the relay's AA25 (`RelayRejection::NonceUsed`); the
		// executor only says what it read.
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new UserOpNotSentError('nonce_used', 'Transaction nonce mismatch. Please try again.')
		);
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'nonce_used' }
		});
		// The relay's words are never read here again: a refusal the core left
		// as `other` stays the relay's.
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new UserOpNotSentError({ other: 'AA25 invalid account nonce' }, 'AA25 invalid account nonce')
		);
		await expect(executor.execute(submit)).resolves.toMatchObject({
			type: 'submit_failed',
			failure: { type: 'other' }
		});
	});

	/** send-11: a prompt that ran out of time is told apart from one closed. */
	it('a passkey prompt that timed out fails as passkey_timed_out; a closed one as cancelled', async () => {
		const { PasskeyError } = await import('$lib/onboarding/core/passkey');
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		const prompt = (timedOut: boolean) =>
			Object.assign(new PasskeyError('cancelled', 'x'), { kind: 'cancelled', timedOut });
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(prompt(true));
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'passkey_timed_out' }
		});
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(prompt(false));
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'passkey_cancelled' }
		});
	});

	/**
	 * r5reg-3: the reads before the passkey (Arc answering 429, the fallback
	 * hosts failing) are said with their time and cause, bounded, and their
	 * giving up is the network's — no passkey opens, nothing is blamed on it.
	 */
	it('a wait before the passkey is said, bounded, and the network’s — never the passkey’s', async () => {
		const { PREPARE_LIMIT_MS } = await import('./prepare-bound');
		const { signChallenge } = await import('$lib/signing/sign-challenge');
		vi.mocked(signChallenge).mockClear();
		vi.useFakeTimers();
		try {
			pool.rateLimited.add(100);
			// The nonce read never answers: the passkey is never reached.
			vi.mocked(sendBatchCalls).mockImplementationOnce(() => new Promise(() => {}));
			const dispatched: SendEvent[] = [];
			const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
				dispatch: (event) => dispatched.push(event)
			});
			const submitting = executor.execute(submit);
			await vi.advanceTimersByTimeAsync(12_000);
			expect(dispatched.at(-1)).toEqual({ type: 'submit_slow', elapsed_ms: 12_000, busy: true });
			await vi.advanceTimersByTimeAsync(PREPARE_LIMIT_MS);
			await expect(submitting).resolves.toEqual({
				type: 'submit_failed',
				failure: { type: 'chain_unreachable', busy: true }
			});
			expect(vi.mocked(signChallenge)).not.toHaveBeenCalled();
		} finally {
			pool.rateLimited.clear();
			vi.useRealTimers();
		}
	});

	it('a read that fails before the passkey is the network’s; any other error keeps its reading', async () => {
		const { PoolFailedError } = await import('$lib/services/rpc-pool');
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: () => {}
		});
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new PoolFailedError('every endpoint failed', { maybeDelivered: false, rateLimited: false })
		);
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'chain_unreachable', busy: false }
		});
		pool.failed.add(100);
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(
			new Error('Could not fetch the account nonce — the network may be unstable.')
		);
		await expect(executor.execute(submit)).resolves.toEqual({
			type: 'submit_failed',
			failure: { type: 'chain_unreachable', busy: false }
		});
		pool.failed.clear();
		vi.mocked(sendBatchCalls).mockRejectedValueOnce(new Error('boom'));
		await expect(executor.execute(submit)).resolves.toMatchObject({
			type: 'submit_failed',
			failure: { type: 'other' }
		});
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
					admitted: true,
					sender: SENDER
				}
			});
			// Straight to the tracker, with `admitted` — the page's sink forwards none —
			// and who signed it, so the account's next confirm waits for this one
			// (correctness batch item 3).
			expect(seams.trackSubmitted).toHaveBeenCalledWith(
				OP,
				[`${OP}-0`, `${OP}-1`],
				100,
				undefined,
				false,
				9,
				true,
				SENDER
			);
		} finally {
			setSendTrackerSink(null);
		}
	});

	/**
	 * The write-ahead hands the op to the tracker BEFORE its POST, and the
	 * send core names the op only when the relay's reply (or its loss) comes
	 * back: a `ReceiptUpdate` before that is a stale hash to it, and dropped.
	 * The chain check can find the landed op while that reply is being lost
	 * (EX-W1 on the Send screen) — the watcher that carried the verdict is
	 * spent, a terminal entry never changes again, and the receipt read "may
	 * have been sent" over money the tracker saw land (the iOS twin: 8772d5dc).
	 */
	it('a verdict the tracker reached while the reply was out reaches the receipt once the op is named', async () => {
		const TX = '0x' + 'fc'.repeat(32);
		vi.mocked(sendBatchCalls).mockImplementation(async (...args: unknown[]) => {
			const signFn = args[4] as SignFn;
			await signFn.beforePost?.({ userOpHash: OP, submitBlock: 9, chainId: 100 });
			return {
				userOpHash: OP,
				maybeSent: true,
				submitBlock: 9,
				waitForTxHash: async () => '0x'
			} as SubmitResult;
		});
		const entry = { user_op_hash: OP, status: 'confirmed', tx_hash: TX };
		seams.trackerView.mockReturnValue({ entries: [entry] });
		seams.outcomeOf.mockImplementation((e: unknown) =>
			e === entry ? { type: 'confirmed', tx_hash: TX } : null
		);
		const dispatched: SendEvent[] = [];
		const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
			dispatch: (event) => dispatched.push(event)
		});
		const submitting = executor.execute(submit);
		await vi.waitFor(() => expect(dispatched).toHaveLength(1));
		await executor.execute(persistAhead(OP));
		await executor.execute({ id: 2, operation: { type: 'clear_to_post', user_op_hash: OP } });
		await expect(submitting).resolves.toMatchObject({ type: 'submitted', maybe_sent: true });
		// Only after the core has the result — it names the op then.
		expect(dispatched.filter((e) => e.type === 'receipt_update')).toEqual([]);
		await vi.waitFor(() =>
			expect(dispatched.filter((e) => e.type === 'receipt_update')).toEqual([
				{
					type: 'receipt_update',
					user_op_hash: OP,
					outcome: { type: 'confirmed', tx_hash: TX }
				}
			])
		);
	});

	it('the admitted hand-off keeps the receipt listening, though an early verdict spent its watcher', async () => {
		// A premature "not sent" (the POST outlasted the grace) was delivered
		// before the op was named — dropped, and the watcher gone with it. The
		// tracker revives the entry on `admitted`; its later landing must still
		// reach the receipt, so the admitted hand-off watches again.
		setSendTrackerSink(vi.fn());
		try {
			const dispatched: SendEvent[] = [];
			const executor = createSendExecutor(ports(), {
				dispatch: (event) => dispatched.push(event)
			});
			await executor.execute({
				id: 1,
				operation: {
					type: 'track_submitted',
					user_op_hash: OP,
					record_ids: [`${OP}-0`],
					chain_id: 100,
					maybe_sent: false,
					submit_block: 9,
					admitted: true,
					sender: SENDER
				}
			});
			const watch = seams.trackSubmitted.mock.calls[0]?.[3] as
				((outcome: unknown) => void) | undefined;
			expect(typeof watch).toBe('function');
			watch?.({ type: 'confirmed', tx_hash: '0x01' });
			expect(dispatched).toEqual([
				{
					type: 'receipt_update',
					user_op_hash: OP,
					outcome: { type: 'confirmed', tx_hash: '0x01' }
				}
			]);
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

describe('round 3: reliable sending', () => {
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
			quoted_fee: null,
			review: { chain_id: 100, account: ACCOUNT, transfers: [], fee: null }
		}
	};
	const REFUSAL = '{"code":-32603,"message":"operation not accepted"}';

	beforeEach(() => {
		seams.noteSubmitFacts.mockReset();
		seams.trackerView.mockReturnValue({ entries: [] });
		trackerListeners.clear();
	});

	/** A submit that comes back as `result`, with no write-ahead gate. */
	function relayAnswers(result: Partial<SubmitResult>, posted = vi.fn()) {
		vi.mocked(sendBatchCalls).mockImplementation(async () => {
			posted();
			return {
				userOpHash: OP,
				maybeSent: false,
				submitBlock: 9,
				waitForTxHash: async () => '0x',
				...result
			} as SubmitResult;
		});
		return posted;
	}

	function trackMaybeSent(executor: ReturnType<typeof createSendExecutor>, maybeSent: boolean) {
		return executor.execute({
			id: 2,
			operation: {
				type: 'track_submitted',
				user_op_hash: OP,
				record_ids: [`${OP}-0`],
				chain_id: 100,
				maybe_sent: maybeSent,
				submit_block: 9,
				admitted: false,
				sender: ACCOUNT
			}
		});
	}

	it('send-5: a lost reply hands the tracker its nonce and the relay’s refusal, after its records', async () => {
		relayAnswers({ maybeSent: true, nonce: '0x5', relayRefusal: REFUSAL });
		setSendTrackerSink(vi.fn());
		try {
			const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }));
			await expect(executor.execute(submit)).resolves.toMatchObject({
				type: 'submitted',
				maybe_sent: true
			});
			expect(seams.noteSubmitFacts).not.toHaveBeenCalled();
			await trackMaybeSent(executor, true);
			expect(seams.noteSubmitFacts).toHaveBeenCalledWith(OP, '0x5', REFUSAL);
		} finally {
			setSendTrackerSink(null);
		}
	});

	it('send-5: an accepted op tells the tracker nothing more', async () => {
		relayAnswers({ maybeSent: false, nonce: '0x5', relayRefusal: null });
		setSendTrackerSink(vi.fn());
		try {
			const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }));
			await executor.execute(submit);
			await trackMaybeSent(executor, false);
			expect(seams.noteSubmitFacts).not.toHaveBeenCalled();
		} finally {
			setSendTrackerSink(null);
		}
	});

	it('send-15: a second tab waits its turn, says so, and goes once the first op is settled', async () => {
		// Two tabs of one browser: one LockManager, two executors.
		const held = new Set<string>();
		const waiters: Array<() => void> = [];
		const locks = {
			request(
				name: string,
				options: { ifAvailable?: boolean },
				callback: (lock: unknown) => unknown
			): Promise<unknown> {
				const run = async () => {
					held.add(name);
					try {
						return await callback({ name });
					} finally {
						held.delete(name);
						waiters.shift()?.();
					}
				};
				if (!held.has(name)) return run();
				if (options.ifAvailable) return Promise.resolve(callback(null));
				return new Promise((resolve) => waiters.push(() => resolve(run())));
			}
		};
		vi.stubGlobal('navigator', { locks });
		try {
			const posted = relayAnswers({ maybeSent: false });
			const first = createSendExecutor(ports({ credentialId: () => 'cred-1' }));
			await first.execute(submit);
			expect(posted).toHaveBeenCalledTimes(1);

			const dispatched: SendEvent[] = [];
			const second = createSendExecutor(ports({ credentialId: () => 'cred-1' }), {
				dispatch: (event) => dispatched.push(event)
			});
			const going = second.execute({ ...submit, id: 3 });
			await vi.waitFor(() => expect(dispatched).toContainEqual({ type: 'send_queued' }));
			await new Promise((resolve) => setTimeout(resolve, 10));
			expect(posted).toHaveBeenCalledTimes(1);

			// The first op is final: the lock goes, and the second reads its nonce.
			seams.trackerView.mockReturnValue({
				entries: [{ user_op_hash: OP, polling: false, stalled: false }]
			});
			for (const listener of [...trackerListeners]) listener();
			await expect(going).resolves.toMatchObject({ type: 'submitted' });
			expect(posted).toHaveBeenCalledTimes(2);
		} finally {
			vi.unstubAllGlobals();
		}
	});

	it('send-15: a submit that failed lets the lock go at once', async () => {
		const requests: string[] = [];
		let holding = 0;
		vi.stubGlobal('navigator', {
			locks: {
				request: async (name: string, _o: unknown, callback: (lock: unknown) => unknown) => {
					requests.push(name);
					holding += 1;
					try {
						return await callback({ name });
					} finally {
						holding -= 1;
					}
				}
			}
		});
		try {
			vi.mocked(sendBatchCalls).mockRejectedValueOnce(new Error('boom'));
			const executor = createSendExecutor(ports({ credentialId: () => 'cred-1' }));
			await expect(executor.execute(submit)).resolves.toMatchObject({ type: 'submit_failed' });
			expect(requests).toEqual([`vela.send:100:${ACCOUNT}`]);
			await vi.waitFor(() => expect(holding).toBe(0));
		} finally {
			vi.unstubAllGlobals();
		}
	});

	it('a history that could not be read reaches the core as unread, never as a first transfer', async () => {
		const { resolveRecipientRisk } = await import('$lib/services/recipient-risk');
		vi.mocked(resolveRecipientRisk).mockResolvedValueOnce({
			isContract: false,
			firstInteraction: null,
			historyUnread: true
		});
		const executor = createSendExecutor(ports());
		await expect(
			executor.execute({
				id: 1,
				operation: { type: 'resolve_risk', chain_id: 100, address: ACCOUNT }
			})
		).resolves.toEqual({
			type: 'risk_resolved',
			risk: {
				is_contract: false,
				first_time: null,
				history_unread: true,
				known_name: null,
				own: false
			}
		});
	});

	it('tells the core the person’s own name for it, and that it is theirs (r4a-18)', async () => {
		const { resolveRecipientRisk } = await import('$lib/services/recipient-risk');
		vi.mocked(resolveRecipientRisk).mockResolvedValueOnce({
			isContract: true,
			firstInteraction: true,
			historyUnread: false
		});
		const executor = createSendExecutor(
			ports({
				knownName: (address) => (address === ACCOUNT ? ' R5 Bob ' : null),
				ownAddress: (address) => address === ACCOUNT
			})
		);
		await expect(
			executor.execute({
				id: 1,
				operation: { type: 'resolve_risk', chain_id: 100, address: ACCOUNT }
			})
		).resolves.toEqual({
			type: 'risk_resolved',
			risk: {
				is_contract: true,
				first_time: true,
				history_unread: false,
				known_name: 'R5 Bob',
				own: true
			}
		});
	});
});

describe('a known recipient is named as the person knows them (vis-24)', () => {
	const resolve = (address: string) => ({
		id: 1,
		operation: { type: 'resolve_identity' as const, address }
	});

	it('their own name first — an account or a contact — told as theirs, with no lookup', async () => {
		const { resolveRecipientIdentity } = await import('$lib/services/recipient-identity');
		vi.mocked(resolveRecipientIdentity).mockReset();
		const executor = createSendExecutor(
			ports({ knownName: (address) => (address === ACCOUNT ? 'Savings' : null) })
		);
		await expect(executor.execute(resolve(ACCOUNT))).resolves.toEqual({
			type: 'identity_resolved',
			identity: { name: 'Savings', source: 'self' }
		});
		expect(resolveRecipientIdentity).not.toHaveBeenCalled();
	});

	it('anyone else is looked up as before', async () => {
		const { resolveRecipientIdentity } = await import('$lib/services/recipient-identity');
		vi.mocked(resolveRecipientIdentity).mockResolvedValueOnce({
			name: 'bob.eth',
			source: 'ENS'
		} as never);
		const executor = createSendExecutor(ports({ knownName: () => null }));
		await expect(executor.execute(resolve('0x' + 'bb'.repeat(20)))).resolves.toEqual({
			type: 'identity_resolved',
			identity: { name: 'bob.eth', source: 'ENS' }
		});
	});
});
