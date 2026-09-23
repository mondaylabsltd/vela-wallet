/**
 * One request, from arrival to answer (spec 027 T334).
 *
 * The window performs; `dapp_permissions` decides. Every branch below is the
 * core's own verdict — `decide_popup_request` for the question, and
 * `consent_approved` for what an approval authors. Nothing here judges an
 * origin, picks an address, or invents a refusal.
 *
 * The three rules this reaches, all the core's:
 *   - a never-connected origin gets no address (4100), it gets a consent card;
 *   - a signature is pinned to the GRANT's address, never to whichever account
 *     happens to be active;
 *   - a request pinning any other address is refused, not silently re-signed.
 */
import { loadCore } from '$lib/core/client';
import type { DpermRespondPayload } from '$lib/core/generated/DpermRespondPayload';
import { decidePopupRequest } from './core/dperm-popup';
import { planPopupConnect } from './core/dperm-connect';
import { dpermRejectMessage, toWireGrant } from './core/dperm-types';
import { getGrant, setGrant } from './grants';
import { answerRequest, type ExtensionRequest } from './transport';
import type { PopupVerdict } from './core/dperm-types';

/** The wallet facts the core is seeded with — observed, never judged. */
export interface WalletFacts {
	/** The ACTIVE account's derived address (`SessionView.address`). */
	activeAddress: string;
	/** Every derived address, or `null` when storage has not been read yet. */
	addresses: string[] | null;
}

/** What the window should show once the core has ruled. */
export type RequestStage =
	| { kind: 'loading' }
	/** The core wants a person to decide. */
	| { kind: 'consent'; origin: string; method: string }
	/** Answered without asking anyone — the origin was already granted. */
	| { kind: 'done' }
	/** The core refused, in its own words. */
	| { kind: 'refused'; code: number; message: string }
	/** A granted origin asking for a signature — 026's sheet, in Phase 5. */
	| { kind: 'signing'; grantedAddress: string }
	/**
	 * Spec 077: signed, answered, and now landing. The dApp already HAS its
	 * answer — this stage watches the chain, which is the person's business
	 * and not the request's, so nothing in it can answer anything.
	 */
	| { kind: 'landing'; opHash: string; chainId: number };

/**
 * Ask the core what to do with this request, and do the parts that need nobody.
 *
 * Returns what the window should render. An answer that could be sent has been
 * sent by the time this resolves.
 */
export async function evaluate(
	request: ExtensionRequest,
	wallet: WalletFacts
): Promise<RequestStage> {
	await loadCore();
	const grant = await getGrant(request.origin);
	const verdict: PopupVerdict = decidePopupRequest({
		method: request.method,
		grant: toWireGrant(grant),
		currentAddresses: wallet.addresses,
		pinnedAddress: pinnedAddressOf(request)
	});

	switch (verdict.outcome.type) {
		case 'respond':
			await answerRequest(request.rid, { result: encode(verdict.outcome.payload) });
			return { kind: 'done' };

		case 'reject': {
			const message = dpermRejectMessage(verdict.outcome.reason);
			await answerRequest(request.rid, {
				error: { code: verdict.outcome.code, message }
			});
			return { kind: 'refused', code: verdict.outcome.code, message };
		}

		case 'forward_to_signing':
			return { kind: 'signing', grantedAddress: verdict.outcome.granted_address };

		case 'consent':
		default:
			return { kind: 'consent', origin: request.origin, method: request.method };
	}
}

/**
 * The person pressed Connect. The core authors the grant, the audit row and the
 * answer; this writes and sends them, and authors none of them.
 *
 * Throws when the core does not sanction the connection — nothing has been
 * persisted or sent at that point, so the window can put the person back on the
 * card with both buttons still working.
 */
export async function approve(
	request: ExtensionRequest,
	wallet: WalletFacts,
	chainId: number
): Promise<void> {
	await loadCore();
	const stored = await getGrant(request.origin);
	const plan = planPopupConnect({
		origin: request.origin,
		requestId: request.id,
		method: request.method,
		activeAddress: wallet.activeAddress,
		currentAddresses: wallet.addresses,
		chainId,
		nowMs: Date.now(),
		storedGrant: toWireGrant(stored)
	});

	await setGrant({
		origin: plan.grant.origin,
		address: plan.grant.address,
		chainId: plan.grant.chain_id,
		grantedAt: plan.grant.granted_at_ms
	});
	// `plan.record` is the core's "Connected to <app>" audit row. The surface
	// that lists it is Phase 6's; writing it is not optional, because a
	// connection nobody can see is a connection nobody can revoke.
	await saveConnectionRecord(plan.record);
	await answerRequest(request.rid, { result: encode(plan.respond) });
}

/** The EIP-1193 shape of what the core said to answer with. */
export function encode(payload: DpermRespondPayload): unknown {
	switch (payload.type) {
		case 'accounts':
			return payload.addresses;
		case 'permissions':
			// EIP-2255's shape.
			return payload.granted ? [{ parentCapability: 'eth_accounts' }] : [];
		default:
			// A refusal arrives as its own outcome, never as a payload — the
			// `Error` variant went with the browser half (spec 070 T063).
			// Defensive tail.
			return null;
	}
}

/**
 * The address a request pinned, if any. Read by SHAPE from the params, which is
 * how the page-side module reads it too — position is the classic wrong answer.
 */
function pinnedAddressOf(request: ExtensionRequest): string | null {
	for (const param of request.params) {
		if (typeof param === 'string' && /^0x[0-9a-fA-F]{40}$/.test(param)) return param.toLowerCase();
	}
	return null;
}

/**
 * The audit row, written into the wallet's own transaction store — the same
 * place a send lands, because "I connected to this site" belongs in the same
 * history as "I paid this person". `buildConnectionRecord` is 026's port of the
 * shape; `saveTransaction` is 026's writer, under its lock.
 */
async function saveConnectionRecord(record: {
	address: string;
	chainId: number;
	origin: string;
}): Promise<void> {
	const [{ buildConnectionRecord }, { saveTransaction }] = await Promise.all([
		import('$lib/services/dapp-history'),
		import('$lib/services/records')
	]);
	await saveTransaction(
		buildConnectionRecord({
			from: record.address,
			chainId: record.chainId,
			dappOrigin: record.origin,
			nowMs: Date.now()
		})
	);
}
