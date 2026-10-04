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
import type { AddNetworkMessages } from './messages';

/** Where a chain this wallet refuses can be made ready (Settings links the same page). */
export const CHAIN_SETUP_URL = 'https://getvela.app/chain-setup';

export interface AddNetworkCard {
	title: string;
	/** "{{host}} asks to add a network" — who asks, from the transport. */
	lead: string;
	rows: { label: string; value: string }[];
	/** The name and coin are the site's, not Vela's catalog's. */
	fromSite: string | null;
	pill: StatusPillModel | null;
	checksTitle: string | null;
	checks: CheckItemModel[];
	/** The sentence under the verdict. */
	note: string | null;
	/** "Add Network" — only where the core says it can act. */
	add: string | null;
	retry: string | null;
	setupTool: string | null;
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
	const checks: CheckItemModel[] = compat
		? [
				...compat.contracts.map((c) => ({ label: c.name, ok: c.deployed })),
				{ label: m.checkSigner, ok: compat.p256_available === true }
			]
		: [];

	let pill: StatusPillModel | null = null;
	let note: string | null = null;
	switch (view.phase) {
		case 'checking':
			pill = { tone: 'neutral', label: m.checking, dot: true };
			break;
		case 'ready':
			pill = { tone: 'ok', label: m.compatible, dot: true };
			note = view.compat && !view.compat.multi_key_ready ? m.singleKeyOnly : null;
			break;
		case 'not_compatible':
			pill = { tone: 'error', label: m.incompatible, dot: true };
			note = m.incompatibleHint;
			break;
		case 'check_failed':
			pill = { tone: 'warn', label: m.unableToVerify, dot: true };
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
		fromSite: view.from_site ? m.fromSite : null,
		pill,
		checksTitle: checks.length > 0 ? m.checksTitle : null,
		checks,
		note,
		add: view.can_add ? m.add : null,
		retry: view.phase === 'check_failed' ? m.retry : null,
		setupTool: view.phase === 'not_compatible' ? m.setupTool : null,
		dismiss: decided ? m.done : m.cancel
	};
}
