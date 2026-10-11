// Ported from src/services/sim-trust.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The simulation preview's asymmetric-trust judgment — WEB, decided by the
 * `token_trust` core (spec 017, `token_trust.rs::judge_delta`).
 *
 * SECURITY BOUNDARY. The deltas passed here come from a sign-time
 * `eth_simulateV1`/Tevm run and are attacker-influenceable by construction: a
 * hostile dApp can synthesize a `Transfer(_, you, big)` and answer `symbol()`.
 * They are dispatched as `sim_deltas_computed`, which in the core can reach a
 * `WriteCustomToken` through no code path at all — the render-only half of the
 * machine. The admission (auto-add) half is reachable ONLY from
 * `receipt_logs_confirmed`. Never route a simulation through that event.
 *
 * The two facts the core needs to judge a *received* amount are pushed first,
 * from the same two reads `trustedReceiveSet` does today: the chain registry's
 * stables + wrapped native, and the tokens this account is cached as holding.
 * Both are best-effort — missing facts shrink the trusted set, which pushes
 * received tokens to `unverified`, the safe direction.
 */

import { nativeSymbol } from '../networks';
import { getCachedHeldTokens } from '../wallet-api';
import {
	judgeSimDeltas,
	notifyHeldTokens,
	primeRegistry
} from '$lib/wallet/core/token-trust-resident';
import type { AssetDelta } from './sim-assets';
import type { AssetChange } from './tx-simulation';
import type { TrustAssetDelta } from '$lib/core/generated/TrustAssetDelta';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';

/** Native coins use 18 decimals on every Vela-supported chain. */
const NATIVE_DECIMALS = 18;

function toWireDelta(delta: AssetDelta): TrustAssetDelta {
	return {
		kind: delta.kind,
		token: delta.token ?? null,
		delta: delta.delta.toString()
	};
}

/**
 * One judgment back into the shape `BalanceChangePreview` renders — or `null`
 * for the one judgment that is no row. The symbol and decimals of the NATIVE
 * row are the shell's vocabulary, exactly as `enrichDeltas` composed them;
 * everything trust-related came from the core.
 *
 * PR 3: an unverified token is a DIRECTION, never a figure. Its number is
 * whatever the site being signed for chose to emit from a contract it
 * controls (Android printed "+5,000,000,000,000,000,000,000.00" of one), so
 * the core no longer hands it over and this change carries none: `in` and
 * `out` are a row with a sign, `unreadable` a row with neither sign nor
 * figure, and `still` — a zero — is not a row at all.
 */
function toAssetChange(judgment: TrustSimJudgment, chainId: number): AssetChange | null {
	switch (judgment.type) {
		case 'native':
			return {
				kind: 'native',
				delta: BigInt(judgment.delta),
				symbol: nativeSymbol(chainId),
				decimals: NATIVE_DECIMALS
			};
		case 'erc20_trusted':
			return {
				kind: 'erc20',
				token: judgment.token,
				delta: BigInt(judgment.delta),
				symbol: judgment.symbol,
				decimals: judgment.decimals
			};
		case 'erc20_unverified':
			if (judgment.direction === 'still') return null;
			return {
				kind: 'erc20',
				token: judgment.token ?? undefined,
				direction: judgment.direction,
				unverified: true
			};
	}
}

/**
 * Judge and enrich one simulation's deltas. Order-preserving: the core answers
 * one judgment per delta, in the order they were given — less an unverified
 * token that did not move, which is no row.
 */
export async function enrichDeltas(
	deltas: AssetDelta[],
	chainId: number,
	from: string
): Promise<AssetChange[]> {
	const hasErc20 = deltas.some((d) => d.kind === 'erc20' && d.token);
	// The same gate `enrichDeltas` applies to the trusted-set fetch today: a
	// preview with no ERC-20 inflow never needed the registry, and this keeps
	// the signing path from gaining a network read it did not have.
	const hasReceive = deltas.some((d) => d.kind === 'erc20' && d.delta > 0n);
	if (hasErc20 && hasReceive) {
		await primeRegistry([chainId]);
		notifyHeldTokens(from, chainId, getCachedHeldTokens(from, chainId));
	}
	const judgments = await judgeSimDeltas(from, chainId, deltas.map(toWireDelta));
	return judgments.flatMap((judgment) => {
		const change = toAssetChange(judgment, chainId);
		return change === null ? [] : [change];
	});
}
