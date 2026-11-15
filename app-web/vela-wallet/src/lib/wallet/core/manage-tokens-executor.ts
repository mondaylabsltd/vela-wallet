// Ported from src/services/wallet-state-core/manage-tokens-executor.ts @ c13e89d4 (spec 025).
/**
 * The only place the `manage_tokens` core touches the outside world (spec 017,
 * group G2).
 *
 * Each operation maps to exactly one existing service call — the ERC-20 probe,
 * the three `vela.customTokens` accessors, the fetchTokens cache. No branching
 * on business meaning: if this file ever grows an `if` that decides what
 * happens next, that decision belongs in the Rust machine.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with, so the core
 * keeps ownership of classification.
 */

import { fetchErc20Meta } from '#lib/services/erc20-meta.ts';
import { hapticSuccess } from '#lib/services/platform.ts';
import { loadCustomTokens, removeCustomToken, saveCustomToken } from '#lib/services/records.ts';
import { clearTokenCache } from '#lib/services/wallet-api.ts';
import type { CustomToken } from '#lib/services/tokens-model.ts';

import type { MtokCustomToken } from '#lib/core/generated/MtokCustomToken.ts';
import type { MtokShellResult } from '#lib/core/generated/MtokShellResult.ts';
import type { ManageTokensEffect, ManageTokensSessionOptions } from './manage-tokens-types';

/**
 * Wire-representability, not policy. The core's `decimals` is `u8` and its
 * `chain_id` is `u32`; a value outside those ranges cannot be serialised into
 * the core at all, and pushing one anyway makes `resolve_effect` throw — which
 * would leave the effect permanently unanswered and the panel wedged
 * (`detecting` true forever). So a non-representable number is reported as
 * "nothing resolved" rather than as a fault. `decU8` is `Number(decU256(...))`,
 * so a contract whose `decimals()` returns a bignum lands here.
 */
function asU8(value: number): number | null {
	return Number.isInteger(value) && value >= 0 && value <= 255 ? value : null;
}

function asU32(value: number): number | null {
	return Number.isInteger(value) && value >= 0 && value <= 4_294_967_295 ? value : null;
}

/**
 * A stored token as the core reads it. A row whose numbers don't fit the wire
 * is dropped — the same silence the TS loader's `catch {}` gives a corrupt
 * `vela.customTokens` file.
 */
function toWireToken(token: CustomToken): MtokCustomToken | null {
	const chainId = asU32(token.chainId);
	const decimals = asU8(token.decimals);
	if (chainId === null || decimals === null) return null;
	return {
		id: token.id,
		chain_id: chainId,
		contract_address: token.contractAddress,
		symbol: token.symbol,
		name: token.name,
		decimals,
		network_name: token.networkName
	};
}

/** The core's token, in the shape `services/storage.ts` persists. */
function fromWireToken(token: MtokCustomToken): CustomToken {
	return {
		id: token.id,
		chainId: token.chain_id,
		contractAddress: token.contract_address,
		symbol: token.symbol,
		name: token.name,
		decimals: token.decimals,
		networkName: token.network_name
	};
}

export function createManageTokensExecutor(
	options: Pick<ManageTokensSessionOptions, 'account' | 'onInvalidated'>
) {
	return async function execute(effect: ManageTokensEffect): Promise<MtokShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'multicall_erc20_meta': {
				const meta = await fetchErc20Meta(operation.chain_id, operation.address);
				const decimals = meta ? asU8(meta.decimals) : null;
				return {
					type: 'chain_meta_resolved',
					chain_id: operation.chain_id,
					// Echoed verbatim: this is the core's staleness key.
					address: operation.address,
					meta:
						meta && decimals !== null ? { name: meta.name, symbol: meta.symbol, decimals } : null
				};
			}
			case 'read_custom_tokens': {
				const tokens = await loadCustomTokens();
				return {
					type: 'custom_tokens_loaded',
					tokens: tokens
						.map(toWireToken)
						.filter((token): token is MtokCustomToken => token !== null)
				};
			}
			case 'write_custom_token':
				await saveCustomToken(fromWireToken(operation.token));
				// The success buzz stays in the shell, fired exactly where
				// `handleSave` fires it today: after the write resolves.
				hapticSuccess();
				return { type: 'saved' };
			case 'remove_custom_token':
				await removeCustomToken(operation.id);
				hapticSuccess();
				// The id rides back so the core can correlate the row.
				return { type: 'removed', id: operation.id };
			case 'invalidate_token_cache':
				// `fetchTokens` is keyed by wallet address; the core doesn't hold one.
				clearTokenCache(options.account());
				// Today's host refresh (`onChanged`) — same moment, core's orders.
				options.onInvalidated?.();
				return { type: 'cache_invalidated' };
		}
	};
}

export function manageTokensOperationFailure(
	effect: ManageTokensEffect,
	error: unknown
): MtokShellResult {
	void error; // classification is the core's; the twin answers by operation
	const operation = effect.operation;
	switch (operation.type) {
		case 'multicall_erc20_meta':
			// The rejected `Promise.allSettled` arm — fail-closed, no card.
			return {
				type: 'chain_meta_resolved',
				chain_id: operation.chain_id,
				address: operation.address,
				meta: null
			};
		case 'read_custom_tokens':
			// Unreadable storage answers as empty, as the TS loader's `catch {}` does.
			return { type: 'custom_tokens_loaded', tokens: [] };
		case 'write_custom_token':
			return { type: 'save_failed' };
		case 'remove_custom_token':
			return { type: 'remove_failed', id: operation.id };
		case 'invalidate_token_cache':
			return { type: 'cache_invalidated' };
	}
}
