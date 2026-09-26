// Issue #212 — "the same BNB transfer is quoted 0.000332 BNB one time and
// 0.000665 the next, and the estimate changes quickly".
//
// Two rules are pinned here, both at the web's edge of the fee machine:
//  1. the chain's gas signals and the relay's gas quote are measured once per
//     15 s per chain, so editing the recipient cannot re-roll the price — but a
//     failed read is never the thing that gets held, and the refresh affordance
//     and the fault seam both get past the cache;
//  2. the TypeScript twin of `advance_generic` charges the same with or without
//     the relay's quote (the core's own test is
//     `issue_212_fee_is_the_same_with_or_without_the_relay_quote`).
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Answer = { result?: unknown; error?: { code: number; message: string } };
const answers = new Map<string, () => Answer | Promise<Answer>>();
const asked: string[] = [];
const count = (method: string) => asked.filter((m) => m === method).length;

vi.mock('./rpc-adapter', () => ({
	rpcCall: vi.fn(async (method: string) => {
		asked.push(method);
		const answer = answers.get(method);
		if (!answer) throw new Error(`transport down: ${method}`);
		return { jsonrpc: '2.0', id: 1, ...(await answer()) };
	})
}));

const NATIVE_ROW = {
	recipient: '0x' + '11'.repeat(20),
	asset: 'native' as const,
	feeToken: null,
	balance: 10n ** 18n,
	decimals: 18,
	symbol: 'BNB',
	usdBalance: '600.00',
	usdPrice: '600.00000000'
};
vi.mock('./bundler-service', async (original) => ({
	...(await original<typeof import('./bundler-service')>()),
	fetchInBandGasQuotes: vi.fn(async () => [NATIVE_ROW]),
	fetchBundlerAccountInfo: vi.fn(async () => ({ depositAddress: '0x' + '22'.repeat(20) }))
}));

import {
	_resetFeeSignalsCache,
	accountIsDeployed,
	estimateTransactionFee,
	fetchRawBundlerQuote,
	fetchRawGasSignals,
	invalidateFeeSignals,
	refreshGasPrice,
	sendNative,
	simulateUserOpGas
} from './safe-transaction';
import { installFaultConsole } from './fault-injection';

// The fault verbs are only published on `window.vela` (the e2e/dev console), so
// the node project borrows a bare window to reach the same seam the browser does.
type FaultConsole = { zeroGasQuote(chainId: number): void; clearFaults(): void };
vi.stubGlobal('window', {});
installFaultConsole();
const faults = (window as unknown as { vela: FaultConsole }).vela;

const BSC = 56;
const GWEI_0_05 = '0x' + (50_000_000).toString(16);
const GWEI_0_1 = '0x' + (100_000_000).toString(16);

