/**
 * Whose wallet the pages show: the multisig shown, else the session's own
 * Vela account. Every page that wears an identity (wallet, contacts,
 * settings) reads it here, so moving between them never changes whose
 * balance, activity or contacts are on screen.
 */
import { accountName } from '$lib/multisig/core/account-name';
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
 * in force on each network it is used on, r4D-7); one not followed shows its
 * row's rule — the rule a chain last said, kept by the core (r9-14), marked
 * last known — never the founding record's when a chain said otherwise.
 */
export function ruleChip(
	account: Pick<MultisigAccount, 'address' | 'owners' | 'threshold'>,
	m: MultisigMessages
): { label: string; title: string } {
	const live = sameAddress(proposals.followed, account.address)
		? proposalsHost.view?.rule_chip
		: null;
	const row = multisig.row(account.address)?.rule;
	const chip: MultisigRuleChip = live ??
		row?.chip ?? {
			kind: 'needs',
			rule: { m: account.threshold, n: account.owners.length, solo: account.owners.length === 1 }
		};
	const lastKnown = !live && row?.from_chain === true && row.last_known;
	const titled = (title: string) => (lastKnown ? `${title} · ${m.presence.ruleLastKnown}` : title);
	if (chip.kind === 'needs') {
		return { label: ruleChipText(m, chip.rule), title: titled(ruleTitle(m, chip.rule)) };
	}
	return {
		label: m.presence.ruleDiffers,
		title: titled(chip.rules.map((r) => ruleTitle(m, r)).join(' · '))
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
		// r9-12: the one name the core gives this address everywhere.
		name: accountName(view.address, view.accounts[view.active_index]?.account.name ?? ''),
		address: view.address,
		identiconSvg: identiconSvgForClient(view.address),
		...waiting
	};
}

/** The address the pages show: the multisig shown, else the session's own. */
export function shownAddress(sessionAddress: string): string {
	return multisig.active?.address ?? sessionAddress;
}
