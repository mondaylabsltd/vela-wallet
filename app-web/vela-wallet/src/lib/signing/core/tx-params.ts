/**
 * What the sheet tells the clear-signing core about a transaction request —
 * read the way the submit path reads the same request (spec 082 RC6), so the
 * card never describes one call while another is sent.
 */

/** A transaction's three params, as the two machines need them. */
export interface TxParams {
	to: string | null;
	data: string | null;
	value: string | null;
}

/**
 * A field the dApp sent, as TEXT for the core (spec 082 RC6).
 *
 * Absent and JSON `null` are both "not given": the submit path reads each as
 * its default (`value ?? '0x0'`, `data ?? '0x'` in `dapp-submit.ts`), and the
 * desktop and Android readers say `None` for both — so a `{to, value, data:
 * null}` is the plain send it is sent as, not a blind card with no amount.
 *
 * Any other present non-string (a JSON number) reaches the core as its own
 * text, which the core refuses to print as an amount — never as absent,
 * which would read as a calm "0" while the submit path reads the number.
 */
export function textOf(value: unknown): string | null {
	if (value === undefined || value === null) return null;
	return typeof value === 'string' ? value : String(value);
}

/**
 * The call the sheet describes: `eth_sendTransaction`'s one transaction, or
 * the FIRST leg of a `wallet_sendCalls` batch (`params[0].calls[0]`) — the
 * rule every rung applies to leg 1 (spec 082 RC7).
 */
export function txParams(paramsJson: string): TxParams | null {
	try {
		const params = JSON.parse(paramsJson) as unknown[];
		const first = params[0] as Record<string, unknown> | undefined;
		if (!first || typeof first !== 'object') return null;
		const calls = (first as { calls?: unknown }).calls;
		const tx = (Array.isArray(calls) ? calls[0] : first) as Record<string, unknown> | undefined;
		if (!tx || typeof tx !== 'object') return null;
		return { to: textOf(tx.to), data: textOf(tx.data), value: textOf(tx.value) };
	} catch {
		return null;
	}
}
