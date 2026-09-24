/**
 * The request window's approve half — and the CORE's answer (spec 027 T330,
 * re-pointed by spec 070 T063).
 *
 * The shell does not author a connection. It states what happened and reads
 * the operations back as the verdict — "the operation IS the answer" — on a
 * throwaway core, constructed and freed inside one call:
 *
 * ```text
 *   popup_approved       ─► write_grant + save_connection_record + respond
 *   popup_account_switch ─► write_grant (re-pinned) | remove_grant | nothing
 * ```
 *
 * Until T063 each of these drove the machine's BROWSER half instead: a
 * `provider_request` and a grant read to reach a consent sheet, a
 * `navigation_started` to make a grant-store machine notice an origin, a
 * `browser_closed` to read a constant back. The window has no tab, no document
 * and no navigation; every real in-app browser is on `dapp_browser` since 070,
 * and the impersonation went with it.
 *
 * What is deliberately NOT done here:
 *
 * **The page events.** The browser path's approve also emits `accountsChanged`
 * / `chainChanged`, which exist for a live document. This window answers ONE
 * request and closes — the `Respond` IS the accounts announcement — so the
 * core does not author them on this entry at all.
 *
 * **`loadCore()` must have resolved before this is called** (026's rule: no
 * kernel call at import time).
 */

import { DappPermissionsCore, dpermSettleOnClose } from '$lib/core/client';

import type { DpermEvent } from '$lib/core/generated/DpermEvent';
import type { DpermGrant } from '$lib/core/generated/DpermGrant';
import type { DpermOperation } from '$lib/core/generated/DpermOperation';
import type { DpermView } from '$lib/core/generated/DpermView';
import type {
	PopupConnectPlan,
	PopupConnectQuestion,
	PopupConnectRecord,
	PopupSettlement
} from './dperm-connect-types';

interface DispatchResult {
	view: DpermView;
	effects: { id: number; operation: DpermOperation }[];
}

/**
 * The operations the core authors for one approved connection.
 *
 * Throws when the core does not sanction the connection (see the module note).
 * Every throw is recoverable at the call site: nothing has been persisted and
 * nothing has been sent to the dApp yet, so the window can put the person back on
 * the consent card with Connect and Cancel both still working.
 */
export function planPopupConnect(question: PopupConnectQuestion): PopupConnectPlan {
	const core = new DappPermissionsCore();
	try {
		// One event, because one thing happened: the person pressed Connect.
		//
		// This used to seed three facts, dispatch `provider_request`, drain the
		// grant read it parked on, assert that a consent sheet had opened for
		// this origin, and only then approve — the window impersonating an
		// in-app browser to reach an approve that was written for one. Spec 070
		// moved every real browser onto `dapp_browser`, and T063 took the
		// browser half of `dapp_permissions` away; `popup_approved` is what the
		// window was always asking for.
		const effects = (
			JSON.parse(
				core.dispatch(
					JSON.stringify({
						type: 'popup_approved',
						origin: question.origin,
						request_id: question.requestId,
						method: question.method,
						address: question.activeAddress,
						chain_id: question.chainId,
						now_ms: question.nowMs
					} satisfies DpermEvent)
				)
			) as DispatchResult
		).effects;

		let grant: PopupConnectPlan['grant'] | null = null;
		let record: PopupConnectRecord | null = null;
		let respond: PopupConnectPlan['respond'] | null = null;
		for (const { operation } of effects) {
			switch (operation.type) {
				case 'write_grant':
					grant = operation.grant;
					break;
				case 'save_connection_record':
					record = {
						address: operation.address,
						chainId: operation.chain_id,
						origin: operation.origin
					};
					break;
				case 'respond':
					// Addressed to this request or to nothing: a payload aimed at another
					// id is not this window's answer.
					if (operation.id === question.requestId) respond = operation.payload;
					break;
				default:
					break;
			}
		}
		if (!grant || !record || !respond) {
			throw new Error('dapp_permissions authored an incomplete connection');
		}
		return { grant, record, respond };
	} finally {
		core.free();
	}
}

/** What the wallet observed when its active account changed. */
export interface AccountSwitchQuestion {
	origin: string;
	/** The `vela.perm.<origin>` value as read. */
	storedGrant: DpermGrant;
	/** Every wallet address; `null`/empty means "not known yet". */
	currentAddresses: string[] | null;
	/** The account the wallet switched TO. */
	activeAddress: string;
	nowMs: number;
}

/** What the core authored for one connected site on an account switch. */
export type AccountSwitchPlan =
	/** `WriteGrant` — the grant re-pinned to the new address. */
	| { kind: 'repin'; grant: DpermGrant }
	/** `RemoveGrant` — the grant's own account left the wallet. */
	| { kind: 'remove' }
	/** The core said nothing: the site was not connected, or nothing changed. */
	| { kind: 'none' };

/**
 * The wallet switched accounts — what does a connected site hear?
 *
 * `account_switched` is the core's rule (`dapp_permissions.rs`): a CONNECTED
 * origin's grant is re-pinned to the new address, and the page is told
 * `accountsChanged([new])`. In the in-app browser that rule fires for the one
 * current origin; the extension has a granted origin per site, so it is asked
 * once per grant, on a throwaway core seeded the way the popup seeds it — the
 * address set, then a navigation to the origin so the core reads the grant
 * and holds the site as connected, then the switch.
 *
 * The `EmitEvent` the core authors alongside is not performed here: the
 * worker announces a grant CHANGE to every tab of the origin (its storage
 * listener), which is the same event, from the same fact, for every writer.
 *
 * **`loadCore()` must have resolved before this is called.**
 */
export function planAccountSwitch(question: AccountSwitchQuestion): AccountSwitchPlan {
	const core = new DappPermissionsCore();
	try {
		// One event, because one thing happened: the wallet switched account.
		//
		// This used to replay a navigation and the page's first `eth_accounts`
		// — a browser's opening moves — to make a grant-store machine re-pin
		// one row. `popup_account_switch` asks the question directly; the two
		// rules behind it (`should_drop_grant`, and the chain being an audit
		// fact a switch must not rewrite) are unchanged and still the core's.
		const effects = (
			JSON.parse(
				core.dispatch(
					JSON.stringify({
						type: 'popup_account_switch',
						origin: question.origin,
						grant: question.storedGrant,
						current_addresses: question.currentAddresses,
						active_address: question.activeAddress,
						now_ms: question.nowMs
					} satisfies DpermEvent)
				)
			) as DispatchResult
		).effects;

		for (const { operation } of effects) {
			if (operation.type === 'write_grant') return { kind: 'repin', grant: operation.grant };
			if (operation.type === 'remove_grant') return { kind: 'remove' };
		}
		return { kind: 'none' };
	} finally {
		core.free();
	}
}

/**
 * How the core settles a request still pending when the window goes away.
 *
 * Asked rather than restated: `browser_closed` names the code and the reason in
 * `SettleForwarded`, and 4900-not-4001 is the entire reason that operation
 * carries a code at all.
 */
export function popupCloseSettlement(): PopupSettlement {
	// The core names it; this reads it. `browser_closed` used to be dispatched
	// into a throwaway core purely to read the operation it authored back —
	// which is a long way round to a constant the core can simply state.
	return JSON.parse(dpermSettleOnClose()) as PopupSettlement;
}