function chainAnswers(gasPrice: string) {
	answers.set('eth_gasPrice', () => ({ result: gasPrice }));
	answers.set('eth_getBlockByNumber', () => ({ result: { baseFeePerGas: '0x0' } }));
	answers.set('eth_maxPriorityFeePerGas', () => ({ result: gasPrice }));
}
function relayAnswers(maxFeePerGas: string) {
	// The relay as it answers today: no networkFeePerGas / relayerFeePerGas.
	answers.set('pimlico_getUserOperationGasPrice', () => ({
		result: { fast: { maxFeePerGas, maxPriorityFeePerGas: maxFeePerGas } }
	}));
}

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date('2026-09-20T00:00:00Z'));
	vi.spyOn(console, 'log').mockImplementation(() => {});
	answers.clear();
	asked.length = 0;
	_resetFeeSignalsCache();
	faults.clearFaults();
});
afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe('fetchRawGasSignals — one measurement per 15 s (issue #212)', () => {
	it('a second ask inside the window reuses the first measurement, even if the endpoint now disagrees', async () => {
		chainAnswers(GWEI_0_05);
		const first = await fetchRawGasSignals(BSC, true);
		// The next endpoint in the pool runs BSC's OLD 0.1 gwei minimum — the 2×.
		chainAnswers(GWEI_0_1);
		vi.advanceTimersByTime(14_000);
		const second = await fetchRawGasSignals(BSC, true);
		expect(first.ethGasPrice).toBe('50000000');
		expect(second).toEqual(first);
		expect(count('eth_gasPrice')).toBe(1);
		expect(count('eth_getBlockByNumber')).toBe(1);
		expect(count('eth_maxPriorityFeePerGas')).toBe(1);
	});

	it('measures again once the window has passed', async () => {
		chainAnswers(GWEI_0_05);
		await fetchRawGasSignals(BSC, true);
		vi.advanceTimersByTime(15_001);
		await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(2);
	});

	it('invalidateFeeSignals forces the next ask to measure', async () => {
		chainAnswers(GWEI_0_05);
		await fetchRawGasSignals(BSC, true);
		invalidateFeeSignals(BSC);
		chainAnswers(GWEI_0_1);
		const fresh = await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(2);
		expect(fresh.ethGasPrice).toBe('100000000');
	});

	it('is per chain: invalidating one chain leaves the other held', async () => {
		chainAnswers(GWEI_0_05);
		await fetchRawGasSignals(BSC, true);
		await fetchRawGasSignals(1, true);
		expect(count('eth_gasPrice')).toBe(2);
		invalidateFeeSignals(1);
		await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(2);
		await fetchRawGasSignals(1, true);
		expect(count('eth_gasPrice')).toBe(3);
	});

	it('never holds a failed eth_gasPrice read — one hiccup must not own the next 15 s', async () => {
		answers.set('eth_getBlockByNumber', () => ({ result: { baseFeePerGas: '0x0' } }));
		const failed = await fetchRawGasSignals(BSC, true);
		expect(failed.ethGasPrice).toBeNull();
		chainAnswers(GWEI_0_05);
		const recovered = await fetchRawGasSignals(BSC, true);
		expect(recovered.ethGasPrice).toBe('50000000');
		expect(count('eth_gasPrice')).toBe(2);
	});

	it('never holds a zero eth_gasPrice — the core prices that at its 5 gwei default, 100x BSC', async () => {
		chainAnswers('0x0');
		expect((await fetchRawGasSignals(BSC, true)).ethGasPrice).toBe('0');
		chainAnswers(GWEI_0_05);
		expect((await fetchRawGasSignals(BSC, true)).ethGasPrice).toBe('50000000');
		expect(count('eth_gasPrice')).toBe(2);
	});

	it('never holds a half-failed read: a missing tip under-prices Gnosis ~40x', async () => {
		chainAnswers(GWEI_0_05);
		answers.delete('eth_maxPriorityFeePerGas');
		const tipless = await fetchRawGasSignals(100, true);
		expect(tipless).toEqual({ ethGasPrice: '50000000', baseFee: '0', priorityFee: null });
		chainAnswers(GWEI_0_05);
		expect((await fetchRawGasSignals(100, true)).priorityFee).toBe('50000000');
		expect(count('eth_gasPrice')).toBe(2);
	});

	it('never holds a read whose block leg failed', async () => {
		chainAnswers(GWEI_0_05);
		answers.delete('eth_getBlockByNumber');
		expect((await fetchRawGasSignals(BSC, true)).baseFee).toBeNull();
		await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(2);
	});

	it('still holds the complete reads that look sparse: a 0x0 tip, a pre-London block, a skipped tip', async () => {
		chainAnswers(GWEI_0_05);
		answers.set('eth_maxPriorityFeePerGas', () => ({ result: '0x0' }));
		answers.set('eth_getBlockByNumber', () => ({ result: { number: '0x1' } }));
		const first = await fetchRawGasSignals(BSC, true);
		expect(first).toEqual({ ethGasPrice: '50000000', baseFee: null, priorityFee: '0' });
		await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(1);
		// Tempo asks for no tip; its absence is not a failed leg.
		await fetchRawGasSignals(4217, false);
		await fetchRawGasSignals(4217, false);
		expect(count('eth_gasPrice')).toBe(2);
		expect(count('eth_maxPriorityFeePerGas')).toBe(1);
	});

	it('concurrent asks share one round', async () => {
		chainAnswers(GWEI_0_05);
		const [a, b, c] = await Promise.all([
			fetchRawGasSignals(BSC, true),
			fetchRawGasSignals(BSC, true),
			fetchRawGasSignals(BSC, true)
		]);
		expect(count('eth_gasPrice')).toBe(1);
		expect(b).toEqual(a);
		expect(c).toEqual(a);
	});

	it('a read already in flight when the person refreshes cannot land its older answer', async () => {
		let release: (answer: Answer) => void = () => {};
		answers.set('eth_gasPrice', () => new Promise<Answer>((resolve) => (release = resolve)));
		answers.set('eth_getBlockByNumber', () => ({ result: { baseFeePerGas: '0x0' } }));
		answers.set('eth_maxPriorityFeePerGas', () => ({ result: '0x0' }));
		const old = fetchRawGasSignals(BSC, true);
		invalidateFeeSignals(BSC);
		release({ result: GWEI_0_1 });
		expect((await old).ethGasPrice).toBe('100000000');
		chainAnswers(GWEI_0_05);
		expect((await fetchRawGasSignals(BSC, true)).ethGasPrice).toBe('50000000');
	});

	it('refreshGasPrice takes the fee signals with it', async () => {
		chainAnswers(GWEI_0_05);
		await fetchRawGasSignals(BSC, true);
		await refreshGasPrice(BSC);
		const before = count('eth_gasPrice');
		await fetchRawGasSignals(BSC, true);
		expect(count('eth_gasPrice')).toBe(before + 1);
	});

	// The submit entries drop the cache at their START, so even a submit that
	// dies on its first read leaves the next quote measuring again. Each send is
	// made to die at the nonce read (eth_call is left unanswered).
	it.each([
		['in-band', BSC],
		['Tempo', 4217]
	])('the %s submit drops the fee signals on its way in', async (_name, chainId) => {
		chainAnswers(GWEI_0_05);
		answers.set('eth_getCode', () => ({ result: '0x6080' }));
		await fetchRawGasSignals(chainId, true);
		const before = count('eth_gasPrice');
		await fetchRawGasSignals(chainId, true);
		expect(count('eth_gasPrice')).toBe(before);
		await expect(
			sendNative('0x' + '88'.repeat(20), '0x' + '11'.repeat(20), '1', chainId, 'ab', async () => {
				throw new Error('must not reach signing');
			})
		).rejects.toThrow(/nonce/);
		const afterSubmit = count('eth_gasPrice');
		await fetchRawGasSignals(chainId, true);
		expect(count('eth_gasPrice')).toBe(afterSubmit + 1);
	});
});

