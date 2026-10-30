/**
 * A multisig transaction's call to another contract (a dApp's) — or a batch
 * of calls, every one of them — read the way the signing sheet reads the
 * same call or `wallet_sendCalls` sent by a site (cs-12): the
 * `clear_signing` machine says what it means, the `approval_guard` machine
 * what it grants, and the sheet's own block builder (`requestBlocks`) draws
 * both. Every member who opens the proposal sees that one review — never only
 * "Contract call".
 *
 * Nothing is capped here. The members approve the call as proposed (another
 * cap would be another transaction, with another safeTxHash), so the guard
 * runs read-only and its cap editor is drawn as what the call grants: the
 * cap it asks for, in the danger tone when that is unlimited, with the
 * guard's own warning under it.
 */
import { loadCore } from '$lib/core/client';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignView } from '$lib/core/generated/SignView';
import { resolvedFormatKeys } from '$lib/services/locale-format';
import { currency } from '$lib/settings/core/currency.svelte';
import {
	createApprovalGuardSession,
	type ApprovalGuardSession
} from '$lib/signing/core/guard-session';
import {
	createClearSigningSession,
	type ClearSigningSession
} from '$lib/signing/core/clear-session';
import { toClearLocale } from '$lib/signing/core/clear-types';
import { INITIAL_CLEAR_VIEW, INITIAL_GUARD_VIEW } from '$lib/signing/core/sheet.svelte';
import { txKickoff } from '$lib/signing/core/tx-params';
import { localizedTerms, readBy, requestBlocks } from '$lib/signing/live';
import type { SigningMessages } from '$lib/signing/messages';
import type { Block } from '$lib/signing/model';
import type { Action } from './kernel';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';

/** The call a SafeTx makes: `value` in base units, decimal (the core's `Action::Call`). */
export interface InnerCall {
	to: string;
	value: string;
	data: string;
}

/** One of the core's pure multisig rules (`multisig_rules::calls`). */
function rule<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigLinkCall(method, JSON.stringify(args))) as T;
}

/**
 * The calls of `action` the signing sheet's machines read — the core's
 * (`calls.readable`): a call to another contract, or every call of a batch
 * that is more than transfers (one of transfers alone keeps its own lines).
 * `null`: nothing for them to read.
 */
export function readableCalls(action: Action | null): InnerCall[] | null {
	return rule<InnerCall[] | null>('calls.readable', { action });
}

/**
 * §3.6: the calls the one review reads as the signing sheet would — the
 * core's (`calls.reviewed`): every call and every transfer, alone or in a
 * batch, as a site would have sent it from the multisig. A member change,
 * a rename or a cancellation is the multisig's own reading (`null`).
 */
export function reviewedCalls(
	action: Action | null,
	tx: { to: string; value: string; data: string }
): InnerCall[] | null {
	return rule<InnerCall[] | null>('calls.reviewed', { action, tx });
}

/** A value as a site sends one: a hex quantity (the core reads one that is not hex as unreadable, RC4). */
function hexValue(value: string): string {
	try {
		return `0x${BigInt(value || '0').toString(16)}`;
	} catch {
		return value;
	}
}

/**
 * The call as a site's `eth_sendTransaction` would carry it — the params both
 * machines read.
 */
export function callParamsJson(call: InnerCall): string {
	return JSON.stringify([{ to: call.to, data: call.data || '0x', value: hexValue(call.value) }]);
}

/**
 * The calls as a site's `wallet_sendCalls` (EIP-5792) would carry them, from
 * the multisig on `chainId`, atomically: the params both machines read for a
 * batch, every call of it.
 */
export function batchParamsJson(calls: InnerCall[], chainId: number, multisig: string): string {
	return JSON.stringify([
		{
			version: '2.0.0',
			chainId: `0x${chainId.toString(16)}`,
			from: multisig,
			atomicRequired: true,
			calls: calls.map((c) => ({ to: c.to, data: c.data || '0x', value: hexValue(c.value) }))
		}
	]);
}

