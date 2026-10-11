/**
 * Constructs the `sign_request` core and wires it to the web shell (spec 026
 * Phase 5; `rust/crates/vela-core/src/app/sign_request.rs`).
 *
 * Ported from src/services/wallet-state-core/sign-session.ts @ f9bcb278.
 * Callers `loadCore()` first; the resident is the only constructor.
 */
import { SignRequestCore } from '$lib/core/client';
import type { EffectLoop } from '$lib/core/effect-loop';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { SignShellResult } from '$lib/core/generated/SignShellResult';
import type { SignView } from '$lib/core/generated/SignView';
import { createSignExecutor } from './sign-executor';
import type { SignEffect, SignRequestSessionOptions } from './sign-types';

export type SignRequestSession = EffectLoop<SignEvent>;

export function createSignRequestSession(options: SignRequestSessionOptions): SignRequestSession {
	const executor = createSignExecutor(options.ports, options.timer);
	return createJsonWasmShell<SignView, SignEvent, SignEffect, SignShellResult>(
		new SignRequestCore(),
		{
			onView: options.onView,
			execute: executor.execute,
			toFailure: executor.toFailure,
			onError: options.onError
		}
	);
}
