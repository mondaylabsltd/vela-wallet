/**
 * What Vela Connect answers without asking anyone (spec 109) — the half the
 * extension's service worker answers there, and the in-app browsers'
 * `dapp_browser` machine answers on the phones and the desktop.
 *
 * Every rule is the core's (`dapp_rpc::classify`, `dapp_permissions`'
 * `granted_to_signed_in`, `dapp_rpc::capabilities`, `dapp_rpc::calls_status`)
 * or the worker's twin of it that the shared vectors pin (`switchRefusal`,
 * `withLastChain`). This module reads and writes what they rule on and
 * nothing more. **`loadCore()` must have resolved.**
 */
import {
	catalogChainIds,
	parseChainId,
	switchChainParam,
	switchRefusal,
	toHexChainId,
	withLastChain
} from '../../../extension/lib/protocol.js';
import { dappArea } from '#lib/dapp/area.ts';
import { decidePopupRequest, toWireGrant } from '#lib/dapp/core/dperm-popup.ts';
import { readExtSnapshot } from '#lib/dapp/core/ext-cache.ts';
import { getGrant, getOriginChain, revokeGrant } from '#lib/dapp/grants.ts';
import { CHAIN_PREFIX, CHAINS_KEY, LAST_CHAIN_KEY } from '#lib/dapp/keys.ts';
import {
	dappRpcCallsStatus,
	dappRpcCallsStatusId,
	dappRpcCapabilities,
	dappRpcClassify,
	type DappRoute
} from '#lib/core/kernels.ts';
import { rpcCall, USER_OP_STATUS_METHOD } from '#lib/services/rpc-adapter.ts';
import type { ConnectRequest, WireAnswer, WireError } from './protocol';
import { findOp } from './ops';

/** What the host does with a request. */
export type Handling =
	/** Answered — send it. */
	| { kind: 'answer'; answer: WireAnswer }
	/** A person decides: the consent card or the signing sheet. */
	| { kind: 'person'; route: 'connect' | 'sign' };

/** `5730`, EIP-5792: a batch id this wallet does not know (for this site). */
export const UNKNOWN_BUNDLE_ID = 5730;

const err = (code: number, message: string): { kind: 'answer'; answer: WireAnswer } => ({
	kind: 'answer',
	answer: { error: { code, message } }
});
const ok = (result: unknown): { kind: 'answer'; answer: WireAnswer } => ({
	kind: 'answer',
	answer: { result }
});

/**
 * The accounts `origin` may see now: its grant, only while its account is
 * the one signed in (the core's connect verdict, asked without a person).
 */
export async function grantedAccounts(origin: string, signedIn: string | null): Promise<string[]> {
	const grant = await getGrant(origin);
	if (!grant) return [];
	const verdict = decidePopupRequest({
		method: 'eth_requestAccounts',
		grant: toWireGrant(grant),
		signedIn,
		pinnedAddress: null,
		origin
	});
	return verdict.outcome.type === 'respond' && verdict.outcome.payload.type === 'accounts'
		? verdict.outcome.payload.addresses
		: [];
}

/** The chain `origin` is on (its switch, its grant, else where a new site starts). */
export async function originChain(origin: string): Promise<number> {
	const snapshot = await readExtSnapshot();
	return getOriginChain(origin, snapshot?.chain_id ?? 0);
}

async function catalog(): Promise<unknown> {
	const all = (await dappArea()?.get(CHAINS_KEY)) ?? {};
	return all[CHAINS_KEY] ?? null;
}

/**
 * Route `request` from `origin` and answer it if nobody has to decide.
 * `signedIn` is the account the session settled on (`null`: nobody).
 */
