/**
 * Spec 100: the add-network sheet a page opened (`NetView.dapp_add`), in words.
 *
 * Every judgement is the core's — which chain, whose name and coin, the
 * verdict, whether Add acts. This picks Settings' own line for each part, the
 * way `ExploreLive.addNetwork` does on the phones and `explore::add_network`
 * on the desktop. Pure, so a node test reads it without a page.
 */

import type { NetDappAddView } from '$lib/core/generated/NetDappAddView';
import type { CheckItemModel, StatusPillModel } from '$lib/settings/model';
import { netRefusal } from '$lib/settings/net-refusal';
import type { AddNetworkMessages } from './messages';

export interface AddNetworkCard {
	title: string;
	/** "{{host}} asks to add a network" — who asks, from the transport. */
	lead: string;
	rows: { label: string; value: string }[];
	/** The name and coin are the site's, not Vela's catalog's — or only its RPC is (dapp-13). */
	fromSite: string | null;
	pill: StatusPillModel | null;
	checksTitle: string | null;
	checks: CheckItemModel[];
	/** The sentence under the verdict. */
	note: string | null;
	/** "Add Network" — only where the core says it can act. */
	add: string | null;
	/**
	 * dapp-13: "unable to verify" is not a dead end — the door to adding it
	 * unverified, and what it risks, said before the explicit confirmation.
	 * Only where the core allows it (`can_add_unverified`).
	 */
	unverified: { open: string; warning: string; confirm: string } | null;
	retry: string | null;
	/**
	 * "Open Chain Setup Tool" and where it goes — the core's
	 * `NetCompatibility.setup_url` (`…/chain-setup?chain=<id>`). Only a
	 * refusal with something to deploy has one: a network with no P-256
	 * verifier gets no button.
	 */
	setupTool: { label: string; url: string } | null;
	/** Cancel while a decision is open, Done after a verdict — either way `dapp_add_declined`. */
	dismiss: string;
}

const fill = (template: string, vars: Record<string, string>): string =>
	Object.entries(vars).reduce(
		(text, [key, value]) => text.replaceAll(`{{${key}}}`, value),
		template
	);

export function addNetworkCard(view: NetDappAddView, m: AddNetworkMessages): AddNetworkCard {
	const chain = String(view.chain_id);
	const rows = [
		{ label: m.name, value: view.name || fill(m.chainName, { chainId: chain }) },
		{ label: m.chainIdLabel, value: chain }
	];
	if (view.native_symbol) rows.push({ label: m.nativeToken, value: view.native_symbol });
	if (view.rpc_host) rows.push({ label: m.rpcUrl, value: view.rpc_host });
	if (view.explorer_host) rows.push({ label: m.explorer, value: view.explorer_host });

	const verdict = view.phase === 'ready' || view.phase === 'not_compatible';
	const compat = verdict && view.compat && view.compat.rpc_failure === null ? view.compat : null;
	// A Vela network out of the person's list (owner decision 2026-10-29):
	// supported, and nothing was checked — no rows that would say it was.
	const checks: CheckItemModel[] =
		compat && !view.vela_network
			? [
					...compat.contracts.map((c) => ({ label: c.name, ok: c.deployed })),
					{ label: m.checkSigner, ok: compat.p256_available === true }
				]
			: [];

	const refusal = view.phase === 'not_compatible' ? netRefusal(view.compat, m.hints) : {};
	let pill: StatusPillModel | null = null;
	let note: string | null = null;
	switch (view.phase) {
		case 'checking':
			pill = { tone: 'neutral', label: m.checking, dot: true };
			break;
		case 'ready':
			if (view.vela_network) {
				pill = { tone: 'ok', label: m.velaNetwork, dot: true };
				note = m.velaNetworkHint;
				break;
			}
			pill = { tone: 'ok', label: m.compatible, dot: true };
			note = view.compat && !view.compat.multi_key_ready ? m.singleKeyOnly : null;
			break;
		case 'not_compatible':
			pill = { tone: 'error', label: m.incompatible, dot: true };
			// WHY, in the core's words (`compat.hint_key`): no P-256 verifier
			// — Vela wallets cannot work here and money sent would be stuck —
			// or contracts that are missing and can be deployed.
			note = refusal.hint ?? null;
			break;
		case 'check_failed':
			pill = { tone: 'warn', label: m.unableToVerify, dot: true };
			// Why, in plain words: nothing Vela may ask, or no answer from it.
			note =
				view.compat?.rpc_failure === 'all_probes_failed' && view.rpc_host
					? fill(m.whyUnreachable, { host: view.rpc_host })
					: m.whyNoRpc;
			break;
		case 'wrong_rpc':
			note = fill(m.wrongRpc, {
				actual: view.reported_chain_id === null ? '' : String(view.reported_chain_id),
				expected: chain
			});
			break;
		case 'no_rpc':
			note = m.noRpc;
			break;
	}
	const decided =
		view.phase === 'not_compatible' || view.phase === 'wrong_rpc' || view.phase === 'no_rpc';
	return {
		title: m.title,
		lead: fill(m.lead, { host: view.host }),
		rows,
		fromSite: view.from_site ? m.fromSite : view.rpc_from_site ? m.rpcFromSite : null,
		pill,
		checksTitle: checks.length > 0 ? m.checksTitle : null,
		checks,
		note,
		add: view.can_add ? m.add : null,
		unverified: view.can_add_unverified
			? {
					open: m.addUnverified,
					warning: fill(m.unverifiedWarning, { host: view.rpc_host ?? '', chainId: chain }),
					confirm: m.addUnverifiedConfirm
				}
			: null,
		retry: view.phase === 'check_failed' ? m.retry : null,
		setupTool:
			refusal.setupUrl === undefined ? null : { label: m.setupTool, url: refusal.setupUrl },
		dismiss: decided ? m.done : m.cancel
	};
}
