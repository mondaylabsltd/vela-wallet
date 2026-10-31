// Ported from src/services/wallet-state-core/feed-session.ts @ c13e89d4 (spec 025).
/**
 * Constructs the `activity_feed` core and wires it to the web shell — WEB entry
 * (spec 017, `rust/crates/vela-core/src/app/activity_feed.rs`).
 *
 * Importing `@/services/vela-core` first is load-bearing: its web entry runs
 * `initSync` on the fetched module at import time, so the wasm is initialised
 * before the core is constructed here.
 *
 * `feed-session.ts` is the native counterpart and throws.
 */

import { ActivityFeedCore } from '$lib/core/client';

import { createJsonWasmShell } from '$lib/core/json-shell';
import type { EffectLoop } from '$lib/core/effect-loop';

import { createFeedExecutor } from './feed-executor';
import type { FeedEvent } from '$lib/core/generated/FeedEvent';
import type { FeedShellResult } from '$lib/core/generated/FeedShellResult';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { FeedEffect, FeedSessionOptions } from './feed-types';

export type ActivityFeedSession = EffectLoop<FeedEvent>;

export function createActivityFeedSession(options: FeedSessionOptions): ActivityFeedSession {
	const executor = createFeedExecutor(options.ownAccounts, options.records);
	return createJsonWasmShell<FeedView, FeedEvent, FeedEffect, FeedShellResult>(
		new ActivityFeedCore(),
		{
			onView: options.onView,
			execute: (effect, signal) => executor.execute(effect, signal),
			toFailure: (effect, error) => executor.toFailure(effect, error),
			onError: options.onError
		},
		// 200 rows were re-sent, parsed and redrawn for every name answer.
		{ skipUnchangedViews: { includeVolatile: false } }
	);
}
