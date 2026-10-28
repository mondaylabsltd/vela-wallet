/**
 * Which multisig sheet is open. One at a time; the host
 * (`MultisigHost.svelte`) draws it, and the switcher, the home panel and the
 * wallet page's own doors open them.
 */
import type { DappCall } from './proposals.svelte';
import type { LinkedProposals } from './linked.svelte';

/**
 * A dApp's transaction from a multisig, owned by the request it came with
 * (`DappRequestHost`): reviewed first, proposed only on the person's
 * "Propose and approve", and answered by the request — whichever way the
 * sheet is left.
 */
export interface DappProposal {
	/** The request it answers. */
	rid: string;
	address: string;
	chainId: number;
	/** `eth_sendTransaction` (one call) or `wallet_sendCalls` (a batch, one proposal). */
	method: string;
	/** The call, or the batch's every call: one transaction either way. */
	calls: DappCall[];
	/** The site, as the browser names it (never the page's own claim). */
	origin: string;
	/** Set once it is in the mailbox. */
	safeTxHash: string | null;
	/** The person confirmed the review: propose it (one passkey prompt). Resolves its safeTxHash. */
	propose(): Promise<string>;
	/** Decline before it is proposed: the site is told the person rejected it. */
	decline(): void;
	/** The sheet was closed (any way out): the request answers as things stand. */
	close(): void;
}

export type MultisigSheet =
	| { kind: 'create' }
	| { kind: 'add' }
	| { kind: 'proposal'; chainId: number; safeTxHash: string }
	/**
	 * ms-21: a link's transaction on a device with none of its multisig's
	 * members — read from `source`, with nothing added and nothing switched.
	 */
	| { kind: 'preview'; chainId: number; safeTxHash: string; source: LinkedProposals }
	| { kind: 'invite' }
	/** ms-22: what a link could not open, said (a damaged link, a network Vela lacks). */
	| { kind: 'notice'; text: string }
	| { kind: 'dapp'; request: DappProposal };

class MultisigUi {
	sheet = $state<MultisigSheet | null>(null);

	open(sheet: MultisigSheet): void {
		this.sheet = sheet;
	}

	close(): void {
		this.sheet = null;
	}

	/** The dApp's transaction `rid` is in the mailbox now: its sheet shows it as one. */
	proposed(rid: string, safeTxHash: string): void {
		const sheet = this.sheet;
		if (sheet?.kind !== 'dapp' || sheet.request.rid !== rid) return;
		this.sheet = { kind: 'dapp', request: { ...sheet.request, safeTxHash } };
	}

	/** Close the dApp sheet of `rid`, if it is the one open. */
	closeDapp(rid: string): void {
		if (this.sheet?.kind === 'dapp' && this.sheet.request.rid === rid) this.sheet = null;
	}
}

export const multisigUi = new MultisigUi();
