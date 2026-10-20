/**
 * Adding a token through the intents (spec 103): a session of the same
 * `manage_tokens` core the add-token sheet builds, sent the sheet's own
 * events — the address, the search, the save — for the one network named.
 * The core's view says whether it was found and stored; nothing is decided
 * here.
 */
import { loadCore } from '$lib/core/client';
import type { IntentReason } from '$lib/core/generated/IntentReason';
import type { MtokView } from '$lib/core/generated/MtokView';
import { getAllNetworksSync } from '$lib/services/networks';
import { session as wallet } from '$lib/session/core/session.svelte';
import { balance } from '$lib/wallet/core/balance.svelte';
import { createManageTokensSession } from '$lib/wallet/core/manage-tokens-session';

/** How long the search and the save may each take. */
const STEP_MS = 30_000;

const reason = (code: IntentReason['code'], detail: string): IntentReason => ({ code, detail });

async function until(read: () => boolean, ms: number): Promise<boolean> {
	const deadline = Date.now() + ms;
	for (;;) {
		if (read()) return true;
		if (Date.now() >= deadline) return false;
		await new Promise((done) => setTimeout(done, 50));
	}
}

/** Find the contract on `chainId` and add it: why not, or `null` once stored. */
export async function addToken(chainId: number, address: string): Promise<IntentReason | null> {
	const network = getAllNetworksSync().find((entry) => entry.chainId === chainId);
	if (!network) return reason('not_found', 'the wallet has no network with that chain id');
	await loadCore();
	let view: MtokView | null = null;
	const tokens = createManageTokensSession({
		account: () => wallet.view.address ?? '',
		onInvalidated: () => balance.refresh(true),
		onView: (next) => (view = next),
		onError: (error) => console.error('[manage_tokens] core fault:', error)
	});
	const read = () => view as MtokView | null;
	try {
		tokens.start({ type: 'start' });
		tokens.dispatch({ type: 'address_input', s: address.trim() });
		await until(() => read()?.input_address === address.trim(), STEP_MS);
		if (!read()?.address_valid) return reason('invalid_input', 'that is not a contract address');
		tokens.dispatch({
			type: 'detect_requested',
			networks: [{ chain_id: network.chainId, name: network.displayName }]
		});
		const settled = await until(() => {
			const now = read();
			return (
				now !== null &&
				!now.detecting &&
				(now.found.length > 0 || now.not_found || now.native_alias)
			);
		}, STEP_MS);
		const searched = read();
		if (!settled || !searched)
			return reason('not_observed', 'the token could not be looked up in time');
		if (searched.native_alias)
			return reason('refused', "that address is the network's own coin, already in the wallet");
		const card = searched.found.find((entry) => entry.chain_id === chainId);
		if (!card) return reason('not_found', 'no token answers at that address on that network');
		if (card.added) return null;
		tokens.dispatch({ type: 'save_requested', chain_id: chainId });
		const saved = await until(() => {
			const now = read();
			return (
				now !== null &&
				!now.saving &&
				(now.save_error || now.found.some((entry) => entry.chain_id === chainId && entry.added))
			);
		}, STEP_MS);
		if (!saved) return reason('not_observed', 'the token was not seen stored in time');
		return read()?.save_error ? reason('refused', 'the token could not be stored') : null;
	} finally {
		tokens.dispose();
	}
}