export async function handle(
	request: ConnectRequest,
	origin: string,
	signedIn: string | null
): Promise<Handling> {
	const route: DappRoute = dappRpcClassify(request.method);
	switch (route.type) {
		case 'connect':
			return { kind: 'person', route: 'connect' };
		case 'sign':
			return { kind: 'person', route: 'sign' };
		case 'accounts':
			return ok(await grantedAccounts(origin, signedIn));
		case 'coinbase':
			return ok((await grantedAccounts(origin, signedIn))[0] ?? null);
		case 'permissions': {
			const granted = await grantedAccounts(origin, signedIn);
			return ok(granted.length ? [{ parentCapability: 'eth_accounts' }] : []);
		}
		case 'revoke_permissions':
			// The site disconnects itself: its grant goes, as in every browser.
			await revokeGrant(origin);
			return ok(null);
		case 'chain_id':
		case 'net_version': {
			const chainId = await originChain(origin);
			if (!(chainId > 0)) return err(4900, 'Vela has no network for this site yet');
			return ok(route.type === 'chain_id' ? toHexChainId(chainId) : String(chainId));
		}
		case 'switch_chain':
		case 'add_chain':
			return switchTo(request.params, origin);
		case 'watch_asset':
			// Tokens join through Vela's own trust rules, not because a page asked.
			return ok(false);
		case 'capabilities': {
			const chains = catalogChainIds(await catalog()) as number[];
			const answer = dappRpcCapabilities(
				request.params,
				await grantedAccounts(origin, signedIn),
				chains
			);
			return { kind: 'answer', answer: answer as WireAnswer };
		}
		case 'calls_status':
			return { kind: 'answer', answer: await callsStatus(request.params, origin) };
		case 'read':
			return err(
				4200,
				`Vela Connect does not read the chain (${request.method}); use your own RPC`
			);
		case 'unsupported':
		default:
			return err(
				4200,
				request.method === 'eth_sign'
					? 'eth_sign is not supported; use personal_sign or eth_signTypedData_v4'
					: `Vela does not support ${request.method}`
			);
	}
}

/**
 * `wallet_switchEthereumChain` / `wallet_addEthereumChain` for a chain Vela
 * has: the site's chain is written for it (no prompt — a site picks its own
 * network, as everywhere). Only a network of the space in front (spec 105):
 * 4902 otherwise, saying which.
 */
async function switchTo(params: unknown[], origin: string): Promise<Handling> {
	const chainId = switchChainParam(params) as number;
	if (!(chainId > 0)) return err(-32602, 'Expected [{ chainId }]');
	const area = dappArea();
	const all = (await area?.get([CHAINS_KEY, LAST_CHAIN_KEY])) ?? {};
	const table = all[CHAINS_KEY];
	if (!area || !table) return err(4900, 'Vela has not loaded its networks yet — try again');
	const refused = switchRefusal(table, chainId) as WireError | null;
	if (refused) return err(refused.code, refused.message);
	await area.set({ [CHAIN_PREFIX + origin]: chainId });
	const last = withLastChain(table, all[LAST_CHAIN_KEY], chainId);
	if (last) await area.set({ [LAST_CHAIN_KEY]: last });
	return ok(null);
}

/**
 * EIP-5792 `wallet_getCallsStatus` for a batch this site sent through Vela
 * Connect: the bundler's receipt of its operation; with none, whether the
 * relay refused it (400) or it is still on its way (100). A bundler that
 * cannot be asked is no news: pending, and the site asks again.
 */
async function callsStatus(params: unknown[], origin: string): Promise<WireAnswer> {
	const id = dappRpcCallsStatusId(params);
	if (!id) return { error: { code: -32602, message: 'Expected [id]' } };
	const op = findOp(id, origin);
	if (!op) return { error: { code: UNKNOWN_BUNDLE_ID, message: 'Unknown bundle id' } };
	const receipt = await bundlerRead('eth_getUserOperationReceipt', id, op.chainId);
	if (receipt) return { result: dappRpcCallsStatus(id, op.chainId, receipt, null) };
	const relay = await bundlerRead(USER_OP_STATUS_METHOD, id, op.chainId);
	return { result: dappRpcCallsStatus(id, op.chainId, null, relay) };
}

async function bundlerRead(method: string, id: string, chainId: number): Promise<unknown> {
	try {
		const response = await rpcCall(method, [id], chainId);
		return response && !response.error ? (response.result ?? null) : null;
	} catch {
		return null;
	}
}

export { parseChainId };
