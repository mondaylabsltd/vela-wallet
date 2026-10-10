/**
 * The incoming-transfer record (spec 025) — extracted from the Expo
 * `transfer-monitor.ts` so the read path can carry the TYPE without the
 * monitor (whose scanning is the token_trust machine's on web).
 */
export interface IncomingTransfer {
	/** Stable id: `${chainId}-${txHash}-${logIndex}`. */
	id: string;
	chainId: number;
	/** Token contract address (lowercased), or null for native. */
	token: string | null;
	isNative: boolean;
	from: string;
	/** Raw on-chain amount (not yet divided by decimals). */
	value: bigint;
	txHash: string;
	blockNumber: number;
	logIndex: number;
	/** Unix seconds: the block's own time, never the clock's (PR 3). */
	timestamp: number;
}
