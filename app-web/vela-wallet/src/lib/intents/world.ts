/**
 * What the intents read (spec 103): the views the app's own machines already
 * hold, gathered at the moment of asking. Nothing here is a copy kept in sync
 * — each call reads the residents as they are.
 */
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { ContactsView } from '$lib/core/generated/ContactsView';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeeTierPrefView } from '$lib/core/generated/FeeTierPrefView';
import type { IntentOperation } from '$lib/core/generated/IntentOperation';
import type { NetView } from '$lib/core/generated/NetView';
import type { SessionView } from '$lib/session/generated/SessionView';

export type World = {
	session: SessionView | null;
	contacts: ContactsView | null;
	currency: CurrencyView | null;
	feeTier: FeeTierPrefView | null;
	/** The preferences the shell keeps itself, in the shared spellings. */
	preferences: {
		theme: string;
		language: string;
		textScale: string;
		numberFormat: string;
		dateFormat: string;
		timeFormat: string;
		analytics: boolean;
	};
	agent: { enabled: boolean };
	/** The transfers the wallet is following (`transfers.svelte.ts`), up to date. */
	operations: IntentOperation[];
	balance: BalanceView | null;
	feed: FeedView | null;
	/** The network ledger, once it has read its stores. */
	networks: NetView | null;
	/** The space in front (spec 105). */
	networkSpace: 'mainnet' | 'testnet';
};

/** The world as the core's judgements read it (`intent_rules::World`): its spellings. */
function coreWorld(world: World): Record<string, unknown> {
	return {
		session: world.session,
		contacts: world.contacts,
		balance: world.balance,
		feed: world.feed,
		networks: world.networks,
		currency: world.currency,
		fee_tier: world.feeTier,
		operations: world.operations,
		preferences: {
			theme: world.preferences.theme,
			language: world.preferences.language,
			text_scale: world.preferences.textScale,
			number_format: world.preferences.numberFormat,
			date_format: world.preferences.dateFormat,
			time_format: world.preferences.timeFormat,
			analytics: world.preferences.analytics
		},
		agent_enabled: world.agent.enabled,
		network_space: world.networkSpace
	};
}

/**
 * The world as JSON for the core — only the views in `needs` when given
 * (`intentWorldNeeds`): a judgement polled while a change lands should not
 * serialize the whole wallet, the feed least of all.
 */
export function worldJson(world: World, needs?: readonly string[]): string {
	const all = coreWorld(world);
	if (!needs) return JSON.stringify(all);
	return JSON.stringify(Object.fromEntries(needs.map((key) => [key, all[key]])));
}
