/**
 * Constructs the `fee_policy` core and wires it to the web shell (spec 026
 * Phase 4; `rust/crates/vela-core/src/app/fee_policy.rs`).
 *
 * Ported from src/services/wallet-state-core/fee-session.ts @ f9bcb278.
 * Callers `loadCore()` first; construction is synchronous once it is aboard.
 */
import { FeePolicyCore } from '#lib/core/client.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';
import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { FeeEvent } from '#lib/core/generated/FeeEvent.ts';
import type { FeeShellResult } from '#lib/core/generated/FeeShellResult.ts';
import type { FeeView } from '#lib/core/generated/FeeView.ts';
import { createFeeExecutor, feeOperationFailure } from './fee-executor';
import type { FeeEffect, FeeSessionOptions } from './fee-types';

export type FeeSession = EffectLoop<FeeEvent>;

export function createFeeSession(options: FeeSessionOptions): FeeSession {
	return createJsonWasmShell<FeeView, FeeEvent, FeeEffect, FeeShellResult>(new FeePolicyCore(), {
		onView: options.onView,
		execute: createFeeExecutor(options),
		toFailure: feeOperationFailure,
		onError: options.onError
	});
}
