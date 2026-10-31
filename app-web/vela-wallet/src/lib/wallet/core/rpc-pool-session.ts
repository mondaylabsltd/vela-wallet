/**
 * Constructs the `rpc_pool` core and wires it to the web shell (spec 025).
 * Callers `loadCore()` first (the facade's ensureReady does) — construction
 * is synchronous once the wasm is aboard.
 */

import { RpcPoolCore } from '$lib/core/client';
import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';
import type { RpcEvent } from '$lib/core/generated/RpcEvent';
import type { RpcShellResult } from '$lib/core/generated/RpcShellResult';
import type { RpcPoolView } from '$lib/core/generated/RpcPoolView';
import { createRpcPoolExecutor } from './rpc-pool-executor';
import type { RpcPoolEffect, RpcPoolSessionOptions } from './rpc-pool-types';

export type RpcPoolSession = EffectLoop<RpcEvent>;

export function createRpcPoolSession(options: RpcPoolSessionOptions): RpcPoolSession {
	const executor = createRpcPoolExecutor(options.registry);
	return createJsonWasmShell<RpcPoolView, RpcEvent, RpcPoolEffect, RpcShellResult>(
		new RpcPoolCore(),
		{
			onView: options.onView,
			execute: (effect) => executor.execute(effect),
			toFailure: (effect, error) => executor.toFailure(effect, error),
			onError: options.onError
		},
		// The hedge reads `pending_urls`: it comes apart, as it moves.
		{ skipUnchangedViews: { includeVolatile: true } }
	);
}
