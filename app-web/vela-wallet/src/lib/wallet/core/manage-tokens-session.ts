// Ported from src/services/wallet-state-core/manage-tokens-session.ts @ c13e89d4 (spec 025).
/**
 * Constructs the `manage_tokens` core and wires it to the web shell — WEB
 * entry (spec 017, `rust/crates/vela-core/src/app/manage_tokens.rs`).
 *
 * Importing `@/services/vela-core` first is load-bearing: its web entry runs
 * `initSync` on the base64-embedded module at import time, so the wasm is
 * initialised before the core is constructed here.
 *
 * `manage-tokens-session.ts` is the native counterpart and throws.
 */

import { ManageTokensCore } from '#lib/core/client.ts';

import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';

import { createManageTokensExecutor, manageTokensOperationFailure } from './manage-tokens-executor';
import type { MtokEvent } from '#lib/core/generated/MtokEvent.ts';
import type { MtokShellResult } from '#lib/core/generated/MtokShellResult.ts';
import type { MtokView } from '#lib/core/generated/MtokView.ts';
import type { ManageTokensEffect, ManageTokensSessionOptions } from './manage-tokens-types';

export type ManageTokensSession = EffectLoop<MtokEvent>;

export function createManageTokensSession(
	options: ManageTokensSessionOptions
): ManageTokensSession {
	return createJsonWasmShell<MtokView, MtokEvent, ManageTokensEffect, MtokShellResult>(
		new ManageTokensCore(),
		{
			onView: options.onView,
			execute: createManageTokensExecutor(options),
			toFailure: manageTokensOperationFailure,
			onError: options.onError
		}
	);
}
