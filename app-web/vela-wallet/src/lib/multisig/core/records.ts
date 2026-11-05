/**
 * An executed multisig transaction in Activity: which rows it writes is the
 * core's (`execution_rows`, on the proposals machine's `activity_rows`
 * notice — one per transfer, else one for the proposal itself); here they
 * are only worded — a token's symbol and decimals (read from the chain), the
 * proposal's title in the page's words.
 */
import { fillTemplate } from '$lib/i18n/fill';
import type { LocalTransaction } from '$lib/services/transactions-model';
import type { MultisigMessages } from '../messages';
import { actionTitle, memberActionTitle, shortenAddress, tokenInfo, UNREAD_TOKEN } from './display';
import { names } from './names.svelte';
import type { Action } from './kernel';
import { executedRowId } from './row-id';
import { plainUnits } from './units';

/**
 * The page's words for a row's title — the proposal's own (`actionTitle`).
 * The multisig host hands them over ({@link useRecordWords}); until then a
 * cancellation keeps its fallback title and anything else its kind's.
 */
let words: MultisigMessages | null = null;
let cancelTitle = 'Cancel transaction #{{nonce}}';

/** The page's words for the rows' titles. */
export function useRecordWords(m: MultisigMessages): void {
	words = m;
	if (m.action.cancel) {
		cancelTitle = m.action.cancel;
		void rewordCancelRows(m.action.cancel).catch(() => {});
	}
	void rewordMemberRows(m).catch(() => {});
}

/** A member change: its rows are titled by the core's one rule (r12-8). */
const isMemberChange = (action: Action | null): boolean =>
	action?.kind === 'addOwner' ||
	action?.kind === 'removeOwner' ||
	action?.kind === 'swapOwner' ||
	action?.kind === 'changeThreshold';

/** r12-8: the name a row gives a member — the accounts machine's (`names.label`). */
const label = (address: string): string => names.label(address);

/**
 * r12-8: every executed member change's row titled again by the core's one
 * rule (`names.change`) in `m`'s language and with the names as they stand
 * now — the sheet's title, on Activity too. A row is rewritten only when
 * its title changed. The rows written before r12 carry no action and keep
 * theirs.
 */
export function rewordMemberRows(m: MultisigMessages): Promise<number> {
	// One at a time, in the order asked: a language switch and a name landing
	// at once must not interleave and leave the older words last.
	const run = rewording.then(() => rewordMemberRowsNow(m));
	rewording = run.catch(() => 0);
	return run;
}

let rewording: Promise<number> = Promise.resolve(0);

async function rewordMemberRowsNow(m: MultisigMessages): Promise<number> {
	const { loadTransactions, updateTransaction } = await import('$lib/services/records');
	let changed = 0;
	for (const row of await loadTransactions()) {
		if (!row.id.startsWith('multisig:') || !row.multisigChange) continue;
		const title = memberActionTitle(m, row.multisigChange as Action, label);
		if (!title || title === row.intent) continue;
		await updateTransaction(row.id, { intent: title });
		changed += 1;
	}
	return changed;
}

/** The fallback a cancellation row was titled with before the page's words were in. */
const FALLBACK_CANCEL = /^Cancel transaction #(\d+)$/;

/**
 * r8-22: a cancellation's row written in English (before the page's words
 * were in, or in another language since) reads in the page's words: the
 * multisig's own rows (`multisig:` ids) whose title is the English fallback
 * are titled again with `template`. Nothing else of a row changes.
 */
export async function rewordCancelRows(template: string): Promise<number> {
	if (template === 'Cancel transaction #{{nonce}}') return 0;
	const { loadTransactions, updateTransaction } = await import('$lib/services/records');
	const rows = (await loadTransactions()).filter(
		(t) => t.id.startsWith('multisig:') && FALLBACK_CANCEL.test(t.intent ?? '')
	);
	for (const row of rows) {
		const nonce = (row.intent ?? '').match(FALLBACK_CANCEL)?.[1] ?? '';
		await updateTransaction(row.id, { intent: fillTemplate(template, { nonce }) });
	}
	return rows.length;
}

/** One row the core planned (`execution_rows`). */
export type PlannedRow =
	| { index: number; kind: 'send'; to: string; token: string | null; amount: string }
	| { index: number; kind: 'call'; to: string; sends: string; calls: number; contract: string };

/** The executed proposal the rows are of, as the notice carries it. */
export interface Executed {
	from: string;
	chainId: number;
	safeTxHash: string;
	nonce: string;
	action: Action | null;
	txHash: string;
	userOpHash: string;
	/** Seconds. */
	timestamp: number;
	rows: PlannedRow[];
}

async function titleOf(e: Executed): Promise<string> {
	if (e.action?.kind === 'cancel' && (!words || words.action.cancel === '')) {
		return fillTemplate(cancelTitle, { nonce: e.nonce });
	}
	if (!words) return '';
	return actionTitle(words, e.chainId, e.action, BigInt(e.nonce), label).catch(() => '');
}

const coin = (chainId: number, token: string | null) =>
	tokenInfo(chainId, token).catch(() => UNREAD_TOKEN);

/** The core's rows of an executed proposal, worded as Activity rows. */
export async function executionRows(e: Executed): Promise<LocalTransaction[]> {
	const out: LocalTransaction[] = [];
	for (const row of e.rows) {
		const base = {
			id: executedRowId(e.chainId, e.safeTxHash, row.index),
			userOpHash: e.userOpHash,
			txHash: e.txHash,
			from: e.from,
			to: row.to,
			chainId: e.chainId,
			timestamp: e.timestamp,
			status: 'confirmed' as const
		};
		if (row.kind === 'send') {
			const info = await coin(e.chainId, row.token);
			// r8-2: unknown decimals are kept raw (base units, decimals 0), never
			// read as 18; the token is named by its contract when it said nothing.
			out.push({
				...base,
				value: info.decimals === null ? row.amount : plainUnits(row.amount, info.decimals),
				symbol: info.symbol || (row.token ? shortenAddress(row.token) : ''),
				decimals: info.decimals ?? 0,
				// Round 5: which token it was, for Activity's lookalike note.
				...(row.token ? { token: row.token } : {}),
				type: 'send'
			});
			continue;
		}
		const sends = BigInt(row.sends || '0');
		const info = sends > 0n ? await coin(e.chainId, null) : { symbol: '', decimals: 18 };
		const places = info.decimals ?? 18;
		out.push({
			...base,
			value: sends > 0n ? plainUnits(row.sends, places) : '0',
			symbol: info.symbol,
			decimals: places,
			type: 'dapp_tx',
			intent: await titleOf(e),
			...(isMemberChange(e.action) ? { multisigChange: e.action } : {}),
			dappSummary: { action: 'call', calls: row.calls, contract: row.contract }
		});
	}
	return out;
}
