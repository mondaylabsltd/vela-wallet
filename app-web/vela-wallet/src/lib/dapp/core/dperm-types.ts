/**
 * Types and copy for the `dapp_permissions` core (spec 027 T330).
 *
 * Ported from src/services/wallet-state-core/dperm-types.ts @ 52ad8fa9.
 */
import type { DpermGrant } from '$lib/core/generated/DpermGrant';
import type { DpermPopupView } from '$lib/core/generated/DpermPopupView';
import type { DpermRejectReason } from '$lib/core/generated/DpermRejectReason';
import type { DAppGrant } from '../grants';

/** The stored grant on the wire. */
export function toWireGrant(grant: DAppGrant | null): DpermGrant | null {
	if (!grant) return null;
	return {
		origin: grant.origin,
		address: grant.address,
		chain_id: grant.chainId,
		granted_at_ms: grant.grantedAt
	};
}

/** One request's question, in the shell's own vocabulary. */
export interface PopupRequestQuestion {
	/** The JSON-RPC method the window was opened for. */
	method: string;
	/** The stored `vela.perm.<origin>` value, or `null` when there is none. */
	grant: DpermGrant | null;
	/**
	 * Every wallet address. `null` (or empty) means "not known yet" — the core
	 * must NOT log the origin out on a transient empty read.
	 */
	currentAddresses: string[] | null;
	/**
	 * The address the request pinned, if any. The empty string is "no pin".
	 * Ignored by the core when `paramsJson` is given (spec 089).
	 */
	pinnedAddress: string | null | undefined;
	/**
	 * The asking page's origin — the browser's fact. A signature asked for by a
	 * public plain-http origin is refused (`insecure_origin`), as in every
	 * in-app browser (spec 089). Absent: no origin rule is asked.
	 */
	origin?: string | null;
	/**
	 * The request's params as JSON. When given, the CORE reads the address the
	 * request names (`dapp_rpc::requested_address`: a transaction's `from`,
	 * `personal_sign`'s second param, typed data's account), spec 089.
	 */
	paramsJson?: string | null;
}

export type PopupVerdict = DpermPopupView;

/**
 * The words for the core's refusal reasons — the core owns the code and the
 * reason, the shell owns the copy. Every string is the one the Expo popup
 * already sent for that situation, verbatim, so no dApp sees a changed message.
 *
 * These are deliberately NOT corpus strings: they are wire messages addressed
 * to a dApp's error handler, not words on a person's screen. What the PERSON
 * reads is in the corpus and rendered by the window.
 */
export function dpermRejectMessage(reason: DpermRejectReason): string {
	switch (reason) {
		case 'not_connected':
			return 'Connect Vela Wallet to this site first';
		case 'stale_authorized_address':
			return 'The requested account is no longer authorized';
		// The in-app browsers' words for the same refusal (`dapp_browser`).
		case 'insecure_origin':
			return 'Signing requires a secure origin';
		// The six refusals an in-app browser could raise went with the browser
		// half of `dapp_permissions` (spec 070 T063): every real browser is on
		// `dapp_browser`, which words its own. What a request WINDOW can refuse
		// is these three.
		case 'browser_closed':
		default:
			return 'The browser closed before the request finished';
	}
}
