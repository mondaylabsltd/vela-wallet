/**
 * home-5: a native coin paid to an account with no code leaves no log, so the
 * home watches its balance — a move between two readings is a payment the
 * balances must read again for. Only the wire is mocked.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/** Per chain, what `eth_getBalance` answers (a hex), or `'down'`. */
const wire = new Map<number, string>();
const asked: number[] = [];

vi.mock('./rpc-pool', () => ({
	poolRpcCall: async (method: string, _params: unknown[], chainId: number) => {
		expect(method).toBe('eth_getBalance');
		asked.push(chainId);
		const answer = wire.get(chainId);
		if (answer === undefined || answer === 'down') throw new Error('every endpoint failed');
		return { result: answer };
	}
}));

import {
	decimalToBase,
	MAX_ASKS,
	onNativeBalanceMoved,
	RECHECK_MS,
	resetNativeWatchForTests,
	setShownNative,
	watchNativeBalances
} from './native-watch';

const ADDR = '0x9d76dd0000000000000000000000000000B9b56a';

beforeEach(() => {
	wire.clear();
	asked.length = 0;
	resetNativeWatchForTests();
});

describe('watchNativeBalances', () => {
	it('the first reading is a baseline; a later change is a move, said once to every listener', async () => {
		const heard: string[] = [];
		onNativeBalanceMoved((address) => heard.push(address));
		wire.set(100, '0x0');
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
		wire.set(100, '0xde0b6b3a7640000'); // 1 xDAI arrived
		expect(await watchNativeBalances(ADDR, [100])).toBe(true);
		expect(heard).toEqual([ADDR]);
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
	});

	it('a read that fails is no reading: never a move to or from nothing', async () => {
		wire.set(100, '0x1');
		await watchNativeBalances(ADDR, [100]);
		wire.set(100, 'down');
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
		wire.set(100, '0x1');
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
		wire.set(100, 'garbage');
		expect(await watchNativeBalances(ADDR, [100])).toBe(false);
	});

	it('each network and each account is its own baseline; a network is asked once a tick', async () => {
		wire.set(1, '0x5');
		wire.set(100, '0x5');
		await watchNativeBalances(ADDR, [1, 100, 100]);
		expect(asked.sort()).toEqual([1, 100]);
		// Another account reading the same figure is a first reading, not a move.
		expect(await watchNativeBalances('0x00000000000000000000000000000000000000aa', [1])).toBe(
			false
		);
		wire.set(1, '0x6');
		expect(await watchNativeBalances(ADDR, [1, 100])).toBe(true);
	});
});

/** Arc Testnet: its coin is USDC, 18 places natively. */
const ARC = 5042002;
const HALF = 5n * 10n ** 17n;
const hex = (wei: bigint) => `0x${wei.toString(16)}`;

describe('held against what the home shows (r4a-17)', () => {
	afterEach(() => vi.useRealTimers());

	it('a coin that landed before the first reading is still asked for', async () => {
		const heard: string[] = [];
		onNativeBalanceMoved((address) => heard.push(address));
		setShownNative(() => 0n); // the home settled on nothing
		wire.set(ARC, hex(HALF)); // …and the first reading already holds 0.5
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
		expect(heard).toEqual([ADDR]);
	});

	it('a re-read that did not land asks again after a while, a few times at most', async () => {
		vi.useFakeTimers();
		let shown = 0n;
		setShownNative(() => shown);
		const heard: string[] = [];
		onNativeBalanceMoved((address) => heard.push(address));
		wire.set(ARC, '0x0');
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
		wire.set(ARC, hex(HALF));
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
		// The home's re-read did not reach Arc (429): still 0 shown.
		vi.advanceTimersByTime(30_000);
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
		vi.advanceTimersByTime(RECHECK_MS);
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
		for (let i = 2; i < MAX_ASKS; i++) {
			vi.advanceTimersByTime(RECHECK_MS);
			expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
		}
		vi.advanceTimersByTime(RECHECK_MS);
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
		expect(heard).toHaveLength(MAX_ASKS);
		// The home caught up: quiet, and the next deposit is a move at once.
		shown = HALF;
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
		wire.set(ARC, hex(2n * HALF));
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
	});

	it('nothing settled to hold it against: two readings decide, as before', async () => {
		setShownNative(() => undefined);
		wire.set(ARC, hex(HALF));
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
		wire.set(ARC, hex(2n * HALF));
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(true);
		expect(await watchNativeBalances(ADDR, [ARC])).toBe(false);
	});

	it('reads the home figure exactly', () => {
		expect(decimalToBase('0.5', 18)).toBe(HALF);
		expect(decimalToBase('12', 6)).toBe(12_000_000n);
		expect(decimalToBase('0.1563871', 6)).toBeUndefined();
		expect(decimalToBase('1e3', 18)).toBeUndefined();
	});
});
