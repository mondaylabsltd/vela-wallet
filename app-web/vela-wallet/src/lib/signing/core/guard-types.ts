// Ported from src/services/wallet-state-core/guard-types.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * Platform-neutral types for the `approval_guard` core (spec 017,
 * `rust/crates/vela-core/src/app/approval_guard.rs`).
 *
 * Standalone for the reason `types.ts` states for itself: the native stub
 * (`guard-session.ts`) needs these declarations, and importing them from a
 * `.web` module would drag the web-only service graph into the native bundle.
 */

import type { GuardOperation } from '#lib/core/generated/GuardOperation.ts';
import type { GuardView } from '#lib/core/generated/GuardView.ts';
import type { SessionOptions } from '#lib/core/types.ts';

/** One request from the core, carrying the id it will be answered by. */
export type GuardEffect = { id: number; operation: GuardOperation };

export type ApprovalGuardSessionOptions = SessionOptions<GuardView>;
