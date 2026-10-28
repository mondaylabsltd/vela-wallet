/**
 * A multisig transaction's call to another contract (a dApp's), read the way
 * the signing sheet reads the same call sent by a site (cs-12): the
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
import { localizedTerms, requestBlocks } from '$lib/signing/live';
import type { SigningMessages } from '$lib/signing/messages';
import type { Block } from '$lib/signing/model';

/** The call a SafeTx makes: `value` in base units, decimal (the core's `Action::Call`). */
export interface InnerCall {
	to: string;
	value: string;
	data: string;
}

/**
 * The call as a site's `eth_sendTransaction` would carry it — the params both
 * machines read. The value goes as a hex quantity, the way a site sends it
 * (the core reads a value that is not hex as unreadable, RC4).
 */
export function callParamsJson(call: InnerCall): string {
	let value: string;
	try {
		value = `0x${BigInt(call.value || '0').toString(16)}`;
	} catch {
		value = call.value;
	}
	return JSON.stringify([{ to: call.to, data: call.data || '0x', value }]);
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
function requestOf(chainId: number, multisig: string, paramsJson: string): SignView {
	return {
		surface: 'sheet',
		request: {
			id: `multisig:${chainId}:${multisig}`,
			method: 'eth_sendTransaction',
			kind: 'transaction',
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
		tracker_withdraw: null,
		failure_refused: false,
		failure_retryable: false,
		failure_refusal_key: null,
		failure_not_sent: false,
		sim_checking: false,
		sim_waited_out_key: null,
		notice: null,
		blocked: null,
		global_chain_id: chainId
	};
}

/** One call, read by both machines; one per proposal shown, disposed with it. */
export class CallReading {
	clear = $state<ClearSigningView>(INITIAL_CLEAR_VIEW);
	guard = $state<GuardView>(INITIAL_GUARD_VIEW);
	readonly paramsJson: string;

	#clear: ClearSigningSession | null = null;
	#guard: ApprovalGuardSession | null = null;
	#disposed = false;

	constructor(
		readonly chainId: number,
		readonly multisig: string,
		readonly call: InnerCall
	) {
		this.paramsJson = callParamsJson(call);
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
		const method = 'eth_sendTransaction';
		this.#clear.start(
			txKickoff(method, this.paramsJson, this.chainId, toClearLocale(resolvedFormatKeys()))
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
	reading: Pick<CallReading, 'chainId' | 'multisig' | 'paramsJson' | 'clear' | 'guard'>,
	m: SigningMessages,
	shown: CurrencyView
): Block[] {
	// Not started yet (the core loading): the sheet's own "Loading…", never an
	// empty review that could pass for a call with nothing to say.
	if (!reading.clear.resolved && !reading.clear.resolving) {
		return [{ kind: 'sentence', text: m.loading, tone: 'neutral' }];
	}
	const blocks = requestBlocks({
		sign: requestOf(reading.chainId, reading.multisig, reading.paramsJson),
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
	return localizedTerms(clear, m).result?.intent ?? clear.record_intent;
}
