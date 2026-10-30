/**
 * Asking an outside owner's wallet, from anywhere a multisig needs it (a
 * proposal's approvals, the send flow's confirm, the proposals machine's
 * `Notice::OutsideSign`). The request is drawn by `ExternalSignSheet`
 * (mounted by the multisig host), which runs the core's session machine
 * (`shell/outside-session.svelte.ts`); the caller awaits how the ask ended —
 * an approval already verified by the core, a transaction sent, or a cancel.
 */
import type { EcdsaApproval, SafeTx } from '../core/kernel';
import type { OutsideSendTx } from '../generated/OutsideSendTx';
import type { OutsideSessionOutcome } from '../generated/OutsideSessionOutcome';
import type { OutsideSessionRequest } from '../generated/OutsideSessionRequest';

export interface ExternalRequest {
	chainId: number;
	safe: string;
	tx: SafeTx;
	safeTxHash: string;
	/** Who may sign: the owners that have not approved and are not on this device. */
	owners: string[];
	/** Send this transaction instead of signing (`null`: an approval). */
	send?: OutsideSendTx | null;
	/** How the ask ended, as the session machine says it. */
	settle(outcome: OutsideSessionOutcome): void;
	/** The sheet could not be shown at all. */
	reject(error: unknown): void;
}

class ExternalSigning {
	/** Raw: kept as the very object `open` made, so it is told apart by identity. */
	request = $state.raw<ExternalRequest | null>(null);

	/**
	 * Ask the wallet that holds one of `request.owners`: settles with the
	 * approval (verified by the core), the transaction's hash, or
	 * `cancelled` when the person closes the sheet (or another ask
	 * supersedes this one).
	 */
	open(request: OutsideSessionRequest): Promise<OutsideSessionOutcome> {
		this.request?.settle({ kind: 'cancelled' });
		return new Promise<OutsideSessionOutcome>((resolve, reject) => {
			const mine: ExternalRequest = {
				chainId: request.chainId,
				safe: request.safe,
				tx: request.tx as SafeTx,
				safeTxHash: request.safeTxHash,
				owners: request.owners,
				send: request.send,
				settle: (outcome) => {
					if (this.request === mine) this.request = null;
					resolve(outcome);
				},
				reject: (error) => {
					if (this.request === mine) this.request = null;
					reject(error);
				}
			};
			this.request = mine;
		});
	}

	/**
	 * An approval of `tx` by one of `owners`, from the wallet that holds it.
	 * Rejects with an `AbortError` when the person cancels.
	 */
	async approve(input: Omit<OutsideSessionRequest, 'send'>): Promise<EcdsaApproval> {
		const outcome = await this.open({ ...input, send: null });
		if (outcome.kind === 'approved') return outcome.approval as EcdsaApproval;
		throw new DOMException('Cancelled', 'AbortError');
	}
}

export const externalSigning = new ExternalSigning();

/**
 * The one entry for asking an outside owner's wallet (the proposals
 * machine's `Notice::OutsideSign { request }`): the sheet shown, settled
 * with what the machine's `OutsideAnswered` carries back.
 */
export function openOutsideSession(request: OutsideSessionRequest): Promise<OutsideSessionOutcome> {
	return externalSigning.open(request);
}
