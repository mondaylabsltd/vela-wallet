/**
 * Which multisig sheet is open. One at a time; the host
 * (`MultisigHost.svelte`) draws it, and the switcher, the home panel and the
 * wallet page's own doors open them.
 */
import type { MultisigHeldRequest } from '../generated/MultisigHeldRequest';
import type { AddLink, ProposalLink } from './link';
import type { MultisigAccount } from './store.svelte';
import type {
	SigningBlockedError,
	SigningRefusedError,
	SigningUncheckedError
} from './gate.svelte';

/**
 * A dApp's request to a multisig, held by the proposals machine
 * (`view.dapp[rid]`, `shell/dapp-requests.ts`): its review first (the
 * machine's draft), proposed only on the person's "Propose and approve",
 * and answered by the machine whichever way the sheet is left. The sheet
 * draws `held` — kept as last seen once it is answered, so a sheet left on
 * the proposal still says what the site was told.
 */
export interface DappRequestSheet {
	/** The request it answers. */
	rid: string;
	/** The machine's held request, as last seen. */
	held: MultisigHeldRequest;
	/** RB5: is the request still live? Asked before the passkey; a gone page is withdrawn. */
	claim(): Promise<boolean>;
	/** Its review could not be drawn (the mailbox did not answer): asked again. */
	retry(): void;
}

export type MultisigSheet =
	| { kind: 'create' }
	/**
	 * Add one by its address. `link`: the invite it came from (its networks,
	 * its members' labels); `broken`: what a damaged invite held, said at
	 * once (r4b-13) — `fault: 'checksum'` when its address's letters were
	 * changed on the way (r5reg-22).
	 */
	| { kind: 'add'; link?: AddLink; broken?: string; fault?: 'checksum' | 'damaged' }
	| { kind: 'proposal'; chainId: number; safeTxHash: string }
	/**
	 * ms-21: a link's transaction on a device with none of its multisig's
	 * members (`account`, kept nowhere) — read by the proposals machine
	 * (`linked`), with nothing added and nothing switched.
	 */
	| { kind: 'preview'; chainId: number; safeTxHash: string; account: MultisigAccount }
	| { kind: 'invite' }
	/** ms-22: what a link could not open, said (a damaged link). */
	| { kind: 'notice'; text: string }
	/**
	 * A proposal on a network this device does not have (from the mailbox, or
	 * a link): said, with Add network — nothing read, judged or counted until
	 * it is added. `link`: the link that named it, opened again once it is.
	 */
	| { kind: 'missing'; chainId: number; link?: ProposalLink }
	| { kind: 'dapp'; request: DappRequestSheet }
	/**
	 * §3.5: a signature asked where this device signs nothing for it (its
	 * genesis refused there, or who can act for it not read yet) by a flow
	 * that cannot say why itself (the send flow): said here.
	 */
	| {
			kind: 'authority';
			error: SigningRefusedError | SigningUncheckedError | SigningBlockedError;
	  };

class MultisigUi {
	sheet = $state<MultisigSheet | null>(null);

	open(sheet: MultisigSheet): void {
		this.sheet = sheet;
	}

	close(): void {
		this.sheet = null;
	}

	/** Where the machine's held request `rid` stands now: its sheet, if it is open, draws it. */
	held(rid: string, held: MultisigHeldRequest): void {
		const sheet = this.sheet;
		if (sheet?.kind !== 'dapp' || sheet.request.rid !== rid) return;
		this.sheet = { kind: 'dapp', request: { ...sheet.request, held } };
	}

	/** Close the dApp sheet of `rid`, if it is the one open. */
	closeDapp(rid: string): void {
		if (this.sheet?.kind === 'dapp' && this.sheet.request.rid === rid) this.sheet = null;
	}
}

export const multisigUi = new MultisigUi();
