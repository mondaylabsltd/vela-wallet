// Ported from src/services/incoming-transfers.ts @ c13e89d4 (spec 025).
/**
 * Incoming-transfer discovery — WEB, driven by the `token_trust` core
 * (spec 017, `rust/crates/vela-core/src/app/token_trust.rs`).
 *
 * Replaces `transfer-monitor.ts`'s decision half on web: the per-chain
 * allowlist, the probe + single capped retry, the local `topics[2]`
 * re-verification, the timestamp cap and the metadata gate are all the core's
 * now. What is left here is the call shape `activity.ts` expects and the wire
 * translation of the machine's judged feed.
 *
 * Two honest differences from the TypeScript monitor, both benign downstream:
 *
 *  - The answer is the machine's whole de-duped feed for this account, not
 *    just this tick's window. `activity.ts` merges by the same stable id, so a
 *    repeat is a no-op and a row that only became renderable on a later poll
 *    still lands.
 *  - An ERC-20 whose metadata never resolved is withheld by the core
 *    (invariant ③) instead of being returned and dropped by `activity.ts`'s
 *    own index filter. Same rows reach the store either way; the core's is the
 *    earlier gate.
 */

import { pollIncoming } from '$lib/wallet/core/token-trust-resident';
import type { IncomingTransfer } from './transfer-types';
import type { TrustIncomingView } from '$lib/core/generated/TrustIncomingView';
import type { TrustScanFacts } from '$lib/core/generated/TrustScanFacts';

/** The judged feed row in the shape `activity.ts` consumes. */
function toTransfer(row: TrustIncomingView): IncomingTransfer {
	return {
		id: row.id,
		chainId: row.chain_id,
		token: row.token,
		isNative: row.is_native,
		from: row.from,
		value: BigInt(row.value),
		txHash: row.tx_hash,
		blockNumber: row.block_number,
		logIndex: row.log_index,
		timestamp: row.timestamp_sec
	};
}

/**
 * Scan for incoming transfers to `address`. Newest-first, the same ordering
 * `fetchIncomingTransfers` promised (the core's view sorts by block then log
 * index descending). r9-17: the core plans the scan from `facts` — the
 * networks, the live window, the bounded backfill; `scanChainIds` is its own
 * answer for those facts (`incomingScanPlan`), here only to prime the
 * registry's stablecoins of those networks before it runs.
 */
export async function fetchIncomingTransfers(
	address: string,
	facts: TrustScanFacts,
	scanChainIds: number[]
): Promise<IncomingTransfer[]> {
	if (!address) return [];
	const incoming = await pollIncoming(address, facts, scanChainIds);
	return incoming.map(toTransfer);
}
