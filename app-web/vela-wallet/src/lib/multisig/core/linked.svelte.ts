/**
 * A proposal link (`#ms=`) opened on this device, decided before anything
 * changes (ms-21).
 *
 * A device with a member of the multisig opens the transaction as it always
 * did: the multisig is added if it is not here yet, and shown. A device with
 * none of its members is only shown the transaction — read the same way,
 * checked the same way — and nothing is added and no account is switched:
 * following the multisig here, or signing with a member's own wallet, are
 * the person's choices, each on a tap.
 */
import { localOwners } from './owners';
import { readChain, type ChainState } from './proposals.svelte';
import type { ProposalLink } from './link';
import type { Proposal } from './service';
import { multisig, type MultisigAccount } from './store.svelte';

/**
 * Where a proposal's sheet reads its transaction from: the shown multisig's
 * (`proposals`), or a linked one's ({@link LinkedProposals}).
 */
export interface ProposalSource {
	loading: boolean;
	chain(chainId: number): ChainState | undefined;
	/** `fresh`: a read begun after this call (never one already under way). */
	refresh(
		account: MultisigAccount,
		chainIds: number[],
		options?: { fresh?: boolean }
	): Promise<void>;
	/** r4b-3: networks whose members could not be read on the last refresh. */
	unreadable: number[];
	/**
	 * r4b-4: keep it fresh while its sheet is open (a poll, and at once when
	 * the tab is shown again); the returned function stops. The shown
	 * multisig's own list is already followed, and has none.
	 */
	watch?(): () => void;
}

/**
 * How often a linked transaction is read again while its sheet is open — the
 * pace `proposals` keeps while a sheet is open on one of its own
 * (r5reg-14, `OPEN_POLL_MS`).
 */
export const LINKED_POLL_MS = 4_000;

/**
 * One chain of a multisig the wallet is not showing, read on demand — the
 * slice of `proposals` a proposal's sheet reads, for a link's transaction.
 */
export class LinkedProposals implements ProposalSource {
	chains = $state<ChainState[]>([]);
	loading = $state(true);
	/** The last read failed: the transaction shown may be stale. */
	stale = $state(false);
	/** r4b-3: the network did not answer the last read. */
	unreadable = $state<number[]>([]);
	/** The mailbox's last list: kept while it cannot be read (`readChain`). */
	#list: Proposal[] | null = null;

	constructor(
		readonly account: MultisigAccount,
		readonly chainId: number
	) {}

	chain(chainId: number): ChainState | undefined {
		return this.chains.find((c) => c.chainId === chainId);
	}

	/** Read it again (the sheet's arguments are its own account and chain). */
	async refresh(): Promise<void> {
		try {
			const read = await readChain(this.account, this.chainId, this.#list);
			if (read.mailbox) this.#list = read.proposals;
			this.chains = [
				{ chainId: this.chainId, members: read.members, items: read.items, mailbox: read.mailbox }
			];
			this.stale = !read.mailbox;
			this.unreadable = [];
		} catch {
			// The members could not be read: what was read stays, said as stale
			// — and the network as the one that did not answer.
			this.stale = true;
			this.unreadable = [this.chainId];
		} finally {
			this.loading = false;
		}
	}

	/** r4b-4: read again every {@link LINKED_POLL_MS} while shown, and when the tab or window comes back. */
	watch(): () => void {
		const timer = setInterval(() => {
			if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return;
			void this.refresh();
		}, LINKED_POLL_MS);
		const onVisible = () => {
			if (document.visibilityState === 'visible') void this.refresh();
		};
		const onOnline = () => void this.refresh();
		if (typeof document !== 'undefined') {
			document.addEventListener('visibilitychange', onVisible);
			window.addEventListener('online', onOnline);
			// r5reg-14: the window comes back (another app, another window): read now.
			window.addEventListener('focus', onOnline);
		}
		return () => {
			clearInterval(timer);
			if (typeof document !== 'undefined') {
				document.removeEventListener('visibilitychange', onVisible);
				window.removeEventListener('online', onOnline);
				window.removeEventListener('focus', onOnline);
			}
		};
	}
}

export type LinkOpening =
	/** A member is on this device: open it as before (added, if it is not here yet). */
	| { kind: 'member'; account: MultisigAccount; added: boolean }
	/** None is: the transaction, read only — nothing added, nothing switched. */
	| { kind: 'preview'; source: LinkedProposals; added: boolean }
	/**
	 * The multisig could not be found or read — or who can act for it is not
	 * only its members (§3.5): its genesis refused (`refused`), not checkable
	 * yet (`unverified`), or deployed with more than its members acting for
	 * it (`highRisk`). Nothing of it is read or signed here; the add sheet
	 * says why, and only the person's explicit tap adds a high-risk one.
	 */
	| {
			kind: 'failed';
			reason:
				| 'notFound'
				| 'unavailable'
				| 'unsupported'
				| 'notMultisig'
				| 'unchecked'
				| 'refused'
				| 'unverified'
				| 'highRisk';
	  };

/** How the link's transaction opens here. Changes nothing on this device. */
export async function openProposalLink(link: ProposalLink): Promise<LinkOpening> {
	const stored = multisig.find(link.multisig);
	// Its founding members are enough to know a member is here.
	if (stored && localOwners(stored.owners).length > 0) {
		return { kind: 'member', account: stored, added: true };
	}
	let account = stored;
	if (!account) {
		// The link names its transaction's network: that is where it is used.
		// r4b-13: one lookup, answered in a few seconds — asked once more at most.
		// The accounts machine's lookup, kept nowhere (its `find`).
		// W8: moves onto the `#ms=` link flow once merged.
		const found = await multisig.lookUp(link.multisig, {
			network: link.chainId,
			chainIds: [link.chainId],
			waitMs: 2500
		});
		// (`chooseNetwork` cannot come back: the link names one.)
		if (!found.ok)
			return {
				kind: 'failed',
				reason: found.reason === 'chooseNetwork' ? 'notFound' : found.reason
			};
		account = found.account;
	}
	// The members now are the chain's: one added since the genesis counts.
	const source = new LinkedProposals(account, link.chainId);
	await source.refresh();
	const owners = source.chain(link.chainId)?.members.owners ?? account.owners;
	if (localOwners(owners).length > 0) return { kind: 'member', account, added: !!stored };
	return { kind: 'preview', source, added: !!stored };
}
