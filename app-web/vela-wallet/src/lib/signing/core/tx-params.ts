/**
 * What the sheet tells the clear-signing core about a transaction request —
 * read the way the submit path reads the same request (spec 082 RC6), so the
 * card never describes one call while another is sent.
 */

import type { ClearLocale } from '$lib/core/generated/ClearLocale';
import type { ClearSigningEvent } from '$lib/core/generated/ClearSigningEvent';

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
 * The ONE call of an `eth_sendTransaction`: `params[0]`'s own `to`, `data`
 * and `value` — the fields the submit path sends (`dapp-submit.ts`
 * `handleSendTransaction`). Never a `calls` key beside them: a stray
 * `"calls":[{…harmless…}]` next to a malicious top-level call once had the
 * harmless one described while the malicious one was signed (the desktop's
 * 083 review, now every shell's rule). A batch is not read here at all —
 * the core reads every call of it (`resolve_batch`, 089 S1).
 */
export function txParams(paramsJson: string): TxParams | null {
	try {
		const params = JSON.parse(paramsJson) as unknown[];
		const tx = params[0] as Record<string, unknown> | undefined;
		if (!tx || typeof tx !== 'object') return null;
		return { to: textOf(tx.to), data: textOf(tx.data), value: textOf(tx.value) };
	} catch {
		return null;
	}
}

/**
 * What the sheet asks the clear-signing core about an on-chain request,
 * chosen by METHOD — the way the submit path chooses what it sends. A
 * `wallet_sendCalls` goes over whole: the core reads EVERY call (089 S1), so
 * the sheet can never describe call 1 while signing them all.
 */
export function txKickoff(
	method: string,
	paramsJson: string,
	chainId: number,
	locale: ClearLocale
): ClearSigningEvent {
	if (method === 'wallet_sendCalls') {
		return { type: 'resolve_batch', params_json: paramsJson, chain_id: chainId, locale };
	}
	const tx = txParams(paramsJson);
	return {
		type: 'resolve_transaction',
		to: tx?.to ?? null,
		data: tx?.data ?? null,
		value: tx?.value ?? null,
		chain_id: chainId,
		locale
	};
}
