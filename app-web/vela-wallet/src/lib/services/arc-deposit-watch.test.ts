/**
 * r4a-17: an idle home — a Vela account's and a multisig's — sees a native
 * deposit on Arc Testnet within its cadence, without a reload. The real
 * incoming check (`syncReceivedTransfers` → the native watch) on the real
 * cadence (`followIncomingCadence`, 30 s idle), fake timers; only the wire and
 * the home's re-read are stand-ins. The home's first re-read after the coin
 * lands does not reach Arc (its nodes answering 429), and the first reading
 * of the fresh account is taken after the coin landed — the two ways the
 * deposit used to stay unseen until a reload.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const ARC = 5042002;
/** The testnet space, Arc past the first six and not one the person added. */
const SPACE = [11155111, 84532, 421614, 11155420, 80002, 97, 43113, ARC];
const VELA = '0xae0f5d3a95dfa811ea6cea8c77bc6bb6618eb59a';
const MULTISIG = '0x710311c8d71cc405c5b7782be1367eafa3d78937';
const HALF = 5n * 10n ** 17n;

const chain = vi.hoisted(() => ({
	/** What `eth_getBalance` answers, per `address:chain` (base units). */
	balance: new Map<string, bigint>(),
	/** Accounts the token API lists an Arc holding for. */
	holdsOnArc: new Set<string>()
}));

vi.mock('./rpc-pool', () => ({
	poolRpcCall: async (method: string, params: unknown[], chainId: number) => {
		if (method !== 'eth_getBalance') throw new Error(`unexpected ${method}`);
		const wei = chain.balance.get(`${String(params[0]).toLowerCase()}:${chainId}`) ?? 0n;
		return { result: `0x${wei.toString(16)}` };
	}
}));
vi.mock('./wallet-api', () => ({
	fetchTokens: async (address: string) =>
		chain.holdsOnArc.has(address.toLowerCase()) ? [{ network: 'arc-testnet', chainId: ARC }] : []
}));
vi.mock('./tokens-model', () => ({
	tokenChainId: (t: { chainId: number }) => t.chainId
}));
vi.mock('./incoming-transfers', () => ({ fetchIncomingTransfers: async () => [] }));
vi.mock('./networks', () => ({
	getCustomChainIdsSync: () => [],
	inSpace: (id: number) => SPACE.includes(id),
	nativeSymbol: () => 'USDC',
	spaceNetworksSync: () => SPACE.map((chainId) => ({ chainId }))
}));
vi.mock('./records', () => ({ mergeTransactions: async () => 0 }));
vi.mock('./token-metadata', () => ({ resolveTokenMetadata: async () => new Map() }));
vi.mock('$lib/flows/marks', () => ({ tokenLogoURLs: () => [] }));

const { syncReceivedTransfers } = await import('./activity');
const { onNativeBalanceMoved, resetNativeWatchForTests, setShownNative } =
	await import('./native-watch');
const { followIncomingCadence, IDLE_MS } = await import('$lib/wallet/core/incoming-cadence');

/** The home: what it shows of Arc's coin per account, and its re-reads. */
const shown = new Map<string, bigint>();
let rereadReachesArc: (address: string) => boolean = () => true;
let rereads = 0;

function home(address: string): { stop: () => void } {
	shown.set(address, chain.balance.get(`${address}:${ARC}`) ?? 0n);
	setShownNative((a, chainId) => (chainId === ARC ? shown.get(a.toLowerCase()) : 0n));
	const off = onNativeBalanceMoved((a) => {
		rereads++;
		if (rereadReachesArc(a)) shown.set(a, chain.balance.get(`${a}:${ARC}`) ?? 0n);
	});
	void syncReceivedTransfers(address); // the account hand-off's own check
	const stop = followIncomingCadence(() => void syncReceivedTransfers(address), {
		now: () => Date.now(),
		visible: () => true,
		setTimeout: (run, ms) => setTimeout(run, ms),
		clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
		on: () => () => {}
	});
	return {
		stop: () => {
			stop();
			off();
		}
	};
}

beforeEach(() => {
	vi.useFakeTimers();
	resetNativeWatchForTests();
	chain.balance.clear();
	chain.holdsOnArc.clear();
	shown.clear();
	rereads = 0;
	rereadReachesArc = () => true;
});
afterEach(() => vi.useRealTimers());

describe('an idle home sees a deposit on Arc without a reload (r4a-17)', () => {
	for (const [who, address, holds] of [
		['a Vela account holding nothing yet', VELA, false],
		['a multisig already holding on Arc', MULTISIG, true]
	] as const) {
		it(`${who}: within the idle cadence`, async () => {
			if (holds) {
				chain.holdsOnArc.add(address);
				chain.balance.set(`${address}:${ARC}`, HALF);
			}
			const before = chain.balance.get(`${address}:${ARC}`) ?? 0n;
			const { stop } = home(address);
			await vi.advanceTimersByTimeAsync(IDLE_MS);
			expect(shown.get(address)).toBe(before);
			expect(rereads).toBe(0);
			chain.balance.set(`${address}:${ARC}`, before + HALF);
			await vi.advanceTimersByTimeAsync(IDLE_MS);
			expect(shown.get(address)).toBe(before + HALF);
			stop();
		});
	}

	it("a coin that landed before the watch's first reading is still seen", async () => {
		chain.balance.set(`${VELA}:${ARC}`, HALF);
		shown.set(VELA, 0n);
		setShownNative((a, chainId) => (chainId === ARC ? shown.get(a.toLowerCase()) : 0n));
		onNativeBalanceMoved((a) => shown.set(a, chain.balance.get(`${a}:${ARC}`) ?? 0n));
		const stop = followIncomingCadence(() => void syncReceivedTransfers(VELA), {
			now: () => Date.now(),
			visible: () => true,
			setTimeout: (run, ms) => setTimeout(run, ms),
			clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
			on: () => () => {}
		});
		await vi.advanceTimersByTimeAsync(IDLE_MS);
		expect(shown.get(VELA)).toBe(HALF);
		stop();
	});

	it("a re-read that doesn't reach Arc is asked again, and the deposit shows", async () => {
		chain.holdsOnArc.add(MULTISIG);
		let busy = true;
		rereadReachesArc = () => !busy;
		const { stop } = home(MULTISIG);
		await vi.advanceTimersByTimeAsync(IDLE_MS);
		chain.balance.set(`${MULTISIG}:${ARC}`, HALF);
		await vi.advanceTimersByTimeAsync(IDLE_MS);
		expect(rereads).toBe(1);
		expect(shown.get(MULTISIG)).toBe(0n); // Arc was busy
		busy = false;
		await vi.advanceTimersByTimeAsync(2 * IDLE_MS);
		expect(shown.get(MULTISIG)).toBe(HALF);
		stop();
	});
});
