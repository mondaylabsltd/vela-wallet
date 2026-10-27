/**
 * Networks and services through the intents (spec 103): the same events
 * Settings' network pages send to the network ledger (`networkAdmin`), in the
 * order its controls send them — open the card, edit the field, leave it.
 * Whether a change held is read back from the ledger's view (`rules.ts`).
 */
import type { IntentReason } from '$lib/core/generated/IntentReason';
import type { NetEndpointField } from '$lib/core/generated/NetEndpointField';
import type { NetOverrideField } from '$lib/core/generated/NetOverrideField';
import type { NetProviderId } from '$lib/core/generated/NetProviderId';
import { networkAdmin } from '$lib/settings/core/network-admin.svelte';

/** How long the add wizard may take to look a chain up and check it. */
const CHECK_MS = 45_000;

async function ready(): Promise<void> {
	await networkAdmin.boot();
}

/**
 * Settings → Add network: look the chain up, try `rpcUrl` first if given,
 * check the wallet can serve it, and add it. Resolves with why not, or `null`.
 *
 * A wizard already checked for this chain (the person's own, at its "Add
 * network") is not checked again: only the add is sent, as its button does.
 */
export async function addNetwork(
	chainId: number,
	rpcUrl: string | null
): Promise<IntentReason | null> {
	await ready();
	const now = networkAdmin.view.wizard;
	const checked =
		now.phase === 'checked' &&
		now.can_add &&
		now.chain_info?.chain_id === chainId &&
		(rpcUrl === null || now.custom_rpc.trim() === rpcUrl.trim());
	if (!checked) {
		networkAdmin.dispatch({ type: 'wizard_reset' });
		networkAdmin.dispatch({ type: 'chain_selected', chain_id: chainId, keep_custom_rpc: false });
		if (rpcUrl !== null) {
			networkAdmin.dispatch({ type: 'custom_rpc_edited', value: rpcUrl.trim() });
			networkAdmin.dispatch({ type: 'chain_selected', chain_id: chainId, keep_custom_rpc: true });
		}
	}
	const deadline = Date.now() + CHECK_MS;
	for (;;) {
		const wizard = networkAdmin.view.wizard;
		if (wizard.phase === 'error' && wizard.error)
			return { code: 'refused', detail: `add network: ${wizard.error.type}` };
		if (wizard.phase === 'checked') {
			if (wizard.can_add) break;
			const why = wizard.compat?.blocker ?? wizard.compat?.rpc_failure ?? 'not compatible';
			return { code: 'refused', detail: `add network: ${why}` };
		}
		if (Date.now() >= deadline)
			return { code: 'not_observed', detail: 'the network could not be checked in time' };
		await new Promise((done) => setTimeout(done, 100));
	}
	networkAdmin.dispatch({ type: 'add_confirmed', now_iso: new Date().toISOString() });
	return null;
}

/** How long the RPC save gate may take to hear the endpoint's chain id. */
const GATE_MS = 20_000;

/**
 * One network's card: open it, edit the field, leave it (which saves). A
 * field already showing `url` is only left.
 *
 * An RPC goes through the network core's save gate: one answering for another
 * chain is not written. The card would keep the refused draft — and the view
 * shows a card's draft as its value — so the field is put back to what is in
 * effect, and the refusal is the answer.
 */
export async function setNetworkEndpoint(
	chainId: number,
	field: NetOverrideField,
	url: string
): Promise<IntentReason | null> {
	await ready();
	const shownOf = () => {
		const row = networkAdmin.view.networks.find((entry) => entry.chain_id === chainId);
		return field === 'rpc' ? row?.rpc_url : row?.explorer_url;
	};
	const before = shownOf();
	if (before !== url.trim()) {
		networkAdmin.dispatch({ type: 'override_expanded', chain_id: chainId });
		networkAdmin.dispatch({
			type: 'override_field_edited',
			chain_id: chainId,
			field,
			value: url.trim()
		});
	}
	networkAdmin.dispatch({ type: 'override_blurred', chain_id: chainId });
	if (field !== 'rpc') return null;
	const deadline = Date.now() + GATE_MS;
	for (;;) {
		const row = networkAdmin.view.networks.find((entry) => entry.chain_id === chainId);
		if (row?.rpc_chain_mismatch) {
			if (before !== undefined)
				networkAdmin.dispatch({
					type: 'override_field_edited',
					chain_id: chainId,
					field,
					value: before
				});
			return {
				code: 'refused',
				detail: 'that RPC answers for another chain; the network keeps the RPC it had'
			};
		}
		if (!row?.rpc_save_deferred || Date.now() >= deadline) return null;
		await new Promise((done) => setTimeout(done, 100));
	}
}

/** The network's own row is what its delete names. */
export async function removeNetwork(chainId: number): Promise<void> {
	await ready();
	const row = networkAdmin.view.networks.find(
		(entry) => entry.chain_id === chainId && entry.is_custom
	);
	networkAdmin.dispatch({ type: 'delete_confirmed', id: row?.id ?? `custom-${chainId}` });
}

/**
 * RPC providers: open the page (it seeds the drafts), type the key, leave it.
 * A key already in its field is only left — opening the page again would
 * reseed every draft.
 */
export async function setProviderKey(provider: NetProviderId, key: string): Promise<void> {
	await ready();
	const row = networkAdmin.view.providers.find((entry) => entry.provider === provider);
	if (row?.key !== key.trim()) {
		networkAdmin.dispatch({ type: 'providers_opened' });
		networkAdmin.dispatch({ type: 'provider_key_edited', provider, value: key.trim() });
	}
	networkAdmin.dispatch({ type: 'provider_key_blurred', provider });
}

/** Service endpoints: open the page, type the URL, leave it — or only leave it. */
export async function setServiceEndpoint(field: NetEndpointField, url: string): Promise<void> {
	await ready();
	const row = networkAdmin.view.endpoints.find((entry) => entry.field === field);
	if (row?.value !== url.trim()) {
		networkAdmin.dispatch({ type: 'endpoints_opened' });
		networkAdmin.dispatch({ type: 'endpoint_edited', field, value: url.trim() });
	}
	networkAdmin.dispatch({ type: 'endpoint_blurred', field });
}

export async function resetServiceEndpoints(): Promise<void> {
	await ready();
	networkAdmin.dispatch({ type: 'reset_endpoints_to_defaults' });
}
