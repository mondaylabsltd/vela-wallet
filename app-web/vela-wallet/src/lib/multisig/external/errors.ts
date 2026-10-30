/**
 * r4C-11: what a wallet's refusal means, said in words a person acts on —
 * never its raw text ("Unauthorized", a Ledger's escaped JSON). What it means
 * is the core's reading (`multisig_outside::wallet_problem`, the same on both
 * sides: Vela's sheet and the connect page); this says it.
 */
import type { OutsideWalletProblem } from '../generated/OutsideWalletProblem';

export type WalletProblem = OutsideWalletProblem;

/** The words for each problem (the multisig catalog's `external` group). */
export interface ProblemWords {
	walletDeclined: string;
	walletUnauthorized: string;
	walletUnsupported: string;
	otherChain: string;
	walletGone: string;
	/** `{{wallet}}`, `{{reason}}`. */
	walletFailed: string;
}

/** A problem said: plain, with what to do next. */
export function problemText(
	p: WalletProblem,
	w: ProblemWords,
	fill: (template: string, values: Record<string, string>) => string,
	wallet: string,
	chain: string
): string {
	switch (p.kind) {
		case 'declined':
			return w.walletDeclined;
		case 'unauthorized':
			return w.walletUnauthorized;
		case 'unsupported':
			return fill(w.walletUnsupported, { wallet });
		case 'otherChain':
			return fill(w.otherChain, { chain });
		case 'disconnected':
			return fill(w.walletGone, { wallet });
		case 'unknown':
			return fill(w.walletFailed, { wallet, reason: p.reason });
	}
}
