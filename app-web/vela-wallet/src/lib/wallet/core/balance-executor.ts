// Ported from src/services/wallet-state-core/balance-executor.ts @ c13e89d4 (spec 025).
/**
 * The only place the `balance_dashboard` core touches the outside world.
 *
 * Seven operations, one existing service call each — the vocabulary the core
 * declares (`FetchTokens` / `FetchAccountAssets` / `ReadBalanceCache` /
 * `ReadBalanceCacheMany` / `WriteBalanceCache` / `StartRetryTimer` /
 * `WritePrivacy`). No branching on business meaning: the merge, the
 * `max(live, cached)` display rule, the complete-results-only write gate, the
 * silent-retry budget and the notice gate all live in Rust.
 *
 * What stays here (the core's module doc lists these as shell-owned): the
 * 5-minute token cache and the per-chain 18s cap inside `wallet-api.ts`, the
 * 24h TTL on the persisted total inside `balance-cache.ts` (it owns the clock,
 * so an expired entry answers `null` and the core reads that as "nothing
 * cached"), and every timer.
 *
 * Wire vs app shape: the core speaks `BalanceToken` (`chain_id`, `price_usd`),
 * the app holds `APIToken` (`network` slug, `chainName`, `logo`). The round trip
 * is lossless because `wallet-api.ts` derives `network`/`chainName` from the
 * chain id and always writes `logo: null` — so `toApiToken` reconstructs exactly
 * what `fetchTokens` produced, and the Assets tab renders the same objects it
 * always did.
 *
 * `FetchTokens` is the one operation that speaks twice: `onProgress` snapshots
 * are pushed through the injected {@link BalanceStreamSink}, and the operation
 * itself settles exactly once, echoing `address` and `pull`.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { setItem } from '$lib/services/storage';

import { chainName, networkId } from '$lib/services/networks';
import { tokenChainId, type APIToken } from '$lib/services/tokens-model';
import {
	getAccountBalance,
	getAccountBalances,
	setAccountBalance
} from '$lib/services/balance-cache';
// `@/services/rpc-pool` resolves to `rpc-pool.web.ts` on web, whose
// `getRateLimitedChains()` IS the `RpcPoolView` projection (G8 landed it): the
// classification snapshot the core asks for now comes from the rpc_pool machine,
// with the dev fault-injection sets folded in exactly as before.
import { getRateLimitedChains } from '$lib/services/rpc-pool';
import { fetchTokens } from '$lib/services/wallet-api';

import type { BalanceShellResult } from '$lib/core/generated/BalanceShellResult';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import { BALANCE_PRIVACY_KEY } from './balance-types';
import type { BalanceEffect, BalanceStreamSink } from './balance-types';

/** An `APIToken` in the core's vocabulary. */
export function toBalanceToken(token: APIToken): BalanceToken {
	return {
		chain_id: tokenChainId(token),
		symbol: token.symbol,
		name: token.name,
		balance: token.balance,
		decimals: token.decimals,
		token_address: token.tokenAddress,
		price_usd: token.priceUsd,
		spam: token.spam
	};
}

/**
 * The inverse — the shape `HoldingsList`, `BalanceDetailSheet` and the
 * token-detail route consume. `network` and `chainName` are derived from the
 * chain id (`wallet-api.ts:430-431` derives them the same way) and `logo` is
 * always `null` there (`wallet-api.ts:488`), so nothing is invented and nothing
 * is lost.
 */
export function toApiToken(token: BalanceToken): APIToken {
	return {
		network: networkId(token.chain_id),
		chainName: chainName(token.chain_id),
		symbol: token.symbol,
		balance: token.balance,
		decimals: token.decimals,
		logo: null,
		name: token.name,
		tokenAddress: token.token_address,
		priceUsd: token.price_usd,
		spam: token.spam
	};
}

/** A cancellable sleep — the core's `StartRetryTimer`, nothing more. */
function delay(ms: number, signal: AbortSignal): Promise<void> {
	return new Promise<void>((resolve) => {
		const timer = setTimeout(resolve, ms);
		signal.addEventListener(
			'abort',
			() => {
				clearTimeout(timer);
				resolve();
			},
			{ once: true }
		);
	});
}