describe('fetchRawBundlerQuote — one relay quote per 15 s (issue #212)', () => {
	it('a second ask inside the window reuses the quote; invalidate asks again', async () => {
		relayAnswers('0x' + (61_200_000).toString(16));
		const first = await fetchRawBundlerQuote(BSC, 'fast');
		relayAnswers('0x' + (79_000_000).toString(16));
		const second = await fetchRawBundlerQuote(BSC, 'fast');
		expect(first).toEqual({
			maxFeePerGas: '61200000',
			// The signed tip, carried since issue 684 — BSC has no base fee, so
			// its whole price IS the tip and the two coincide.
			maxPriorityFeePerGas: '61200000',
			networkFeePerGas: null,
			relayerFeePerGas: null
		});
		expect(second).toEqual(first);
		expect(count('pimlico_getUserOperationGasPrice')).toBe(1);
		invalidateFeeSignals(BSC);
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('79000000');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(2);
	});

	it('never holds a missing quote, whether the transport threw or the relay answered an error', async () => {
		expect(await fetchRawBundlerQuote(BSC, 'fast')).toBeNull();
		answers.set('pimlico_getUserOperationGasPrice', () => ({
			error: { code: -32601, message: 'method not found' }
		}));
		expect(await fetchRawBundlerQuote(BSC, 'fast')).toBeNull();
		relayAnswers(GWEI_0_05);
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('50000000');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(3);
	});

	it('never holds a zero quote — the core refuses it as degenerate, and that must not own 15 s', async () => {
		relayAnswers('0x0');
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('0');
		relayAnswers(GWEI_0_05);
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('50000000');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(2);
	});

	it('concurrent asks share one call', async () => {
		relayAnswers(GWEI_0_05);
		await Promise.all([fetchRawBundlerQuote(BSC, 'fast'), fetchRawBundlerQuote(BSC, 'fast')]);
		expect(count('pimlico_getUserOperationGasPrice')).toBe(1);
	});

	it('the vela.zeroGasQuote fault seam sits ahead of the cache: a held real quote cannot mask it, and the forged zero is never held', async () => {
		relayAnswers(GWEI_0_05);
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('50000000');
		faults.zeroGasQuote(BSC);
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('0');
		faults.clearFaults();
		// Back to the held REAL quote — the zero did not replace it.
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('50000000');
	});
});

