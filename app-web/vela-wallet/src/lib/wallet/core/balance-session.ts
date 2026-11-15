// Ported from src/services/wallet-state-core/balance-session.ts @ c13e89d4 (spec 025).
/**
 * Constructs the `balance_dashboard` core and wires it to the web shell — WEB
 * entry (spec 017, `rust/crates/vela-core/src/app/balance_dashboard.rs`).
 *
 * Importing `@/services/vela-core` first is load-bearing: its web entry runs
 * `initSync` on the fetched module at import time, so the wasm is initialised
 * before the core is constructed here.
 *
 * `balance-session.ts` is the native counterpart and throws.
 */

import { BalanceDashboardCore } from '#lib/core/client.ts';

import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';

import { createBalanceExecutor } from './balance-executor';
import type { BalanceEvent } from '#lib/core/generated/BalanceEvent.ts';
import type { BalanceShellResult } from '#lib/core/generated/BalanceShellResult.ts';
import type { BalanceView } from '#lib/core/generated/BalanceView.ts';
import type { BalanceEffect, BalanceSessionOptions } from './balance-types';

export type BalanceSession = EffectLoop<BalanceEvent>;

export function createBalanceSession(options: BalanceSessionOptions): BalanceSession {
	const executor = createBalanceExecutor(options.stream);
	return createJsonWasmShell<BalanceView, BalanceEvent, BalanceEffect, BalanceShellResult>(
		new BalanceDashboardCore(),
		{
			onView: options.onView,
			execute: (effect, signal) => executor.execute(effect, signal),
			toFailure: (effect, error) => executor.toFailure(effect, error),
			onError: options.onError
		}
	);
}