export function createBalanceExecutor(stream: BalanceStreamSink) {
	async function execute(effect: BalanceEffect, signal: AbortSignal): Promise<BalanceShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'fetch_tokens': {
				// `failed` is filled by the callback and read at settle — the exact
				// shape `useHomeController.ts:321-333` used, so a cache-served fetch
				// (the callback never fires) still reports an empty set.
				let failed: number[] = [];
				const tokens = await fetchTokens(operation.address, {
					forceRefresh: operation.force,
					onProgress: (partial) =>
						stream.chainAssetsArrived(operation.address, partial.map(toBalanceToken)),
					onFailedChains: (ids) => {
						failed = ids;
					}
				});
				stream.roundEnded?.(operation.address);
				return {
					type: 'fetch_settled',
					address: operation.address,
					pull: operation.pull,
					tokens: tokens.map(toBalanceToken),
					failed_chain_ids: failed,
					rate_limited_chain_ids: [...getRateLimitedChains()],
					now_ms: Date.now()
				};
			}
			case 'fetch_account_assets': {
				// The switcher row: TTL-cached, never forced, never streaming
				// (`useHomeController.ts:459`).
				const tokens = await fetchTokens(operation.address);
				return {
					type: 'account_assets_fetched',
					address: operation.address,
					tokens: tokens.map(toBalanceToken)
				};
			}
			case 'read_balance_cache':
				// `getAccountBalance` applies the 24h TTL and answers `null` for a
				// missing OR expired entry — the core reads both as "nothing cached".
				return {
					type: 'cached_total_loaded',
					address: operation.address,
					usd: await getAccountBalance(operation.address)
				};
			case 'read_balance_cache_many': {
				const balances = await getAccountBalances(operation.addresses);
				return {
					type: 'cached_balances_loaded',
					// Keyed by the address strings the core asked for, so the core's
					// upsert of the active row lands on the same key the roster carries.
					balances: [...balances].map(([address, usd]) => ({ address, usd }))
				};
			}
			case 'write_balance_cache':
				await setAccountBalance(operation.address, operation.usd);
				return { type: 'balance_cache_written' };
			case 'start_retry_timer':
				await delay(operation.ms, signal);
				return { type: 'retry_elapsed', timer_id: operation.timer_id };
			case 'write_privacy':
				// Straight to the byte. The other masking surfaces (HoldingsList,
				// BalanceDetailSheet, AccountSwitcherModal) read `hidden` off the core's
				// own view through `use-balance-privacy.web.ts`, so there is no second
				// in-memory flag to keep in step — and no second hydrate race.
				await setItem(BALANCE_PRIVACY_KEY, operation.hidden ? '1' : '0');
				return { type: 'privacy_written' };
		}
	}

	function toFailure(effect: BalanceEffect, error: unknown): BalanceShellResult {
		void error; // classification is the core's; the twin answers by operation
		const operation = effect.operation;
		switch (operation.type) {
			case 'fetch_tokens':
				// `catch { /* keep last-known tokens + total */ }` — the skeleton
				// closes, nothing else moves (`useHomeController.ts:367-369`).
				stream.roundEnded?.(operation.address);
				return { type: 'fetch_errored', address: operation.address, pull: operation.pull };
			case 'fetch_account_assets':
				// Per-account best effort: the row keeps its cached value
				// (`useHomeController.ts:463`).
				return { type: 'account_assets_fetched', address: operation.address, tokens: null };
			case 'read_balance_cache':
				// `getAccountBalance` swallows its own read errors; this covers the rest.
				return { type: 'cached_total_loaded', address: operation.address, usd: null };
			case 'read_balance_cache_many':
				return { type: 'cached_balances_loaded', balances: [] };
			case 'write_balance_cache':
				// Best effort, exactly like today's un-awaited `setAccountBalance`.
				return { type: 'balance_cache_written' };
			case 'start_retry_timer':
				// A sleep cannot fail; answering keeps the core from waiting forever.
				return { type: 'retry_elapsed', timer_id: operation.timer_id };
			case 'write_privacy':
				// `setItem(...).catch(() => {})` — the in-memory flag stands.
				return { type: 'privacy_written' };
		}
	}

	return { execute, toFailure };
}