/** No fee belongs to a member's review: the executor pays, on its own sheet. */
const NO_FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: false,
	no_coin_pays: false,
	nothing_to_pay_from: false,
	provisional: false,
	failure: null
};

/**
 * The request the sheet's builder reads the call from: the multisig's own
 * transaction to `call.to`, nothing refused (the core's multisig review has
 * already ruled on what a multisig signs), no site.
 */
function requestOf(
	chainId: number,
	multisig: string,
	method: string,
	paramsJson: string
): SignView {
	return {
		surface: 'sheet',
		request: {
			id: `multisig:${chainId}:${multisig}`,
			method,
			kind: method === 'wallet_sendCalls' ? 'batch' : 'transaction',
			params_json: paramsJson,
			origin: '',
			dapp: null,
			chain_id: chainId,
			signer_address: multisig,
			first_party: false
		},
		is_signing: false,
		is_submitting: false,
		phase: 'idle',
		pending_op_hash: null,
		pending_op_maybe_sent: false,
		error: null,
		funding: null,
		confirm_gate_open: false,
		confirm_block: null,
		reconcile_pending: false,
		swipe_action: 'none',
		tracker_handoff: null,
		landing_closed: false,
		tracker_withdraw: null,
		failure_refused: false,
		failure_retryable: false,
		failure_refusal_key: null,
		failure_not_sent: false,
		passkey_note: null,
		sim_checking: false,
		sim_waited_out_key: null,
		notice: null,
		blocked: null,
		global_chain_id: chainId
	};
}

/** One call (or one batch), read by both machines; one per proposal shown, disposed with it. */
export class CallReading {
	clear = $state<ClearSigningView>(INITIAL_CLEAR_VIEW);
	guard = $state<GuardView>(INITIAL_GUARD_VIEW);
	/** `eth_sendTransaction` for one call, `wallet_sendCalls` for a batch. */
	readonly method: string;
	readonly paramsJson: string;

	#clear: ClearSigningSession | null = null;
	#guard: ApprovalGuardSession | null = null;
	#disposed = false;

	constructor(
		readonly chainId: number,
		readonly multisig: string,
		calls: InnerCall | InnerCall[]
	) {
		const list = Array.isArray(calls) ? calls : [calls];
		const batch = list.length > 1;
		this.method = batch ? 'wallet_sendCalls' : 'eth_sendTransaction';
		this.paramsJson = batch
			? batchParamsJson(list, chainId, multisig)
			: callParamsJson(list[0] ?? { to: '', value: '0', data: '0x' });
	}

	/** Read it. Browser-only (loads the core); a reading disposed meanwhile stays quiet. */
	async start(): Promise<void> {
		void currency.boot();
		await loadCore();
		if (this.#disposed) return;
		this.#clear = createClearSigningSession({
			onView: (view) => (this.clear = view),
			onError: (error) => console.error('[clear_signing] core fault:', error)
		});
		this.#guard = createApprovalGuardSession({
			onView: (view) => (this.guard = view),
			onError: (error) => console.error('[approval_guard] core fault:', error)
		});
		const method = this.method;
		this.#clear.start(
			txKickoff(method, this.paramsJson, this.chainId, toClearLocale(resolvedFormatKeys()), {
				// The multisig signs it: a call into itself — Safe's own member
				// management — is read by Vela from the bytes, never named from
				// the public function-name database (r4a-15).
				from: this.multisig
			})
		);
		this.#guard.start({
			type: 'approval_detected',
			method,
			params_json: this.paramsJson,
			chain_id: this.chainId,
			// Whose allowance an increase adds to: the multisig's.
			wallet_address: this.multisig,
			read_only: true,
			now_ms: Date.now()
		});
	}

	dispose(): void {
		this.#disposed = true;
		this.#clear?.dispose();
		this.#guard?.dispose();
		this.#clear = null;
		this.#guard = null;
	}
}

