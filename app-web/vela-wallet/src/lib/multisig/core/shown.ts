/**
 * Whose wallet the pages show: the multisig shown, else the session's own
 * Vela account. Every page that wears an identity (wallet, contacts,
 * settings) reads it here, so moving between them never changes whose
 * balance, activity or contacts are on screen.
 */
import { resolve } from '$app/paths';
import { identiconSvgForClient } from '$lib/wallet/identicon';
import type { WalletIdentity } from '$lib/wallet/identity';
import type { MultisigMessages } from '../messages';
import { multisigName, ruleChipText, ruleTitle } from './display';
import { sameAddress } from './kernel';
import { presence } from './presence.svelte';
import { proposals } from './proposals.svelte';
import { multisig, type MultisigAccount } from './store.svelte';

interface SessionLike {
	address: string;
	active_index: number;
	accounts: { account: { name: string } }[];
}

/**
 * The rule in force on each network the multisig is used on (r4D-7): its
 * members there now where it is deployed (read for the multisig shown), its
 * original ones where it is not — or for one not read yet. Distinct rules,
 * in the networks' order.
 */
export function currentRules(
	account: Pick<MultisigAccount, 'address' | 'owners' | 'threshold'>
): { m: number; n: number }[] {
	const original = { m: account.threshold, n: account.owners.length };
	if (!sameAddress(proposals.followed, account.address) || proposals.chains.length === 0) {
		return [original];
	}
	const rules = proposals.chains.map((c) =>
		c.members.deployed ? { m: c.members.threshold, n: c.members.owners.length } : original
	);
	return rules.filter((r, i) => rules.findIndex((o) => o.m === r.m && o.n === r.n) === i);
}

/** The rule chip's words: "Needs 2 of 3", or that it differs by network. */
export function ruleChip(
	account: Pick<MultisigAccount, 'address' | 'owners' | 'threshold'>,
	m: MultisigMessages
): { label: string; title: string } {
	const rules = currentRules(account);
	if (rules.length === 1) {
		return { label: ruleChipText(m, rules[0]), title: ruleTitle(m, rules[0]) };
	}
	return {
		label: m.presence.ruleDiffers,
		title: rules.map((r) => ruleTitle(m, r)).join(' · ')
	};
}

export function shownIdentity(
	view: SessionLike,
	m: MultisigMessages,
	locale: string
): WalletIdentity {
	const shown = multisig.active;
	// r4D-7: a dot on the switcher's button while another account has something waiting.
	const waiting = presence.othersWaiting ? { waiting: m.presence.othersWaiting } : {};
	if (shown) {
		return {
			name: multisigName(shown, m),
			address: shown.address,
			identiconSvg: identiconSvgForClient(shown.address),
			rule: { ...ruleChip(shown, m), href: resolve('/[locale]/settings', { locale }) },
			...waiting
		};
	}
	return {
		name: view.accounts[view.active_index]?.account.name ?? '',
		address: view.address,
		identiconSvg: identiconSvgForClient(view.address),
		...waiting
	};
}

/** The address the pages show: the multisig shown, else the session's own. */
export function shownAddress(sessionAddress: string): string {
	return multisig.active?.address ?? sessionAddress;
}
