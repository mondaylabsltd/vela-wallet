/**
 * The wallet as an AI agent reads it (spec 103): ids and values, never the
 * screen's display text. Balance privacy holds, and a provider's key is only
 * "set or not" — decided in the core (`intent_rules::agent_state`), the same
 * for every shell.
 */
import { intentAgentState } from '$lib/core/client';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { ActivityEntry, PendingApproval } from './intents.svelte';
import { worldJson, type World } from './world';

export type AgentStateInput = {
	world: World;
	balance: BalanceView | null;
	feed: FeedView | null;
	/** The page the person is on, by its route: `wallet`, `contacts`, `settings`, … */
	page: string;
	approvals: PendingApproval[];
	activity: ActivityEntry[];
};

export function agentState({ world, balance, feed, page, approvals, activity }: AgentStateInput) {
	return JSON.parse(
		intentAgentState(
			worldJson({ ...world, balance, feed }),
			page,
			JSON.stringify(approvals.map((pending) => ({ id: pending.id, action: pending.action }))),
			JSON.stringify(
				activity.slice(0, 5).map((entry) => ({
					action: entry.action,
					stage: entry.stage,
					outcome: entry.outcome
				}))
			)
		)
	) as Record<string, unknown>;
}
