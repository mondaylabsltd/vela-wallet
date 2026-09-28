/**
 * The local transaction record — WEB (spec 025 D14).
 *
 * The `LocalTransaction` stored shape, ported from src/services/storage.ts
 * @ c13e89d4: the bytes under `vela.transactionHistory` on every client. Read
 * by the activity feed (folded through the core), written by the receive
 * sync here and by 026's send path later. Fields the web does not yet produce
 * stay declared so a record written elsewhere round-trips untouched.
 */

export type TransactionType =
	'send' | 'receive' | 'dapp_tx' | 'sign_message' | 'sign_typed_data' | 'connect';

export interface LocalTransaction {
	id: string;
	userOpHash: string;
	/** On-chain tx hash. Empty string for off-chain signatures. */
	txHash: string;
	from: string;
	to: string;
	/** Resolved identity name of the recipient (e.g. "vitalik.eth"). */
	toName?: string;
	value: string;
	symbol: string;
	decimals: number;
	/** Ordered token-logo URL candidates, captured at write time. */
	logoUrls?: string[];
	chainId: number;
	/** Unix seconds. */
	timestamp: number;
	status: 'pending' | 'confirmed' | 'failed';
	/** Defaults to 'send' for records older than the field. */
	type?: TransactionType;
	/** `requestDApp(...)?.name ?? origin` — a dApp's own name when it gave one. */
	dappOrigin?: string;
	/**
	 * The origin the request arrived from, never the dApp's name (083 H2).
	 * Activity names the site from this alone, so a dApp cannot choose it.
	 */
	dappUrl?: string;
	intent?: string;
	/** USD value at event time, pre-formatted (e.g. "$1.00"). */
	usd?: string;
	signedContent?: string;
	signedRequest?: { method: string; params: unknown[] };
	requestTruncated?: boolean;
	/** Opaque to this feature; carried so a stored record survives a rewrite. */
	assetSim?: unknown;
}

/** Cap on persisted signed content (`storage.ts:434` on Expo) — a record stays a record, not a payload dump. */
export const MAX_SIGNED_CONTENT = 8000;
