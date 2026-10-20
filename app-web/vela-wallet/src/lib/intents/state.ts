/**
 * The wallet as an AI agent reads it (spec 103): ids and values, never the
 * screen's display text, so what it reads does not change with the language
 * the person reads in.
 *
 * Balance privacy holds here as on every money surface: while the person has
 * amounts hidden, this withholds them too and says so — a leak in one surface
 * defeats the mask everywhere (`privacy.rs`).
 */
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { ActivityEntry, PendingApproval } from './intents.svelte';
import type { World } from './world';

/** How many of the latest transactions the agent reads. */
export const RECENT_KEPT = 20;

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
	const session = world.session;
	const hidden = balance?.hidden === true;
	const book = world.contacts?.loaded ? world.contacts : null;
	return {
		signed_in: session?.has_wallet === true,
		page,
		accounts: (session?.accounts ?? []).map((row) => ({
			address: row.account.address,
			name: row.account.name,
			active: row.index === session?.active_index
		})),
		balances: balance && {
			hidden,
			total_usd: hidden ? null : balance.display_total_usd,
			partial: balance.balance_partial,
			tokens: balance.tokens
				.filter((token) => !token.spam)
				.map((token) => ({
					chain_id: token.chain_id,
					symbol: token.symbol,
					token_address: token.token_address,
					balance: hidden ? null : token.balance,
					price_usd: token.price_usd
				}))
		},
		recent_transactions: feed?.transactions.slice(0, RECENT_KEPT).map((tx) => ({
			id: tx.id,
			chain_id: tx.chain_id,
			kind: tx.kind,
			status: tx.status,
			from: tx.from,
			to: tx.to,
			to_name: tx.to_name,
			symbol: tx.symbol,
			value: hidden ? null : tx.value,
			tx_hash: tx.tx_hash,
			timestamp_ms: tx.timestamp
		})),
		contacts:
			book?.contacts.map((entry) => ({
				address: entry.address,
				name: entry.name,
				favorite: entry.favorite
			})) ?? null,
		groups:
			book?.groups.map((group) => ({
				id: group.id,
				name: group.name,
				members: group.members.map((member) => member.address)
			})) ?? null,
		preferences: {
			theme: world.preferences.theme,
			language: world.preferences.language,
			text_scale: world.preferences.textScale,
			number_format: world.preferences.numberFormat,
			date_format: world.preferences.dateFormat,
			time_format: world.preferences.timeFormat,
			analytics: world.preferences.analytics,
			currency: world.currency?.code ?? null,
			fee_speed: world.feeTier?.tier ?? null
		},
		/**
		 * The networks, and the services the wallet uses. A provider's API key
		 * is a credential: only whether one is set is read here.
		 */
		networks: world.networks && {
			chains: world.networks.networks.map((row) => ({
				chain_id: row.chain_id,
				name: row.display_name,
				native_symbol: row.native_symbol,
				added_by_person: row.is_custom,
				rpc_url: row.rpc_url,
				explorer_url: row.explorer_url
			})),
			providers: world.networks.providers.map((row) => ({
				provider: row.provider,
				has_key: row.has_key
			})),
			services: world.networks.endpoints.map((row) => ({
				field: row.field,
				url: row.value === '' ? row.default_value : row.value,
				is_default: row.value === '' || row.value === row.default_value
			}))
		},
		/** The transfers the wallet is following, newest first. */
		transfers: world.operations,
		/** The agent's own requests still waiting for the person. */
		waiting_for_person: approvals.map((pending) => ({ id: pending.id, action: pending.action })),
		recent_agent_actions: activity.slice(0, 5).map((entry) => ({
			action: entry.action,
			stage: entry.stage,
			outcome: entry.outcome
		}))
	};
}
