/**
 * Which multisig sheet is open. One at a time; the host
 * (`MultisigHost.svelte`) draws it, and the switcher, the home panel and the
 * wallet page's own doors open them.
 */
export type MultisigSheet =
	| { kind: 'create' }
	| { kind: 'add' }
	| { kind: 'propose'; assetId?: string }
	| { kind: 'proposal'; chainId: number; safeTxHash: string }
	| { kind: 'invite' };

class MultisigUi {
	sheet = $state<MultisigSheet | null>(null);

	open(sheet: MultisigSheet): void {
		this.sheet = sheet;
	}

	close(): void {
		this.sheet = null;
	}
}

export const multisigUi = new MultisigUi();
