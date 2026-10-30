/**
 * What waits on this device, per multisig (r4D-7): how many transactions
 * need an approval from a member here, and how many are ready to execute —
 * for the home's strip, the switcher's rows, and the dot on the switcher's
 * button when another account has something waiting.
 *
 * The shown multisig's counts are its followed proposals' (`proposals`,
 * re-read every 12 s). The others are the core's proposals machine's
 * (`multisig_proposals`, through `shell/proposals-host.svelte.ts`): the
 * mailbox's cross-chain list, then each network it has proposals on through
 * this wallet's RPC — the same checks the shown one gets — read only while a
 * page is shown. A count is what this device read; a multisig not read yet
 * on this page counts what the waiting list kept from an earlier one says
 * (r4b-11, the list its home shows at once), else has none — never a zero it
 * did not see.
 */
import { fillTemplate } from '$lib/i18n/fill';
import { pluralForm } from '$lib/i18n/plural';
import type { MultisigMessages } from '../messages';
import { executionInput, proposals, type ProposalItem } from './proposals.svelte';
import { blockOf, preflight } from './preflight.svelte';
import { multisig, type MultisigAccount } from './store.svelte';
import { sameAddress } from './kernel';
import type { MultisigWaiting } from '../generated/MultisigWaiting';
import { proposalsHost } from '../shell/proposals-host.svelte';

export interface Waiting {
	/** Open, short of approvals, and a member on this device has not approved. */
	needsYou: number;
	/**
	 * Enough approvals, its nonce next, and a member here can execute it now
	 * (r5reg-6): not one nobody here can pay for, nor one that would fail.
	 */
	ready: number;
	/**
	 * r5reg-6: enough approvals, its nonce next, but no member here can pay
	 * its fee (or none is here): it waits for a member who can.
	 */
	waitingPayer: number;
	/** The first of them, to open: the one needing an approval first, else the first ready. */
	first: { chainId: number; safeTxHash: string } | null;
}

/** Why a ready transaction cannot run here now (`blockOf`'s word), or `null`: it can, or not known yet. */
export type ReadyBlock = 'fails' | 'nobodyPays' | null;

/**
 * What `items` (one multisig's, as this device reads them) ask of this
 * device. `blockedOf`: why a ready one cannot run here (the looks the home
 * already takes); absent, a ready one with a member here counts as ready.
 */
export function waitingOf(
	items: ProposalItem[],
	blockedOf: (item: ProposalItem) => ReadyBlock = () => null
): Waiting {
	const open = items.filter((i) => i.status !== 'done' && !i.cancelledBy);
	const needs = open.filter(
		(i) => i.status !== 'ready' && i.approved < i.threshold && i.canApprove.length > 0
	);
	const readyAll = open.filter((i) => i.status === 'ready');
	// r5reg-6: ready only where someone here can run it; a would-fail one is
	// its row's to say ("Can't run now — …"), counted nowhere here.
	const ready = readyAll.filter((i) => i.canExecute.length > 0 && blockedOf(i) === null);
	const waitingPayer = readyAll.filter(
		(i) => i.canExecute.length === 0 || blockedOf(i) === 'nobodyPays'
	);
	const first = needs[0] ?? ready[0] ?? null;
	return {
		needsYou: needs.length,
		ready: ready.length,
		waitingPayer: waitingPayer.length,
		first: first ? { chainId: first.chainId, safeTxHash: first.safeTxHash } : null
	};
}

/** "1 needs your approval · 1 ready to execute · 1 waiting for a member who can pay" — '' when nothing waits. */
export function waitingWords(w: Waiting, m: MultisigMessages): string {
	const parts: string[] = [];
	if (w.needsYou > 0) {
		parts.push(fillTemplate(pluralForm(m.presence.needsYou, w.needsYou), { count: w.needsYou }));
	}
	if (w.ready > 0) {
		parts.push(fillTemplate(pluralForm(m.presence.ready, w.ready), { count: w.ready }));
	}
	if (w.waitingPayer > 0) {
		parts.push(
			fillTemplate(pluralForm(m.presence.waitingPayer, w.waitingPayer), {
				count: w.waitingPayer
			})
		);
	}
	return parts.join(' · ');
}

/**
 * Why the shown multisig's ready `item` cannot run here, from the looks the
 * home and its sheet already take at each executor (`preflight`) — `null`
 * while they are out, or when someone here can run it.
 */
function readyBlock(account: MultisigAccount | undefined, item: ProposalItem): ReadyBlock {
	const members = proposals.chain(item.chainId)?.members;
	if (!account || !members || item.canExecute.length === 0) return null;
	const block = blockOf(
		item.canExecute.map((o) => preflight.get(executionInput(account, item, o, members)))
	);
	return block?.kind ?? null;
}

/** The machine's word for one multisig, in this module's shape. */
function fromMachine(w: MultisigWaiting): Waiting {
	return {
		needsYou: w.needs_you,
		ready: w.ready,
		waitingPayer: w.waiting_payer,
		first: w.first ? { chainId: w.first.chain_id, safeTxHash: w.first.safe_tx_hash } : null
	};
}

class Presence {
	/**
	 * What waits on `address` here, or `null` when it was not read yet. The
	 * shown multisig's is its followed proposals'; every other one's is the
	 * proposals machine's (`presence`): read now and then, else what the
	 * waiting list kept from an earlier page says (r4b-11) — never a zero it
	 * did not see.
	 */
	of(address: string): Waiting | null {
		if (sameAddress(proposals.followed, address)) {
			const account = multisig.find(address);
			return waitingOf(proposals.items, (item) => readyBlock(account, item));
		}
		const read = proposalsHost.view?.presence[address.toLowerCase()];
		return read ? fromMachine(read) : null;
	}

	/** Whether a multisig other than `shown` (the address on screen) has something waiting. */
	othersWaiting(shown: string | null): boolean {
		return multisig.accounts.some((a) => {
			if (shown && sameAddress(a.address, shown)) return false;
			const w = this.of(a.address);
			return !!w && w.needsYou + w.ready > 0;
		});
	}

	/** Read every multisig not shown again (the machine's `others_tick`: one round at a time). */
	refreshOthers(): Promise<void> {
		return proposalsHost.othersTick();
	}

	/**
	 * Read the others now and every `OTHERS_EVERY_MS` while the page is
	 * shown, and when it is shown again. Returns the stop.
	 */
	follow(): () => void {
		return proposalsHost.followOthers();
	}
}

export const presence = new Presence();
