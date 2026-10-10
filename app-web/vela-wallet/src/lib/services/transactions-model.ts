/**
 * The local transaction record — WEB (spec 025 D14).
 *
 * The `LocalTransaction` stored shape, ported from src/services/storage.ts
 * @ c13e89d4: the bytes under `vela.transactionHistory` on every client. Read
 * by the activity feed (folded through the core), written by the receive
 * sync here and by 026's send path later. Fields the web does not yet produce
 * stay declared so a record written elsewhere round-trips untouched.
 */

import type { DappSummary } from '$lib/core/generated/DappSummary';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import type { TrackSettlement } from '$lib/core/generated/TrackSettlement';

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
	/**
	 * `receive` only (PR 3): `timestamp` is the time of the transaction's own
	 * block, as a chain gave it. Written `true` on every receipt taken from the
	 * `token_trust` feed (whose times are block times) and by the feed's
	 * `write_receive_time`. Absent on a record from before the mark, when a
	 * receipt whose block could not be read was stamped with the clock: the
	 * feed core re-reads such a record's block time and rewrites it.
	 */
	timeVerified?: boolean;
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
	/**
	 * The request as the record keeps it: the core's `stored_request` (≤ 8 KB,
	 * spec 093), parsed back into its params — never a cut of this shell's own.
	 */
	signedRequest?: { method: string; params: unknown[] };
	/** The core's `request_truncated`: `signedRequest` is shorter than the request was. */
	requestTruncated?: boolean;
	/**
	 * What the dApp request was, as Activity states it — the core's
	 * `SignRecord.summary`, stored verbatim and handed back to the feed
	 * untouched (spec 093). Absent on records from before 093.
	 */
	dappSummary?: DappSummary;
	/**
	 * What the wallet's own simulation said the operation moves, as the sheet
	 * drew it when the person approved — the core's `SignRecord.balance_changes`,
	 * verbatim (083 F1, spec 093). Absent when the sheet recorded none.
	 *
	 * A record from before PR 3 holds an unverified token's line in its older
	 * shape (`delta` where there is `direction` now). Nothing on this side
	 * reads the lines: they go back to the core as stored, and it reads both.
	 */
	balanceChanges?: TrustSimJudgment[];
	/** Opaque to this feature; carried so a stored record survives a rewrite. */
	assetSim?: unknown;
	/**
	 * How the operation ended, as the tracker proved it — the closing patch's
	 * `settlement`, stored verbatim and handed back to the feed untouched
	 * (spec 097): what its receipt proves it moved, or why it failed. Absent
	 * on records closed before 097.
	 */
	settlement?: TrackSettlement;
	/**
	 * The submit's reply was lost: `userOpHash` is the locally computed hash and
	 * the op MAY have been sent (spec 082 RA3/RA4). Kept with the row so a reload
	 * hands the tracker a may-have-been-sent op again, not a plain one. Absent on
	 * rows written before 082 — read as `false`.
	 */
	maybeSent?: boolean;
	/**
	 * The chain head read once before the first submit POST — where the
	 * tracker's relay-independent landing check starts (spec 082 ruling 8).
	 * Absent = unknown.
	 */
	submitBlock?: number;
}

/** Cap on persisted signed content (`storage.ts:434` on Expo) — a record stays a record, not a payload dump. */
export const MAX_SIGNED_CONTENT = 8000;
