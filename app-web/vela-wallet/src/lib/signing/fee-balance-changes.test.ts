/**
 * Issue #411 on the web — over the REAL `fee_policy` core.
 *
 * Android 0.9.6: a Uniswap swap on Polygon (pUSD −128.51 → USDC +128.4946)
 * opened with POL as the fee coin; the wallet held no POL, while pUSD (257.02)
 * and USDC (99.99) could each pay ~0.317. The router's path names both
 * stablecoins, so until a simulation says what the swap leaves of them the
 * core cannot count on either (spec 096 F2) and the coin asked for — native —
 * stands. What is pinned here is the shell's half: the measurement reaches
 * every fee session pricing those calls, and again after every new question
 * about them (the core forgets it on each), and a run that reverted or that
 * nobody could check tells the core nothing.
 *
 * The core is real (`wasm-init.server`); only the network is scripted — the
 * fee executor answers each operation with the issue's numbers.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';
import type { FeeOperation } from '$lib/core/generated/FeeOperation';
import type { FeeShellResult } from '$lib/core/generated/FeeShellResult';
import type { FeeCall } from '$lib/core/generated/FeeCall';
import type { FeeBalanceChange } from '$lib/core/generated/FeeBalanceChange';
import type { FeeView } from '$lib/core/generated/FeeView';

const net = vi.hoisted(() => {
	const RECIPIENT = '0x2222222222222222222222222222222222222222';
	const GAS = '2185000000000';
	const stable = (token: string, symbol: string, balance: string, usd: string) => ({
		recipient: RECIPIENT,
		asset: 'erc20',
		fee_token: token,
		balance,
		decimals: 6,
		symbol,
		usd_balance: usd,
		usd_price: '1',
		native_usd_floor_price: null
	});
	return {
		/** Polygon's native USDC, checksummed as the relay writes it. */
		USDC: '0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359',
		/** The swap's input coin (synthetic: only its role matters). */
		PUSD: '0x7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a',
		answer(operation: { type: string; calls?: unknown[] }): Promise<unknown> {
			switch (operation.type) {
				case 'read_deployment':
					// The account read is the fee's own (issue 483): a deployed Safe.
					return Promise.resolve({ type: 'deployment', read: { type: 'read', deployed: true } });
				case 'fetch_gas_price':
					return Promise.resolve({
						type: 'gas_price',
						eth_gas_price: GAS,
						base_fee: '0',
						priority_fee: '0'
					});
				case 'fetch_bundler_quote':
					return Promise.resolve({
						type: 'bundler_quote',
						quote: {
							max_fee_per_gas: '4370000000000',
							max_priority_fee_per_gas: null,
							network_fee_per_gas: GAS,
							relayer_fee_per_gas: GAS
						}
					});
				case 'fetch_in_band_quotes':
					// The three rows the sheet listed: POL 0, pUSD 257.02, USDC 99.99.
					return Promise.resolve({
						type: 'in_band_quotes',
						quotes: [
							{
								recipient: RECIPIENT,
								asset: 'native',
								fee_token: null,
								balance: '0',
								decimals: 18,
								symbol: 'POL',
								usd_balance: '0',
								usd_price: '0.1075',
								native_usd_floor_price: null
							},
							stable(net.PUSD, 'pUSD', '257020000', '257.02'),
							stable(net.USDC, 'USDC', '99990000', '99.99')
						]
					});
				case 'fetch_fee_recipient':
					return Promise.resolve({ type: 'fee_recipient', recipient: RECIPIENT });
				case 'estimate_user_op_gas':
					return Promise.resolve({
						type: 'user_op_gas',
						outcome: {
							type: 'estimated',
							verification_gas_limit: '100000',
							call_gas_limit: '50000',
							pre_verification_gas: '40000'
						}
					});
				case 'measure_inner_calls':
					return Promise.resolve({
						type: 'inner_calls_measured',
						gas: (operation.calls ?? []).map(() => null)
					});
				default:
					// The quote's TTL and the run's deadline: timers these tests never let fire.
					return new Promise(() => {});
			}
		}
	};
});

vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
vi.mock('$lib/services/safe-transaction', () => ({
	invalidateFeeSignals: vi.fn(),
	accountIsDeployed: async () => true
}));
vi.mock('$lib/flows/core/send-estimates', () => ({ resolveFee: () => null }));
vi.mock('$lib/flows/core/fee-session', async () => {
	const { FeePolicyCore } = await import('$lib/core/client');
	const { createJsonWasmShell } = await import('$lib/core/json-shell');
	return {
		createFeeSession: (options: { onView: (view: FeeView) => void }) =>
			createJsonWasmShell<
				FeeView,
				unknown,
				{ id: number; operation: FeeOperation },
				FeeShellResult
			>(new FeePolicyCore(), {
				onView: options.onView,
				execute: (effect) => net.answer(effect.operation) as Promise<FeeShellResult>,
				toFailure: (_effect, error) => {
					throw error;
				},
				onError: (error) => {
					throw error;
				}
			})
	};
});

