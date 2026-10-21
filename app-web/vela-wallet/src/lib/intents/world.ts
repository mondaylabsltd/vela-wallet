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
};