// The founder's "the three speeds' fees appear one after another": each speed
// row is its own fee session, and each made its own calls. The relay's gas
// quote is ONE answer for every tier, and the simulation does not depend on the
// speed — so every session must read the one answer, at the same moment.
describe('the speed rows share one relay quote and one simulation', () => {
	const SAFE = '0x' + '88'.repeat(20);
	const hex = (n: number) => '0x' + n.toString(16);

	it('one pimlico_getUserOperationGasPrice answers every tier, concurrent or later', async () => {
		answers.set('pimlico_getUserOperationGasPrice', () => ({
			result: {
				slow: { maxFeePerGas: hex(40_000_000), maxPriorityFeePerGas: hex(1) },
				standard: { maxFeePerGas: hex(50_000_000), maxPriorityFeePerGas: hex(2) },
				fast: { maxFeePerGas: hex(60_000_000), maxPriorityFeePerGas: hex(3) }
			}
		}));
		const [slow, standard, fast] = await Promise.all([
			fetchRawBundlerQuote(BSC, 'slow'),
			fetchRawBundlerQuote(BSC, 'standard'),
			fetchRawBundlerQuote(BSC, 'fast')
		]);
		expect([slow?.maxFeePerGas, standard?.maxFeePerGas, fast?.maxFeePerGas]).toEqual([
			'40000000',
			'50000000',
			'60000000'
		]);
		expect(count('pimlico_getUserOperationGasPrice')).toBe(1);
		// A tier asked later in the window is read from the same answer.
		expect((await fetchRawBundlerQuote(BSC, 'standard'))?.maxPriorityFeePerGas).toBe('2');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(1);
	});

	it('a zero row is not held, and does not stop the good rows beside it from being held', async () => {
		answers.set('pimlico_getUserOperationGasPrice', () => ({
			result: {
				slow: { maxFeePerGas: '0x0' },
				fast: { maxFeePerGas: GWEI_0_05 }
			}
		}));
		expect((await fetchRawBundlerQuote(BSC, 'slow'))?.maxFeePerGas).toBe('0');
		expect((await fetchRawBundlerQuote(BSC, 'fast'))?.maxFeePerGas).toBe('50000000');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(1);
		// The zero row is asked again, never pinned.
		await fetchRawBundlerQuote(BSC, 'slow');
		expect(count('pimlico_getUserOperationGasPrice')).toBe(2);
	});

	function simulation() {
		return simulateUserOpGas({
			chainId: BSC,
			account: SAFE,
			deployed: true,
			calls: [{ to: '0x' + '11'.repeat(20), value: '0x1', data: '0x' }]
		});
	}

	it('the in-force session and the previews get one simulation of one exact operation', async () => {
		answers.set('eth_call', () => ({ result: '0x' + '0'.repeat(64) }));
		answers.set('eth_estimateUserOperationGas', () => ({
			result: {
				verificationGasLimit: hex(100_000),
				callGasLimit: hex(50_000),
				preVerificationGas: hex(40_000)
			}
		}));
		const [a, b, c] = await Promise.all([simulation(), simulation(), simulation()]);
		expect(count('eth_estimateUserOperationGas')).toBe(1);
		expect(a).toEqual({
			kind: 'estimated',
			verificationGasLimit: 100_000n,
			callGasLimit: 50_000n,
			preVerificationGas: 40_000n
		});
		expect(b).toEqual(a);
		expect(c).toEqual(a);
		// Held for the window…
		await simulation();
		expect(count('eth_estimateUserOperationGas')).toBe(1);
		// …dropped with the fee signals (a submit, a refresh, a failed quote)…
		invalidateFeeSignals(BSC);
		await simulation();
		expect(count('eth_estimateUserOperationGas')).toBe(2);
		// …and gone when the window passes.
		vi.advanceTimersByTime(15_001);
		await simulation();
		expect(count('eth_estimateUserOperationGas')).toBe(3);
	});

	it('a refused simulation is shared while out, but never held', async () => {
		answers.set('eth_call', () => ({ result: '0x' + '0'.repeat(64) }));
		answers.set('eth_estimateUserOperationGas', () => ({
			error: { code: -32500, message: 'AA23 reverted' }
		}));
		const [a, b] = await Promise.all([simulation(), simulation()]);
		expect(a).toEqual({ kind: 'simulation_failed' });
		expect(b).toEqual(a);
		expect(count('eth_estimateUserOperationGas')).toBe(1);
		await simulation();
		expect(count('eth_estimateUserOperationGas')).toBe(2);
	});

	it('concurrent deployment reads share one eth_getCode; an undeployed answer is never held', async () => {
		// A Safe of its own: a deployed answer is held for good, by design, and
		// the submit tests above deploy `SAFE`.
		const fresh = '0x' + '77'.repeat(20);
		answers.set('eth_getCode', () => ({ result: '0x' }));
		const [a, b] = await Promise.all([
			accountIsDeployed(fresh, BSC),
			accountIsDeployed(fresh, BSC)
		]);
		expect([a, b]).toEqual([false, false]);
		expect(count('eth_getCode')).toBe(1);
		await accountIsDeployed(fresh, BSC);
		expect(count('eth_getCode')).toBe(2);
	});
});