/**
 * The review's blocks: the signing sheet's, for this call. The cap editor
 * becomes the row it would open on (what the call grants, in its tone), with
 * its notes — a member has no cap to choose.
 */
export function callReviewBlocks(
	reading: Pick<CallReading, 'chainId' | 'multisig' | 'method' | 'paramsJson' | 'clear' | 'guard'>,
	m: SigningMessages,
	shown: CurrencyView
): Block[] {
	// Not started yet (the core loading): the sheet's own "Loading…", never an
	// empty review that could pass for a call with nothing to say.
	if (!reading.clear.resolved && !reading.clear.resolving) {
		return [{ kind: 'sentence', text: m.loading, tone: 'neutral' }];
	}
	const blocks = requestBlocks({
		sign: requestOf(reading.chainId, reading.multisig, reading.method, reading.paramsJson),
		clear: reading.clear,
		guard: reading.guard,
		fee: NO_FEE,
		currency: shown,
		m,
		identity: { name: '', address: reading.multisig, identiconSvg: '' },
		identicon: () => ''
	});
	return blocks.flatMap((block): Block[] => {
		if (block.kind !== 'allowance') return [block];
		const granted: Block[] = [
			{
				kind: 'rows',
				rows: [
					{ label: block.label, value: block.value, valueTone: block.valueTone },
					...(block.resultingTotal ? [block.resultingTotal] : [])
				]
			}
		];
		if (block.note && !blocks.some((b) => b.kind === 'warning' && b.text === block.note)) {
			granted.push({ kind: 'warning', tone: 'caution', text: block.note });
		}
		return granted;
	});
}

/**
 * The proposal's one line once the call is read: the verb the core would
 * title its record with (`record_intent` — none for a best-effort guess or a
 * call nobody could read), in the reader's words; `null` keeps "Contract call".
 */
export function callTitle(clear: ClearSigningView, m: SigningMessages): string | null {
	if (clear.resolving || !clear.resolved || clear.record_intent === null) return null;
	const said = localizedTerms(clear, m);
	if (said.result) return said.result.intent;
	// A plain coin send has nothing decoded: its word is the confirm's term
	// (`intentSend`), in the reader's words — the record's English "Send" was
	// the zh review's heading (r5reg-10).
	const term = clear.confirm.type === 'confirm_intent' ? clear.confirm.intent_term : null;
	if (term && m.terms[term] && said.confirm.type === 'confirm_intent') return said.confirm.intent;
	return clear.record_intent;
}

/**
 * Who read the call its title names (`callTitle`), as a row says it beside
 * it: "Read by Vela, from the transaction", "Read by a fetched description
 * (host)" — `null` while there is no title.
 */
export function callSource(clear: ClearSigningView, m: SigningMessages): string | null {
	const source = clear.result?.reading.source;
	if (callTitle(clear, m) === null || source === undefined) return null;
	return `${m.certain.labelSource} ${readBy(source, m)}`;
}

/**
 * Read one call to its end — for a row that names it (Activity), not a
 * sheet that follows it: a title and its source, or `null` when the call
 * could not be read in `ms` (the row keeps "Contract call").
 */
export async function readCall(
	chainId: number,
	multisig: string,
	call: InnerCall,
	m: SigningMessages,
	ms = 15_000
): Promise<{ title: string; source: string | null } | null> {
	const reading = new CallReading(chainId, multisig, call);
	try {
		await reading.start();
		const deadline = Date.now() + ms;
		while ((!reading.clear.resolved || reading.clear.resolving) && Date.now() < deadline)
			await new Promise((done) => setTimeout(done, 100));
		const title = callTitle(reading.clear, m);
		return title === null ? null : { title, source: callSource(reading.clear, m) };
	} finally {
		reading.dispose();
	}
}
