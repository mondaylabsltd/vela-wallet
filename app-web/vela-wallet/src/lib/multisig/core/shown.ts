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
import { proposalsHost } from '../shell/proposals-host.svelte';
import type { MultisigRuleChip } from '../generated/MultisigRuleChip';

interface SessionLike {
	address: string;
	active_index: number;
	accounts: { account: { name: string } }[];
}

/**
 * The rule chip's words: "Needs 2 of 3", or that it differs by network —
 * the proposals machine's `rule_chip` for the multisig it follows (the rule
 * in force on each network it is used on, r4D-7); one not followed shows
 * the rule it was made with.
 */
export function ruleChip(
	account: Pick<MultisigAccount, 'address' | 'owners' | 'threshold'>,
	m: MultisigMessages
): { label: string; title: string } {
	const chip: MultisigRuleChip = (sameAddress(proposals.followed, account.address)
		? proposalsHost.view?.rule_chip
		: null) ?? {
		kind: 'needs',
		rule: { m: account.threshold, n: account.owners.length, solo: account.owners.length === 1 }
	};
	if (chip.kind === 'needs') {
		return { label: ruleChipText(m, chip.rule), title: ruleTitle(m, chip.rule) };
	}
	return {
		label: m.presence.ruleDiffers,
		title: chip.rules.map((r) => ruleTitle(m, r)).join(' · ')
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