import { FeeQuote, type FeeQuoteRequest } from '$lib/flows/core/fee-quote.svelte';
import { TierPreview } from '$lib/flows/core/tier-preview.svelte';
import {
	checkRequest,
	feeBalanceChanges,
	measureBalanceChanges,
	simulateVerdict,
	tellBalanceChanges
} from './fee-balance-changes';

const SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const ROUTER = '0x1095692A6237d83C6a72F3F5eFEdb9A670C49223';
const bare = (address: string) => address.slice(2).toLowerCase();
/** A Universal Router `execute` whose V3 path is pUSD → USDC: both named, no `transfer`. */
const SWAP_DATA =
	'0x3593564c' +
	'00'.repeat(32 * 9) +
	bare(net.PUSD) +
	'000064' +
	bare(net.USDC) +
	'00'.repeat(32 * 2 + 9);
const CALLS: FeeCall[] = [{ to: ROUTER, value: '0', data: SWAP_DATA }];
/** What the simulation measured, as the shell hands it over (lower-case contracts). */
const CHANGES: FeeBalanceChange[] = [
	{ token: net.PUSD.toLowerCase(), delta: '-128510000' },
	{ token: net.USDC.toLowerCase(), delta: '128494600' }
];
const REQUEST: FeeQuoteRequest = {
	chainId: 137,
	account: SAFE,
	calls: CALLS,
	feeToken: null,
	autoFeeToken: true,
	publicKeyHex: undefined,
	tier: 'fast'
};

const coin = (view: FeeView, contract: string | null) =>
	view.options.find((option) =>
		contract === null
			? option.contract === null
			: option.contract?.toLowerCase() === contract.toLowerCase()
	);
const paidIn = (view: FeeView | undefined) => view?.fee_token?.toLowerCase() ?? null;

describe('#411 — the swap the simulation measured pays its fee in a coin that can', () => {
	it('unmeasured, the fallback stands (POL, short); told, the fee moves to pUSD — never POL', async () => {
		const quote = new FeeQuote();
		expect(await quote.requestQuote(REQUEST)).toMatchObject({ kind: 'ok' });
		expect(quote.view.fee_token).toBeNull();
		expect(coin(quote.view, null)).toMatchObject({ selected: true, insufficient: true });
		expect(quote.view.confirm_fee_ready).toBe(false);

		quote.balanceChangesMeasured(CALLS, CHANGES);
		expect(paidIn(quote.view)).toBe(net.PUSD);
		// Correctness batch item 4: the figure switched to pUSD is provisional
		// until the gas is measured again with pUSD's fee leg — drawn, with the
		// measuring sign, and never confirmable meanwhile.
		expect(quote.view.provisional).toBe(true);
		expect(quote.view.busy).toBe(true);
		expect(quote.view.confirm_fee_ready).toBe(false);
		await vi.waitFor(() => expect(quote.view.busy).toBe(false));
		expect(quote.view.provisional).toBe(false);
		expect(paidIn(quote.view)).toBe(net.PUSD);
		expect(quote.view.confirm_fee_ready).toBe(true);
		expect(coin(quote.view, null)?.selected).toBe(false);
		expect(quote.view.fee?.fee_asset).toMatchObject({ type: 'erc20', symbol: 'pUSD' });
		quote.dispose();
	});

	it('a chip tap to another coin is measured again before it can be confirmed (item 4)', async () => {
		const quote = new FeeQuote();
		quote.balanceChangesMeasured(CALLS, CHANGES);
		await quote.requestQuote(REQUEST);
		await vi.waitFor(() => expect(quote.view.busy).toBe(false));
		expect(paidIn(quote.view)).toBe(net.PUSD);
		expect(quote.view.confirm_fee_ready).toBe(true);
		// The person taps USDC: the figure moves to USDC at once, provisional.
		quote.selectAsset(net.USDC);
		expect(paidIn(quote.view)).toBe(net.USDC.toLowerCase());
		expect(quote.view.fee).not.toBeNull();
		expect(quote.view.provisional).toBe(true);
		expect(quote.view.confirm_fee_ready).toBe(false);
		await vi.waitFor(() => expect(quote.view.busy).toBe(false));
		expect(quote.view.provisional).toBe(false);
		expect(paidIn(quote.view)).toBe(net.USDC.toLowerCase());
		expect(quote.view.confirm_fee_ready).toBe(true);
		quote.dispose();
	});

	it('a measurement made before the question is told right after it, and after every new one', async () => {
		const quote = new FeeQuote();
		quote.balanceChangesMeasured(CALLS, CHANGES);
		await quote.requestQuote(REQUEST);
		expect(paidIn(quote.view)).toBe(net.PUSD);
		// The same question again (a stale quote's re-ask): the core forgot, and was told again.
		await quote.requestQuote({ ...REQUEST });
		expect(paidIn(quote.view)).toBe(net.PUSD);
		expect(quote.view.confirm_fee_ready).toBe(true);
		// The refresh keeps what it was told.
		quote.requote();
		await vi.waitFor(() => expect(quote.view.busy).toBe(false));
		expect(paidIn(quote.view)).toBe(net.PUSD);
		quote.dispose();
	});

	it('a measurement of other calls is told to no question', async () => {
		const quote = new FeeQuote();
		quote.balanceChangesMeasured([{ ...CALLS[0], data: SWAP_DATA + '00' }], CHANGES);
		await quote.requestQuote(REQUEST);
		expect(quote.view.fee_token).toBeNull();
		quote.dispose();
	});

	it('every other speed is told too — rows made before it and rows made after', async () => {
		const early = new TierPreview();
		early.show(REQUEST, ['slow', 'standard'], 1);
		early.measure(CALLS, CHANGES);
		const late = new TierPreview();
		late.measure(CALLS, CHANGES);
		late.show(REQUEST, ['slow', 'standard'], 1);
		for (const preview of [early, late]) {
			expect(preview.rows).toHaveLength(2);
			for (const row of preview.rows) {
				await vi.waitFor(() => {
					expect(row.quote.view.busy).toBe(false);
					expect(paidIn(row.quote.view)).toBe(net.PUSD);
				});
				expect(row.quote.view.confirm_fee_ready).toBe(true);
			}
			preview.hide();
		}
	});
});

