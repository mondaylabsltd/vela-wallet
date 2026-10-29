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
import { findMultisig } from './accounts';
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
	refresh(account: MultisigAccount, chainIds: number[]): Promise<void>;
}

/**
 * One chain of a multisig the wallet is not showing, read on demand — the
 * slice of `proposals` a proposal's sheet reads, for a link's transaction.
 */
export class LinkedProposals implements ProposalSource {
	chains = $state<ChainState[]>([]);
	loading = $state(true);
	/** The last read failed: the transaction shown may be stale. */
	stale = $state(false);
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
		} catch {
			// The members could not be read: what was read stays, said as stale.
			this.stale = true;
		} finally {
			this.loading = false;
		}
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
		const found = await findMultisig(link.multisig, [link.chainId], link.chainId);
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
