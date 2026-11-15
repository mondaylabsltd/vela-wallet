/**
 * Types for the request window's APPROVE half — the second door into the
 * `dapp_permissions` core (spec 027 T330).
 *
 * Ported from src/services/wallet-state-core/dperm-connect-types.ts @ 52ad8fa9.
 *
 * `dperm-types.ts` covers the window's QUESTION (`decide_popup_request`); this
 * covers what happens after the person presses Connect, which the core owns end
 * to end in `consent_approved`: which address the grant is written for, which
 * chain it records, that a "Connected to <app>" audit row is written at all,
 * and what the dApp is answered with.
 */

import type { DpermGrant } from '#lib/core/generated/DpermGrant.ts';
import type { DpermRejectReason } from '#lib/core/generated/DpermRejectReason.ts';
import type { DpermRespondPayload } from '#lib/core/generated/DpermRespondPayload.ts';

/**
 * Everything the core needs to author one popup connection. Every field is a
 * FACT the shell observed, never a judgement it made:
 *
 * - `chainId` is `peer.request.chainId`, the chain this popup session is for.
 *   The window has no other notion of a current chain — it is a one-shot window
 *   opened for one request, and the request names its chain.
 * - `nowMs` is the shell's clock; the core owns none.
 * - `storedGrant` is the `vela.perm.<origin>` value as read, `null` when
 *   absent or unreadable.
 */
export interface PopupConnectQuestion {
	origin: string;
	/** `the page's request id` — the id the answer must be addressed to. */
	requestId: string;
	/** The JSON-RPC method the popup was opened for. */
	method: string;
	/** The account shown on the consent card. */
	activeAddress: string;
	/** Every wallet address; `null`/empty means "not known yet". */
	currentAddresses: string[] | null;
	chainId: number;
	nowMs: number;
	storedGrant: DpermGrant | null;
}

/** The `SaveConnectionRecord` operation, in the shell's own vocabulary. */
export interface PopupConnectRecord {
	address: string;
	chainId: number;
	/** The full origin. The shell derives the display host — presentation. */
	origin: string;
}

/**
 * The core's three authored operations for one approved connection. The shell
 * performs them; it decides none of them.
 */
export interface PopupConnectPlan {
	/** `WriteGrant` — persist verbatim. */
	grant: DpermGrant;
	/** `SaveConnectionRecord` — the "Connected to <app>" audit row. */
	record: PopupConnectRecord;
	/** `Respond` — the payload `popupResult` encodes for the dApp. */
	respond: DpermRespondPayload;
}

/**
 * How a still-pending request is settled when the window goes away.
 *
 * The code is the core's, and it is the whole point: 4900 unknown-pending,
 * never 4001. A dApp reads 4001 as "the user said no, nothing happened" and
 * re-sends — double-spending a UserOp that may already be at the bundler.
 */
export interface PopupSettlement {
	code: number;
	reason: DpermRejectReason;
}
