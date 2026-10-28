/**
 * Asking an outside owner's wallet for an approval, from anywhere a multisig
 * needs one (a proposal's approvals, the send flow's confirm). The request
 * is drawn by `ExternalSignSheet` (mounted by the multisig host); the caller
 * awaits the approval, already checked by the core, or a cancel.
 */
import {
	safeTxTypedData,
	verifyEcdsaApproval,
	type EcdsaApproval,
	type SafeTx
} from '../core/kernel';

export interface ExternalRequest {
	chainId: number;
	safe: string;
	tx: SafeTx;
	safeTxHash: string;
	/** Who may sign: the owners that have not approved and are not on this device. */
	owners: string[];
	resolve(approval: EcdsaApproval): void;
	reject(error: unknown): void;
}

class ExternalSigning {
	request = $state<ExternalRequest | null>(null);

	/**
	 * An approval of `tx` by one of `owners`, from the wallet that holds it.
	 * Rejects with an `AbortError` when the person cancels.
	 */
	approve(input: Omit<ExternalRequest, 'resolve' | 'reject'>): Promise<EcdsaApproval> {
		this.request?.reject(new DOMException('Superseded', 'AbortError'));
		return new Promise<EcdsaApproval>((resolve, reject) => {
			this.request = {
				...input,
				resolve: (a) => {
					this.request = null;
					resolve(a);
				},
				reject: (e) => {
					this.request = null;
					reject(e);
				}
			};
		});
	}
}

export const externalSigning = new ExternalSigning();

/** The typed data the wallet is asked to sign for a request. */
export function typedDataOf(r: Pick<ExternalRequest, 'chainId' | 'safe' | 'tx'>) {
	return safeTxTypedData(r.chainId, r.safe, r.tx);
}

/** A wallet's answer, as the Safe will read it, or `null` if it is not an owner's for this hash. */
export function checked(
	safeTxHash: string,
	owners: string[],
	answer: { owner: string; signature: string }
): EcdsaApproval | null {
	try {
		const approval = verifyEcdsaApproval(safeTxHash, answer);
		return owners.some((o) => o.toLowerCase() === approval.owner.toLowerCase()) ? approval : null;
	} catch {
		return null;
	}
}