/*
 * PR 3 device round, item 3: what the node's reply MEANS is the core's
 * (`simOutcome`), read here over the REAL wasm — the replies below are
 * `eth_simulateV1` envelopes as a node sends them. One read serves the fee
 * (a check's moves) and the sheet (the core's "No asset changes").
 */
const TRANSFER = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';
const topic = (address: string) => '0x' + bare(address).padStart(64, '0');
const word = (amount: bigint) => '0x' + amount.toString(16).padStart(64, '0');
const moved = (token: string, from: string, to: string, amount: bigint) => ({
	address: token,
	topics: [TRANSFER, topic(from), topic(to)],
	data: word(amount)
});
/** The node's envelope around one block of call results. */
const answered = (...calls: object[]) =>
	JSON.stringify({ jsonrpc: '2.0', id: 1, result: [{ calls }] });
/** The swap ran: pUSD out, USDC back. */
const SWAPPED = answered({
	status: '0x1',
	logs: [
		moved(net.PUSD, SAFE, ROUTER, 128510000n),
		moved(net.USDC, ROUTER, SAFE, 128494600n),
		// Somebody else's transfer in the same call: not the account's.
		moved(net.USDC, ROUTER, '0x' + '99'.repeat(20), 7n)
	]
});
/** It ran, and nothing of the account's moved (an approval). */
const NOTHING_MOVED = answered({ status: '0x1', logs: [] });
/** `Error("STF")`, as a node reports a revert. */
const REVERTED = answered({
	status: '0x0',
	error: { code: 3, message: 'execution reverted' },
	returnData:
		'0x08c379a0' + '20'.padStart(64, '0') + '03'.padStart(64, '0') + '535446'.padEnd(64, '0')
});
/** A status that is neither 0 nor 1, and no error beside it. */
const UNREADABLE_STATUS = answered({ status: '0x2', logs: [] });
/** A node that does not offer the method (Arbitrum's public one). */
const NOT_OFFERED = JSON.stringify({
	jsonrpc: '2.0',
	id: 1,
	error: { code: -32603, message: 'method handler crashed' }
});
const UNREACHABLE = JSON.stringify({ unreachable: true });

