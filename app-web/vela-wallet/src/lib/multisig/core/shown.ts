/**
 * Whose wallet the pages show: the multisig shown, else the session's own
 * Vela account. Every page that wears an identity (wallet, contacts,
 * settings) reads it here, so moving between them never changes whose
 * balance, activity or contacts are on screen.
 */
import { resolve } from '$app/paths';
import { fillTemplate } from '$lib/i18n/fill';
import { identiconSvgForClient } from '$lib/wallet/identicon';
import type { WalletIdentity } from '$lib/wallet/identity';
import type { MultisigMessages } from '../messages';
import { multisigName } from './display';
import { multisig } from './store.svelte';

interface SessionLike {
	address: string;
	active_index: number;
	accounts: { account: { name: string } }[];
}

export function shownIdentity(
	view: SessionLike,
	m: MultisigMessages,
	locale: string
): WalletIdentity {
	const shown = multisig.active;
	if (shown) {
		const n = shown.owners.length;
		return {
			name: multisigName(shown, m),
			address: shown.address,
			identiconSvg: identiconSvgForClient(shown.address),
			rule: {
				label: `${shown.threshold}/${n}`,
				title: fillTemplate(m.panel.title, { m: shown.threshold, n }),
				href: resolve('/[locale]/settings', { locale })
			}
		};
	}
	return {
		name: view.accounts[view.active_index]?.account.name ?? '',
		address: view.address,
		identiconSvg: identiconSvgForClient(view.address)
	};
}

/** The address the pages show: the multisig shown, else the session's own. */
export function shownAddress(sessionAddress: string): string {
	return multisig.active?.address ?? sessionAddress;
}
