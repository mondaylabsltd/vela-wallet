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
 * Only a change between two readings counts: the first reading of a network
 * is the baseline, and a read that fails is no reading (never a "change" to
 * or from nothing).
 */
import { poolRpcCall } from './rpc-pool';

/** The last native balance read, per account and network (`address:chain`). */
const seen = new Map<string, bigint>();
const listeners = new Set<(address: string) => void>();

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
 * listener told) when one moved since the last reading of it. Never throws.
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
			return before !== undefined && before !== wei;
		})
	);
	if (!moved.some(Boolean)) return false;
	for (const listener of listeners) listener(address);
	return true;
}

/** Tests only: forget every reading. */
export function resetNativeWatchForTests(): void {
	seen.clear();
	listeners.clear();
}
