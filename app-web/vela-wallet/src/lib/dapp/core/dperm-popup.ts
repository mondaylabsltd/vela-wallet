/**
 * The request window's permission verdict — `dapp_permissions`, asked one pure
 * question (spec 027 T330).
 *
 * Ported from src/services/wallet-state-core/dperm-popup.ts @ 52ad8fa9.
 *
 * `popup_request` asks for no shell operation, so no effect loop is needed:
 * construct a core, dispatch once, read the typed verdict, free it.
 *
 * The three fund-safety rules this reaches are the core's, not this file's: a
 * never-connected origin gets no address (4100); a signature is pinned to the
 * GRANT's address rather than to whichever account happens to be active; and a
 * request pinning any other address is refused rather than silently re-signed.
 *
 * The core is NOT app-resident: nothing about a one-shot request window
 * survives it, and a resident core would hold a browser model this window never
 * populates.
 *
 * **`loadCore()` must have resolved before this is called.** Every module in
 * this tree obeys 026's rule — no kernel call at import time — so the caller
 * awaits the core, not the module.
 */
import { DappPermissionsCore } from '$lib/core/client';
import type { DpermView } from '$lib/core/generated/DpermView';
import type { PopupRequestQuestion, PopupVerdict } from './dperm-types';

export { dpermRejectMessage, toWireGrant } from './dperm-types';

export function decidePopupRequest(question: PopupRequestQuestion): PopupVerdict {
	const core = new DappPermissionsCore();
	try {
		const result = JSON.parse(
			core.dispatch(
				JSON.stringify({
					type: 'popup_request',
					method: question.method,
					grant: question.grant,
					// `''` and `null` mean the same "nobody" to the core.
					signed_in: question.signedIn ? question.signedIn : null,
					pinned_address: question.pinnedAddress ? question.pinnedAddress : null,
					origin: question.origin ?? null,
					params_json: question.paramsJson ?? null
				})
			)
		) as { view: DpermView };
		const verdict = result.view.popup;
		if (!verdict) {
			// Unreachable: `popup_request` always publishes a verdict. Fail closed
			// rather than let a caller read this as "allowed".
			throw new Error('dapp_permissions returned no popup verdict');
		}
		return verdict;
	} finally {
		core.free();
	}
}
