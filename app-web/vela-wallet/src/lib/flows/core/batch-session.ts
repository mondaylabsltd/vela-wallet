/**
 * Constructs the `batch_import` core and wires it to the web shell (spec 026
 * Phase 6; `rust/crates/vela-core/src/app/batch_import.rs`).
 *
 * Ported from src/services/wallet-state-core/batch-import-session.ts @
 * f9bcb278. One session per open importer sheet; callers `loadCore()` first.
 */
import { BatchImportCore } from '#lib/core/client.ts';
import type { EffectLoop } from '#lib/core/effect-loop.ts';
import { createJsonWasmShell } from '#lib/core/json-shell.ts';
import type { BatchImportEvent } from '#lib/core/generated/BatchImportEvent.ts';
import type { BatchShellResult } from '#lib/core/generated/BatchShellResult.ts';
import type { BatchView } from '#lib/core/generated/BatchView.ts';
import { batchOperationFailure, executeBatchOperation } from './batch-executor';
import type { BatchEffect, BatchSessionOptions } from './batch-types';

export type BatchImportSession = EffectLoop<BatchImportEvent>;

export function createBatchImportSession(options: BatchSessionOptions): BatchImportSession {
	return createJsonWasmShell<BatchView, BatchImportEvent, BatchEffect, BatchShellResult>(
		new BatchImportCore(),
		{
			onView: options.onView,
			execute: (effect) => executeBatchOperation(effect),
			toFailure: (effect) => batchOperationFailure(effect),
			onError: options.onError
		}
	);
}