describe('#411 — what the simulation hands the fee machine', () => {
	it('maps a check’s moves as the desktop’s fee_balance_changes does', () => {
		expect(
			feeBalanceChanges([
				{ kind: 'native', token: null, delta: '-1000' },
				{ kind: 'erc20', token: net.PUSD.toLowerCase(), delta: '-128510000' },
				// A token move with no contract names no coin: dropped, never native.
				{ kind: 'erc20', token: null, delta: '5' }
			])
		).toEqual([
			{ token: null, delta: '-1000' },
			{ token: net.PUSD.toLowerCase(), delta: '-128510000' }
		]);
	});

	it('a clean run is told; a revert, a node that could not check, or a gone request tells nothing', async () => {
		const clean = vi.fn(async () => SWAPPED);
		const told: unknown[] = [];
		const sink = {
			balanceChanges: (calls: FeeCall[], changes: FeeBalanceChange[]) =>
				told.push({ calls, changes })
		};
		const request = { from: SAFE, calls: CALLS, chainId: 137 };

		expect(await tellBalanceChanges(sink, request, () => true, clean)).toBe(true);
		// The core's own reading of the logs: the account's two moves, by the
		// contract that emitted each — the issue's numbers.
		expect(told).toEqual([{ calls: CALLS, changes: CHANGES }]);
		expect(clean).toHaveBeenCalledWith(SAFE, [{ to: ROUTER, value: '0', data: SWAP_DATA }], 137);

		const thrown = vi.fn(async () => {
			throw new Error('every endpoint failed');
		});
		const silent = [REVERTED, UNREADABLE_STATUS, NOT_OFFERED, UNREACHABLE, 'not json', '{}'].map(
			(reply) => vi.fn(async () => reply)
		);
		for (const simulate of [...silent, thrown]) {
			expect(await measureBalanceChanges(SAFE, CALLS, 137, simulate)).toBeNull();
			expect(await tellBalanceChanges(sink, request, () => true, simulate)).toBe(false);
		}
		expect(await tellBalanceChanges(sink, request, () => false, clean)).toBe(false);
		expect(told).toHaveLength(1);
		// Nothing to ask: nobody is asked.
		const unasked = vi.fn(async () => SWAPPED);
		expect(await simulateVerdict(SAFE, [], 137, unasked)).toBeNull();
		expect(unasked).not.toHaveBeenCalled();
	});
});

describe('one read, the core’s verdict — for the fee and for the sheet (device round, item 3)', () => {
	const request = { from: SAFE, calls: CALLS, chainId: 137 };
	const sinkOf = () => {
		const told: FeeBalanceChange[][] = [];
		return {
			told,
			balanceChanges: (_calls: FeeCall[], changes: FeeBalanceChange[]) => void told.push(changes)
		};
	};

	it('a check under which nothing moves: the fee is told "nothing", the sheet gets the core’s line', async () => {
		const sink = sinkOf();
		const simulate = vi.fn(async () => NOTHING_MOVED);
		const verdict = await checkRequest(sink, request, () => true, simulate);
		expect(verdict).toMatchObject({
			kind: 'deltas',
			deltas: [],
			no_change_key: 'componentsUi.signing.simResultNoChange',
			notice_key: null
		});
		// One read served both.
		expect(simulate).toHaveBeenCalledOnce();
		expect(sink.told).toEqual([[]]);
	});

	it('a check under which something moves carries no line', async () => {
		const sink = sinkOf();
		const verdict = await checkRequest(
			sink,
			request,
			() => true,
			async () => SWAPPED
		);
		expect(verdict?.kind).toBe('deltas');
		expect(verdict?.no_change_key).toBeNull();
		expect(sink.told).toEqual([CHANGES]);
	});

	it('a revert, a status nobody can read, a node that cannot check, no node: never "nothing moves"', async () => {
		const cases: [string, string, string][] = [
			[REVERTED, 'reverts', 'componentsUi.signing.simWillFailReason'],
			// This shell's own port read this one as a success with no moves.
			[UNREADABLE_STATUS, 'not_offered', 'componentsUi.signing.simUnavailableWarning'],
			[NOT_OFFERED, 'not_offered', 'componentsUi.signing.simUnavailableWarning'],
			[UNREACHABLE, 'unreachable', 'componentsUi.signing.simUnavailableWarning'],
			['{}', 'not_offered', 'componentsUi.signing.simUnavailableWarning']
		];
		for (const [reply, kind, notice] of cases) {
			const sink = sinkOf();
			const verdict = await checkRequest(
				sink,
				request,
				() => true,
				async () => reply
			);
			expect(verdict, reply).toMatchObject({ kind, notice_key: notice, no_change_key: null });
			expect(sink.told, reply).toEqual([]);
		}
		// The pool threw: the same "no node answered".
		const thrown = await checkRequest(
			sinkOf(),
			request,
			() => true,
			async () => {
				throw new Error('every endpoint failed');
			}
		);
		expect(thrown).toMatchObject({ kind: 'unreachable', no_change_key: null });
	});

	it('a request that is gone by the time the node answers has no verdict, and nobody is told', async () => {
		const sink = sinkOf();
		expect(
			await checkRequest(
				sink,
				request,
				() => false,
				async () => NOTHING_MOVED
			)
		).toBeNull();
		expect(sink.told).toEqual([]);
	});
});
