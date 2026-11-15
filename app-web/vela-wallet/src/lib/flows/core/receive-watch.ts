/**
 * The `receive_watch` machine, wired for the web (spec 025 Phase 4, D12).
 *
 * Ported from the receive_watch arms of
 * src/services/wallet-state-core/executors.ts + session.ts @ c13e89d4. A
 * per-screen session bound to ONE address: created when the receive surface
 * opens, disposed when it closes. The core owns the polling cadence, the
 * baseline and the deposit verdict; this file fetches (visibility-gated —
 * `isAppActive` is the document being visible here), sleeps abortably, and
 * acknowledges the deposit signal (no haptics on web — the feed's row and
 * the balance's update are the visible acknowledgement).
 */

import { ReceiveWatchCore } from '#lib/core/client.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';
import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { ReceiveWatchEvent } from '#lib/core/generated/ReceiveWatchEvent.ts';
import type { ReceiveWatchOperation } from '#lib/core/generated/ReceiveWatchOperation.ts';
import type { ReceiveWatchShellResult } from '#lib/core/generated/ReceiveWatchShellResult.ts';
import type { ReceiveWatchView } from '#lib/core/generated/ReceiveWatchView.ts';
import type { TokenSnapshot } from '#lib/core/generated/TokenSnapshot.ts';
import type { SessionOptions } from '#lib/core/types.ts';
import { hapticSuccess, isAppActive } from '#lib/services/platform.ts';
import {
	tokenBalanceDouble,
	tokenChainId,
	tokenId,
	type APIToken
} from '#lib/services/tokens-model.ts';
import { fetchTokens } from '#lib/services/wallet-api.ts';

export type ReceiveWatchEffect = { id: number; operation: ReceiveWatchOperation };

export type ReceiveWatchSessionOptions = SessionOptions<ReceiveWatchView> & {
	/** The account whose balances the watcher polls. One session per account. */
	address: string;
	/** The deposit signal, for the shell to refresh what it shows. */
	onDeposit?: () => void;
};

function toSnapshot(token: APIToken): TokenSnapshot {
	return {
		id: tokenId(token),
		symbol: token.symbol,
		chain_id: tokenChainId(token),
		balance: tokenBalanceDouble(token),
		price_usd: token.priceUsd ?? null
	};
}

export function createReceiveWatchExecutor(address: string, onDeposit?: () => void) {
	return async function execute(
		effect: ReceiveWatchEffect,
		signal: AbortSignal
	): Promise<ReceiveWatchShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'fetch_tokens': {
				// Activity is checked BEFORE fetching — the `checkDeposit` early return.
				if (!isAppActive()) return { type: 'inactive' };
				const tokens = await fetchTokens(address, { forceRefresh: true });
				return { type: 'tokens_fetched', tokens: tokens.map(toSnapshot), now_ms: Date.now() };
			}
			case 'wait':
				await new Promise<void>((resolve) => {
					const id = setTimeout(resolve, operation.ms);
					signal.addEventListener('abort', () => clearTimeout(id), { once: true });
				});
				return { type: 'waited', now_ms: Date.now() };
			case 'signal_deposit':
				hapticSuccess();
				onDeposit?.();
				return { type: 'signalled' };
			default: {
				const never: never = operation;
				throw new Error(`unhandled receive_watch operation: ${JSON.stringify(never)}`);
			}
		}
	};
}

export function receiveWatchOperationFailure(effect: ReceiveWatchEffect): ReceiveWatchShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'fetch_tokens':
			return { type: 'fetch_failed', now_ms: Date.now() };
		case 'wait':
			return { type: 'waited', now_ms: Date.now() };
		case 'signal_deposit':
			return { type: 'signalled' };
		default: {
			const never: never = operation;
			throw new Error(`unhandled receive_watch operation: ${JSON.stringify(never)}`);
		}
	}
}

export type ReceiveWatchSession = EffectLoop<ReceiveWatchEvent>;

/** Callers `loadCore()` first; construction is synchronous once it is aboard. */
export function createReceiveWatchSession(
	options: ReceiveWatchSessionOptions
): ReceiveWatchSession {
	return createJsonWasmShell<
		ReceiveWatchView,
		ReceiveWatchEvent,
		ReceiveWatchEffect,
		ReceiveWatchShellResult
	>(new ReceiveWatchCore(), {
		onView: options.onView,
		execute: createReceiveWatchExecutor(options.address, options.onDeposit),
		toFailure: receiveWatchOperationFailure,
		onError: options.onError
	});
}
