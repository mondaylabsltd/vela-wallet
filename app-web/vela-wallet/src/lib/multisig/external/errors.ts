/**
 * r4C-11: what a wallet's refusal means, in words a person acts on — never
 * its raw text ("Unauthorized", a Ledger's escaped JSON). The same reading
 * on both sides: Vela's sheet (the error comes back over WalletPair with its
 * EIP-1193 code) and the connect page (straight from the wallet).
 */

export type WalletProblem =
	/** 4001: the person said no in the wallet. */
	| { kind: 'declined' }
	/** 4100: the account is not connected to this page in the wallet. */
	| { kind: 'unauthorized' }
	/** 4200 / -32601: the wallet cannot do this (typed data, say). */
	| { kind: 'unsupported' }
	/** 4901: the wallet is on another network. */
	| { kind: 'otherChain' }
	/** 4900: the wallet (or the page) went away. */
	| { kind: 'disconnected' }
	/** Anything else: its own words, short. */
	| { kind: 'unknown'; reason: string };

const MAX_REASON = 120;

/**
 * A wallet's message cut to one readable line: JSON and escapes undone,
 * "Error:" prefixes dropped, at most {@link MAX_REASON} characters.
 */
export function shortReason(message: string): string {
	let text = message;
	try {
		const parsed = JSON.parse(text) as unknown;
		if (typeof parsed === 'string') text = parsed;
		else if (parsed && typeof parsed === 'object' && 'message' in parsed) {
			text = String((parsed as { message: unknown }).message);
		}
	} catch {
		// not JSON: as it is
	}
	text = text
		.replace(/\\n/g, ' ')
		.replace(/\\"/g, '"')
		.replace(/^(?:\s*(?:Error|WalletError|CallError):\s*)+/i, '')
		.split(/\r?\n/)[0]
		.replace(/\s+/g, ' ')
		.trim();
	return text.length > MAX_REASON ? `${text.slice(0, MAX_REASON - 1)}…` : text;
}

/** How a wallet's error reads; `code` is the EIP-1193 one when there is one. */
export function walletProblem(error: unknown): WalletProblem {
	const e = error as { code?: unknown; message?: unknown } | null;
	const code = typeof e?.code === 'number' ? e.code : null;
	const message = typeof e?.message === 'string' ? e.message : String(error ?? '');
	if (code === 4001 || /user (rejected|denied|cancel)/i.test(message)) return { kind: 'declined' };
	if (code === 4100 || /^unauthori[sz]ed\b/i.test(message.trim())) return { kind: 'unauthorized' };
	if (
		code === 4200 ||
		code === -32601 ||
		/not (supported|implemented)|unsupported/i.test(message)
	) {
		return { kind: 'unsupported' };
	}
	if (code === 4901) return { kind: 'otherChain' };
	if (code === 4900) return { kind: 'disconnected' };
	return { kind: 'unknown', reason: shortReason(message) || String(code ?? '') };
}

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
