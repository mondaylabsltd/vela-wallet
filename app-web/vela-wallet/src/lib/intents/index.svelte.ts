/**
 * The app's one `intents` (spec 103), over the live residents: the person's
 * UI and the AI agent both ask through it.
 */
import { analyticsConsent } from '$lib/analytics/consent.svelte';
import { networkSpace } from '$lib/network-space/network-space.svelte';
import { agentSettings } from '$lib/agent/settings.svelte';
import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
import { intentCheck, loadCore } from '$lib/core/client';
import type { IntentCheck } from '$lib/core/generated/IntentCheck';
import { preferences } from '$lib/services/preferences.svelte';
import { session } from '$lib/session/core/session.svelte';
import { currency } from '$lib/settings/core/currency.svelte';
import { feeTierPreference } from '$lib/settings/core/fee-tier.svelte';
import { Intents } from './intents.svelte';
import { perform, show } from './perform';
import { transfers } from './transfers.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import { feed } from '$lib/wallet/core/feed.svelte';
import { networkAdmin } from '$lib/settings/core/network-admin.svelte';
import type { World } from './world';
import { shownAddress } from '$lib/multisig/core/shown';
import { multisigAsAccount } from '$lib/multisig/core/ext';
import { proposals } from '$lib/multisig/core/proposals.svelte';
import { multisig } from '$lib/multisig/core/store.svelte';

export type { IntentOutcome } from './intents.svelte';

/** The residents' views, now: what the intents read, and what the agent's state is drawn from. */
export function world(): World {
	return {
		session: session.view,
		contacts: contactsBook.view,
		currency: currency.view,
		feeTier: feeTierPreference.view,
		preferences: {
			theme: preferences.theme,
			language: preferences.language,
			textScale: preferences.textScale,
			numberFormat: preferences.numberFormat,
			dateFormat: preferences.dateFormat,
			timeFormat: preferences.timeFormat,
			analytics: analyticsConsent.enabled
		},
		agent: { enabled: agentSettings.enabled, wallets: agentSettings.wallets },
		operations: transfers.list(),
		balance: balance.view,
		feed: feed.view,
		networks: networkAdmin.view.loaded ? networkAdmin.view : null,
		networkSpace: networkSpace.current,
		multisigs: multisig.accounts.map((a) => ({
			address: a.address,
			name: multisigAsAccount(a).name,
			threshold: a.threshold,
			owners: a.owners,
			waiting: multisig.active?.address === a.address ? proposals.open.length : null
		})),
		shownMultisig: multisig.active?.address ?? null
	};
}

export const intents = new Intents({
	ready: loadCore,
	check: (actor, action, seen) =>
		JSON.parse(
			intentCheck(
				actor,
				JSON.stringify(action),
				// The switch, and the wallets it may use (spec 107 E8).
				JSON.stringify({
					agent_enabled: seen.agent.enabled,
					wallets: seen.agent.wallets,
					// The account on screen: a multisig shown is the agent's wallet too.
					wallet: shownAddress(seen.session?.address ?? ''),
					// Every wallet here: what an erase takes with it.
					device_wallets: [
						...(seen.session?.accounts ?? []).map((row) => row.account.address),
						...seen.multisigs.map((m) => m.address)
					],
					// ob-2: what an erase may lose for good — never an agent's to take.
					unsynced_wallets: (seen.session?.unsynced ?? []).map(
						(wallet) => wallet.name.trim() || wallet.address
					)
				})
			)
		) as IntentCheck,
	world,
	perform,
	// Where an agent's change is seen opens once its request is done.
	reveal: async (screen) => {
		await show(screen);
	},
	now: () => Date.now(),
	// Under the bridge's own call timeout (ClientMCP's default is 120 s), so
	// the wallet says how the wait ended before the bridge gives up on it.
	approvalMs: 100_000,
	observeMs: 10_000,
	pollMs: 50
});
