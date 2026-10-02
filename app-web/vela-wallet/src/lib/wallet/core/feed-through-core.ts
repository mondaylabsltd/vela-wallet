/**
 * Stored records → the feed's view, through the REAL `activity_feed` core
 * (wasm) — for the node tests that check a row or a detail is the core's
 * words, not a fixture's. Test-only: nothing on a page imports it, and it
 * loads the wasm the way the build-time i18n does.
 */
import type { FeedEvent } from '$lib/core/generated/FeedEvent';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { toFeedRecord } from './feed-executor';

/** The view after the store loaded `records`, then each of `events` in turn. */
export async function feedViewThroughCore(
	records: LocalTransaction[],
	account: string,
	nowMs: number,
	events: FeedEvent[] = []
): Promise<FeedView> {
	await import('$lib/i18n/wasm-init.server');
	const { ActivityFeedCore } = await import('$lib/core/client');
	const core = new ActivityFeedCore();
	type Result = {
		view: FeedView;
		effects: { id: number; operation: { type: string; read_id?: number } }[];
	};
	try {
		const switched = JSON.parse(
			core.dispatch(JSON.stringify({ type: 'account_switched', address: account }))
		) as Result;
		const read = switched.effects.find((e) => e.operation.type === 'read_tx_store');
		if (!read) throw new Error('the feed asked for no store read');
		let view = (
			JSON.parse(
				core.resolve_effect(
					BigInt(read.id),
					JSON.stringify({
						type: 'store_loaded',
						records: records.map(toFeedRecord).filter((r) => r !== null),
						now_ms: nowMs,
						read_id: read.operation.read_id
					})
				)
			) as Result
		).view;
		for (const event of events) {
			view = (JSON.parse(core.dispatch(JSON.stringify(event))) as Result).view;
		}
		return view;
	} finally {
		core.free();
	}
}

/** The feed's items (date headers dropped) after the store loaded `records`. */
export async function feedItemsThroughCore(
	records: LocalTransaction[],
	account: string,
	nowMs: number
): Promise<FeedItem[]> {
	const view = await feedViewThroughCore(records, account, nowMs);
	return view.rows.flatMap((r) => (r.type === 'item' ? [r.item] : []));
}
