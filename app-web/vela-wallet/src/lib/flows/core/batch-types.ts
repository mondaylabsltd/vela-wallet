// Ported from src/services/wallet-state-core/batch-import-types.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * Platform-neutral types for the `batch_import` core (spec 017, group G5).
 *
 * Separate from `batch-import-executor.web.ts` for the same reason
 * `types.ts` is separate from `executors.web.ts`: the native stub
 * (`batch-import-session.ts`) needs these declarations, and importing them
 * from a `.web` module would drag the web-only service graph (the file
 * pickers) into the native bundle — where the wasm cannot load at all.
 */

import type { BatchOperation } from '$lib/core/generated/BatchOperation';
import type { BatchView } from '$lib/core/generated/BatchView';
import type { SessionOptions } from '$lib/core/types';

/** One request from the core, carrying the id it will be answered by. */
export type BatchEffect = { id: number; operation: BatchOperation };

export type BatchSessionOptions = SessionOptions<BatchView>;
