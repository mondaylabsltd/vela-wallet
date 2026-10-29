/**
 * r4C-16: an owner that approved on chain — called the Safe's
 * `approveHash(safeTxHash)` from its own wallet, as Safe{Wallet} lets an
 * owner do — has approved, though the mailbox holds nothing from it. Read
 * from the chain (`approvedHashes(owner, hash)`), never from a service, and
 * only where it can be there: a deployed Safe, a transaction still open.
 *
 * A yes is kept for the session (the Safe never clears it); a no is asked
 * again at most every {@link RECHECK_MS}, so the 12 s refresh does not ask
 * the chain once per owner per round.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';

const APPROVED_HASHES = '0x7d832974'; // approvedHashes(address,bytes32)
const APPROVE_HASH = '0xd4d9bdcd'; // approveHash(bytes32)
export const RECHECK_MS = 30_000;

const seen = new Map<string, { yes: boolean; at: number }>();

function word(hex: string): string {
	return hex.replace(/^0x/, '').toLowerCase().padStart(64, '0');
}

/** `approvedHashes(owner, hash)` calldata. */
export function approvedHashesCall(owner: string, hash: string): string {
	return `${APPROVED_HASHES}${word(owner)}${word(hash)}`;
}

/** `approveHash(hash)` calldata: an owner's approval on chain, sent from its own wallet. */
export function approveHashCall(hash: string): string {
	return `${APPROVE_HASH}${word(hash)}`;
}

/**
 * Which of `owners` approved `hash` on `safe` on chain. An owner the chain
 * does not answer for is left out (not approved, said nowhere: the mailbox's
 * approvals still count as they are).
 */
export async function onChainApprovals(
	chainId: number,
	safe: string,
	hash: string,
	owners: string[],
	now = Date.now()
): Promise<string[]> {
	const found = await Promise.all(
		owners.map(async (owner) => {
			const key = `${chainId}:${safe}:${hash}:${owner}`.toLowerCase();
			const known = seen.get(key);
			if (known && (known.yes || now - known.at < RECHECK_MS)) return known.yes ? owner : null;
			try {
				const answer = await poolRpcCall(
					'eth_call',
					[{ to: safe, data: approvedHashesCall(owner, hash) }, 'latest'],
					chainId
				);
				if (typeof answer.result !== 'string' || answer.error) return null;
				const yes = /^0x0*$/.test(answer.result) === false;
				seen.set(key, { yes, at: now });
				return yes ? owner : null;
			} catch {
				return null;
			}
		})
	);
	return found.filter((o): o is string => o !== null);
}

/** For tests: forget what was read. */
export function resetOnChainForTests(): void {
	seen.clear();
}
