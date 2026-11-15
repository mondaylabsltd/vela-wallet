// Ported from src/services/wallet-state-core/guard-executor.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The only place the `approval_guard` core touches the outside world (spec
 * 017, audit items ⑪–⑳).
 *
 * Three RPC reads, each one existing service call, none of them deciding
 * anything: the Multicall3 token metadata the cap editor scales amounts with,
 * the on-chain `allowance` behind the increaseAllowance resulting total, and
 * the `balanceOf` behind the one-tap finite Balance cap. Every classification
 * — "the whole read failed" vs "this token was not resolvable", which of the
 * two symbol fallbacks applies, whether a Balance chip is offered at all —
 * belongs to the machine and is asserted there.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with, so the core
 * keeps ownership of classification.
 */

import { readErc20Allowance, readErc20Balance } from '#lib/services/token-reads.ts';
import { resolveTokenMetadata } from '#lib/services/token-metadata.ts';

import type { GuardShellResult } from '#lib/core/generated/GuardShellResult.ts';
import type { GuardTokenMetaEntry } from '#lib/core/generated/GuardTokenMetaEntry.ts';
import type { GuardEffect } from './guard-types';

/**
 * Wire-representability, not policy. `decimals` is a `u32` in the core; a
 * value that cannot be serialised would make `resolve_effect` throw and leave
 * the effect permanently unanswered (the sheet stuck on `…`/loading). Such a
 * token is reported as simply unresolved, which is the fallback path the core
 * already owns.
 */
function asU32(value: number): number | null {
	return Number.isInteger(value) && value >= 0 && value <= 4_294_967_295 ? value : null;
}

export function createGuardExecutor() {
	return async function execute(effect: GuardEffect): Promise<GuardShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'read_token_metadata': {
				// `resolveTokenMetadata` never throws and keys its map by lowercased
				// address; a token it could not resolve is simply absent, which is
				// exactly the "missing from Some(list)" case the core distinguishes
				// from a whole-read failure (see `toFailure`).
				const map = await resolveTokenMetadata(operation.chain_id, operation.tokens);
				const metas: GuardTokenMetaEntry[] = [];
				for (const token of operation.tokens) {
					const meta = map.get(token.toLowerCase());
					if (!meta) continue;
					const decimals = asU32(meta.decimals);
					if (decimals === null) continue;
					metas.push({ token: token.toLowerCase(), symbol: meta.symbol, decimals });
				}
				return { type: 'meta_resolved', metas };
			}
			case 'read_erc20_allowance': {
				const allowance = await readErc20Allowance(
					operation.chain_id,
					operation.token,
					operation.owner,
					operation.spender
				);
				return {
					type: 'allowance_read',
					allowance: allowance === null ? null : allowance.toString()
				};
			}
			case 'read_erc20_balance': {
				const balance = await readErc20Balance(
					operation.chain_id,
					operation.token,
					operation.owner
				);
				return { type: 'balance_read', balance: balance === null ? null : balance.toString() };
			}
		}
	};
}

export function guardOperationFailure(effect: GuardEffect): GuardShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'read_token_metadata':
			// The rejected arm of today's `.catch()` — `metas: null` is "the WHOLE
			// read failed", which the core answers with the short-address fallback
			// for a single approval and the …/18/unverified defaults for a batch.
			return { type: 'meta_resolved', metas: null };
		case 'read_erc20_allowance':
			// The resulting-total row still warns the increment ADDS to an existing
			// allowance rather than hiding — that decision is the core's.
			return { type: 'allowance_read', allowance: null };
		case 'read_erc20_balance':
			return { type: 'balance_read', balance: null };
	}
}
