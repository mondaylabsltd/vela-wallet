/**
 * Constructs the `clear_signing` core and wires it to the web shell (spec 026
 * Phase 5; `rust/crates/vela-core/src/app/clear_signing.rs`).
 *
 * Ported from src/services/wallet-state-core/clear-session.ts @ f9bcb278.
 *
 * ONE session per presented request, and one per batch leg: the machine
 * resolves one request at a time and supersedes anything in flight, which is
 * exactly what a sheet needs and exactly what a batch of legs must not share.
 * The descriptor / ERC-165 / decimals caches are per-session as a consequence;
 * the executor's in-flight coalescing is what keeps two legs touching the same
 * token from printing two rows that disagree.
 */
import { ClearSigningCore } from '#lib/core/client.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';
import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { ClearShellResult } from '#lib/core/generated/ClearShellResult.ts';
import type { ClearSigningEvent } from '#lib/core/generated/ClearSigningEvent.ts';
import type { ClearSigningView } from '#lib/core/generated/ClearSigningView.ts';
import { clearOperationFailure, executeClearOperation } from './clear-executor';
import type { ClearEffect, ClearSigningSessionOptions } from './clear-types';

export type ClearSigningSession = EffectLoop<ClearSigningEvent>;

export function createClearSigningSession(
	options: ClearSigningSessionOptions
): ClearSigningSession {
	return createJsonWasmShell<ClearSigningView, ClearSigningEvent, ClearEffect, ClearShellResult>(
		new ClearSigningCore(),
		{
			onView: options.onView,
			execute: (effect, signal) => executeClearOperation(effect, signal),
			toFailure: (effect) => clearOperationFailure(effect),
			onError: options.onError
		}
	);
}
