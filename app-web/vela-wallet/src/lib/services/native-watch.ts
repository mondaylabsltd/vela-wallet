/**
 * Native coins arriving while the home listens (home-5) — WEB.
 *
 * The Activity scan finds a payment by its log: an ERC-20's `Transfer`, the
 * wallet's own `SafeReceived`, an EIP-7708 native log. A plain native transfer
 * to an account with no code yet leaves no log at all, so "Live · listening
 * for payments" sat over a deposit for up to the 10-minute poll. This watches
 * the one fact such a payment does change — the coin's balance — on the same
 * networks the scan watches, beside it on every tick, and says when it moved.
 * What follows (a forced re-read of the balances) is the caller's; what the
 * balances ARE stays the balance core's.
 *
 * It cannot make an Activity row: a balance has no sender and no transaction.
 *
 * A read that fails is no reading (never a "change" to or from nothing).
 *
 * r4a-17: what a reading is held against is what the home SHOWS for that
 * coin, where the home has settled a round for the account
 * ({@link setShownNative}) — not only the reading before it. Two readings
 * alone missed a deposit for good on Arc: the first reading taken after the
 * coin had landed is a baseline that already holds it, and a re-read that
 * did not reach the network (Arc's nodes answering 429) left the home on the
 * old figure while the watch had already spent the change. Held against the
 * home, a coin that is there and not shown asks again — at most once a
 * {@link RECHECK_MS} for the same figure and {@link MAX_ASKS} times, so a
 * network whose rows never agree cannot keep the home re-reading. Where the
 * home shows nothing settled for the account, a change between two readings
 * counts, as before.
 */
import { poolRpcCall } from './rpc-pool';

/** How long a figure the home has not caught up with waits before it asks again. */
export const RECHECK_MS = 60_000;
/** How many times one figure asks before it waits for the next change. */
export const MAX_ASKS = 3;

/**
 * What the home shows of `address`'s native coin on `chainId`, in base
 * units: `0n` for no row, `undefined` when the home has no settled figure
 * for that account there (or the network has no coin of its own).
 */
export type ShownNative = (address: string, chainId: number) => bigint | undefined;

/** The last native balance read, per account and network (`address:chain`). */
const seen = new Map<string, bigint>();
/** The figure the home was last asked to catch up with, per `address:chain`. */
const asked = new Map<string, { wei: bigint; at: number; times: number }>();
const listeners = new Set<(address: string) => void>();
let shownNative: ShownNative = () => undefined;

/** Tell the watch what the home shows (the feed does, once booted). */
export function setShownNative(shown: ShownNative): void {
	shownNative = shown;
}

/**
 * `"1.5"` with `decimals` places as base units; `undefined` when it is not a
 * plain decimal (a figure the watch cannot hold a reading against).
 */
export function decimalToBase(amount: string, decimals: number): bigint | undefined {
	const match = /^(\d+)(?:\.(\d+))?$/.exec(amount.trim());
	if (!match || !Number.isInteger(decimals) || decimals < 0) return undefined;
	const fraction = match[2] ?? '';
	if (fraction.length > decimals) return undefined;
	return BigInt(match[1] + fraction.padEnd(decimals, '0'));
}

/** Hear every move; returns the unsubscribe. */
export function onNativeBalanceMoved(listener: (address: string) => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

async function readNative(address: string, chainId: number): Promise<bigint | null> {
	try {
		const response = await poolRpcCall('eth_getBalance', [address, 'latest'], chainId);
		if (response.error || typeof response.result !== 'string') return null;
		if (!/^0x[0-9a-fA-F]+$/.test(response.result)) return null;
		return BigInt(response.result);
	} catch {
		return null;
	}
}

/**
 * Read `address`'s native balance on each of `chainIds`; `true` (and every
 * listener told) when one is not what the home shows — or, with nothing
 * settled to hold it against, moved since the last reading. Never throws.
 */
export async function watchNativeBalances(address: string, chainIds: number[]): Promise<boolean> {
	if (!address) return false;
	const account = address.toLowerCase();
	const moved = await Promise.all(
		[...new Set(chainIds)].map(async (chainId) => {
			const wei = await readNative(address, chainId);
			if (wei === null) return false;
			const key = `${account}:${chainId}`;
			const before = seen.get(key);
			seen.set(key, wei);
			const shown = shownNative(address, chainId);
			const behind = shown === undefined ? before !== undefined && before !== wei : shown !== wei;
			if (!behind) {
				asked.delete(key);
				return false;
			}
			const now = Date.now();
			const last = asked.get(key);
			if (last && last.wei === wei) {
				if (last.times >= MAX_ASKS || now - last.at < RECHECK_MS) return false;
				asked.set(key, { wei, at: now, times: last.times + 1 });
				return true;
			}
			asked.set(key, { wei, at: now, times: 1 });
			return true;
		})
	);
	if (!moved.some(Boolean)) return false;
	for (const listener of listeners) listener(address);
	return true;
}

/** Tests only: forget every reading. */
export function resetNativeWatchForTests(): void {
	seen.clear();
	asked.clear();
	listeners.clear();
	shownNative = () => undefined;
}
