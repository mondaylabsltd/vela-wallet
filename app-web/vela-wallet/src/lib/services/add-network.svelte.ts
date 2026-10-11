/**
 * "Add this network and retry" — the send flow's escape hatch when a pay link
 * names a chain this wallet does not have.
 *
 * Ported from src/services/add-network.ts @ f9bcb278. The wizard is the
 * `network_admin` core's; this file only turns the view transitions it produces
 * back into one answer. The session is the shared app-resident one (024), so
 * the ledger this gate reads is the ledger the settings screen writes.
 */
import type { NetNetworkRow } from '$lib/core/generated/NetNetworkRow';
import type { NetView } from '$lib/core/generated/NetView';
import { networkAdmin } from '$lib/settings/core/network-admin.svelte';

export type AddNetworkResult =
	| { ok: true; chainId: number }
	| {
			ok: false;
			/**
			 * `unverified` is a check that could not be made (`check_failed`) —
			 * never folded into `not-compatible`: an inconclusive probe is not a
			 * refusal (024's invariant ③, and the core keeps the two apart on
			 * this path since spec 038 #E1).
			 */
			reason: 'not-found' | 'not-compatible' | 'unverified';
			error?: string;
	  };

/**
 * The core drops every mutation until its stores are read (mutating a ledger
 * that is not the ledger would fabricate state), and a dropped event emits no
 * view — so waiting for `loaded` is what keeps this promise from hanging. The
 * read always concludes: an unreadable store still answers, empty.
 */
async function whenLoaded(): Promise<void> {
	await networkAdmin.boot();
	if (networkAdmin.view.loaded) return;
	await new Promise<void>((resolve) => {
		const stop = $effect.root(() => {
			$effect(() => {
				if (networkAdmin.view.loaded) {
					resolve();
					queueMicrotask(() => stop());
				}
			});
		});
	});
}

function savedRow(view: NetView, chainId: number): NetNetworkRow | undefined {
	return view.networks.find((row) => row.chain_id === chainId && row.is_custom);
}

/**
 * Ask the wizard to add `chainId` from the registry, and answer once it has
 * settled. Never throws: an unknown chain is `not-found`, an incompatible one
 * is `not-compatible`, one that could not be checked is `unverified`, and the
 * core's own wording is what the send screen shows.
 */
export async function addCustomNetworkByChainId(chainId: number): Promise<AddNetworkResult> {
	await whenLoaded();
	// A timestamp, not a clock: the record carries when it was added. The
	// reactivity rule wants `SvelteDate` for a Date that is READ over time; this
	// one is formatted once and thrown away.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const nowIso = new Date().toISOString();

	return new Promise<AddNetworkResult>((resolve) => {
		let settled = false;
		const stop = $effect.root(() => {
			$effect(() => {
				if (settled) return;
				const view = networkAdmin.view;
				const wizard = view.wizard;

				if (wizard.phase === 'error' && wizard.error) {
					settled = true;
					queueMicrotask(() => stop());
					switch (wizard.error.type) {
						case 'not_found':
							resolve({ ok: false, reason: 'not-found' });
							return;
						case 'already_added': {
							// The chain is present either way, so the caller's "retry
							// now that it exists" is the honest answer; a built-in chain
							// is present too and simply has no custom record.
							resolve(
								savedRow(view, chainId)
									? { ok: true, chainId }
									: { ok: false, reason: 'not-compatible' }
							);
							return;
						}
						case 'check_failed':
							// The probe did not answer: nothing was learned about the
							// network, so nothing is said against it.
							resolve({ ok: false, reason: 'unverified' });
							return;
						default:
							// `not_compatible`, and `no_rpc_endpoint` which the auto path
							// cannot reach. WHY it is refused stays on the wizard's view
							// (`error_key`, and the check beside it): this answer is a
							// code for the send machine, not words.
							resolve({ ok: false, reason: 'not-compatible' });
							return;
					}
				}

				// Success: the wizard resets to idle and the record joins the ledger.
				if (wizard.phase === 'idle' && savedRow(view, chainId)) {
					settled = true;
					queueMicrotask(() => stop());
					resolve({ ok: true, chainId });
				}
			});
		});

		networkAdmin.dispatch({
			type: 'add_by_chain_id_requested',
			chain_id: chainId,
			now_iso: nowIso
		});
	});
}
