/**
 * Constructs the `send` core and wires it to the web shell (spec 026 Phase 4;
 * `rust/crates/vela-core/src/app/send.rs`).
 *
 * Ported from src/services/wallet-state-core/send-session.ts @ f9bcb278.
 * Callers `loadCore()` first; construction is synchronous once it is aboard.
 */
import { SendCore } from '$lib/core/client';
import type { EffectLoop } from '$lib/core/effect-loop';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { SendEvent } from '$lib/core/generated/SendEvent';
import type { SendShellResult } from '$lib/core/generated/SendShellResult';
import type { SendView } from '$lib/core/generated/SendView';
import { createSendExecutor } from './send-executor';
import type { SendEffect, SendSessionOptions } from './send-types';

export type SendSession = EffectLoop<SendEvent>;

export function createSendSession(options: SendSessionOptions): SendSession {
	// The executor dispatches one fact into its own session mid-submit: the
	// op is signed and hashed and nothing has been POSTed (`OpSigned`, spec
	// 082 RJ1) — the core then writes the records and clears the POST.
	let session: SendSession | null = null;
	const executor = createSendExecutor(options.ports, {
		dispatch: (event) => session?.dispatch(event)
	});
	session = createJsonWasmShell<SendView, SendEvent, SendEffect, SendShellResult>(new SendCore(), {
		onView: options.onView,
		execute: executor.execute,
		toFailure: executor.toFailure,
		onError: options.onError
	});
	return session;
}