// Issue #212 looked like a bug here and is not one. With the relay's quote
// missing, the fee is priced on measurement × tier (2× at `fast`) instead of on
// the measurement alone, so the same send reads twice as dear. That is
// deliberate, and the relay's own `docs/fees.md` is why:
//
//   ① reject: R > 3 × C            (MAX_QUOTE_VS_CHAIN_MULTIPLE)
//   ② anchor: basis = max(C, R)    C = our chain measurement, R = the relay's quote
//   ③ pay:    3 × gas × basis      (INBAND_MARKUP)
//
// The relay requires only `1.4 × cost` to settle (settlement_markup_bps, floored
// at 10000 so it can never go below cost). Our 3× is the quote→inclusion drift
// buffer, and it is spent, not spare: the client prices against quote time
// (~1.2×base) while the relay settles against inclusion time (2×base'), and the
// signed amount is NEVER re-priced before submit. Netted out, that buys about a
// +70% base-fee spike; past it the relay reprices down into what was actually
// paid, and past THAT the op fails the inclusion floor and is refused. The doc
// says it plainly: "a client that lowered its markup toward the relay's 1.4×
// would lose almost all drift tolerance."
//
// So equalising the two paths — which an earlier pass of #212 proposed — would
// have halved the buffer exactly when the client is blindest. Pricing the blind
// path at 2 × C is not padding: it lands on the relay's own settlement basis
// (2×base') instead of the quote-time basis it cannot see. `docs/fees.md`
// documents only the quoted pipeline, so this path is the undocumented corner.
//
// What #212 did fix is everything AROUND this: the endpoint that reported 2×
// and 20× the real price is gone, and a measurement is held for 15 s, so the
// figure no longer moves when only the recipient does.
describe('estimateTransactionFee — a missing relay quote carries more headroom, on purpose (issue #212)', () => {
	const SAFE = '0x' + '88'.repeat(20);
	async function quote(relay: 'answers' | 'down') {
		answers.clear();
		chainAnswers(GWEI_0_05);
		answers.set('eth_getCode', () => ({ result: '0x6080' }));
		answers.set('eth_call', () => ({ result: '0x' + '0'.repeat(64) }));
		if (relay === 'answers') relayAnswers('0x' + (61_200_000).toString(16));
		// eth_estimateUserOperationGas is left unanswered → the static gas model,
		// identical on both runs.
		await refreshGasPrice(BSC);
		return estimateTransactionFee(SAFE, BSC, 'fast');
	}

	it('prices a quoted run on the measurement, and an unquoted one on measurement × tier', async () => {
		const quoted = await quote('answers');
		const fallback = await quote('down');
		expect(quoted.quoted).toBe(true);
		expect(fallback.quoted).toBe(false);
		// Same gas, same measurement — only the relay's answer differs.
		expect(fallback.totalGas).toBe(quoted.totalGas);
		expect(quoted.inBandGasBasis).toBe(50_000_000n);
		expect(quoted.totalWei).toBe(quoted.totalGas * 50_000_000n * 3n);
		// Flying blind: the fast tier's 2× rides into what is charged.
		expect(fallback.networkFeePerGas).toBe(100_000_000n);
		expect(fallback.inBandGasBasis).toBe(100_000_000n);
		expect(fallback.totalWei).toBe(quoted.totalGas * 100_000_000n * 3n);
		// The reporter's two figures, to the wei: 0.000332 and 0.000665 BNB are
		// this pair at 2,216,000 gas — never below what a quoted run pays.
		expect(fallback.totalWei).toBe(quoted.totalWei * 2n);
	});
});
