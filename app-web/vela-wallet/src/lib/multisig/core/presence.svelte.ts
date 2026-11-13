/**
 * What waits on this device, per multisig (r4D-7): how many transactions
 * need an approval from a member here, and how many are ready to execute —
 * for the home's strip, the switcher's rows, and the dot on the switcher's
 * button when another account has something waiting.
 *
 * Every count is the core's proposals machine's (`multisig_proposals`,
 * through `shell/proposals-host.svelte.ts`): the shown multisig's from the
 * list it follows and the looks at its executors, the others from the
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
import type { MultisigWaiting } from '../generated/MultisigWaiting';
import type { MultisigWaitingBanner } from '../generated/MultisigWaitingBanner';
import type { MultisigWaitingPart } from '../generated/MultisigWaitingPart';
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

/**
 * "1 needs your approval · 1 ready to execute · 1 waiting for a member who
 * can pay" — '' when nothing waits. Which parts, in which order, is the
 * proposals machine's (`waiting_parts`, `banner.parts`); here only words.
 */
export function waitingWords(parts: MultisigWaitingPart[], m: MultisigMessages): string {
	return parts
		.map(({ kind, count }) => {
			const plural =
				kind === 'needsYou'
					? m.presence.needsYou
					: kind === 'ready'
						? m.presence.ready
						: m.presence.waitingPayer;
			return fillTemplate(pluralForm(plural, count), { count });
		})
		.join(' · ');
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
	 * What waits on `address` here, or `null` when it was not read yet: the
	 * proposals machine's (`presence`) — the shown one from its list and the
	 * looks at its executors, every other one read now and then, else what
	 * the waiting list kept from an earlier page says (r4b-11) — never a zero
	 * it did not see.
	 */
	of(address: string): Waiting | null {
		const read = proposalsHost.view?.presence[address.toLowerCase()];
		return read ? fromMachine(read) : null;
	}

	/** The waiting line's parts for `address`, in order (the machine's `waiting_parts`). */
	parts(address: string): MultisigWaitingPart[] {
		return proposalsHost.view?.waiting_parts[address.toLowerCase()] ?? [];
	}

	/**
	 * The line at the top of the followed multisig's home, when something
	 * waits there: its parts and the transaction it opens (`banner`).
	 */
	get banner(): MultisigWaitingBanner | null {
		return proposalsHost.view?.banner ?? null;
	}

	/** The followed multisig is deployed on a network read: its first steps are behind it. */
	get setUp(): boolean {
		return proposalsHost.view?.set_up ?? false;
	}

	/**
	 * Whether a multisig other than the one shown has something waiting —
	 * the machine's word (`others_waiting`: every multisig here but the one
	 * it follows, which is the one on screen; all of them when a Vela account
	 * is shown).
	 */
	get othersWaiting(): boolean {
		return proposalsHost.view?.others_waiting ?? false;
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
