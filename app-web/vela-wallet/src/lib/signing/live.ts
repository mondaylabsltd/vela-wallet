/**
 * The signing sheet, built from the cores (spec 026 T242).
 *
 * Four machines answer one screen: `sign_request` owns the request and the
 * gate, `clear_signing` owns what the request MEANS, `approval_guard` owns the
 * cap, `fee_policy` owns the number. This file turns their four views into the
 * one drawn `SigningModel` — the 13 block kinds spec 022 drew — and decides
 * nothing.
 *
 * Two rules are load-bearing and are asserted in the tests beside this file:
 *
 * 1. **The confirm gate is an AND.** `SignView.confirm_gate_open` says the
 *    request may be signed; `GuardView.confirm_allowed` says the cap has been
 *    chosen. The slider arms only when both are true (and the fee, when the
 *    request has one, is ready). Its own doc in the drawn component says the
 *    shell must AND them — this is that place.
 * 2. **Dismissal is rejection.** The 022 interaction contract draws no reject
 *    button: closing the sheet IS the refusal, and the route answers the
 *    requester with 4001.
 */
import type { ClearSignField } from '$lib/core/generated/ClearSignField';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignView } from '$lib/core/generated/SignView';
import { chainName } from '$lib/services/networks';
import { shortenAddress } from '$lib/wallet/identity';
import type { WalletIdentity } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import type { SigningMessages } from './messages';
import type {
	AllowanceChip,
	AmountLine,
	Block,
	FeeModel,
	KeyValueRow,
	SigningModel,
	TechModel,
	Tone
} from './model';

export interface SigningLiveInputs {
	sign: SignView;
	clear: ClearSigningView;
	guard: GuardView;
	fee: FeeView;
	m: SigningMessages;
	identity: WalletIdentity;
	identicon: (seed: string) => string;
}

/** The core's risk grade, in the drawn vocabulary. */
function toneOf(risk: 'safe' | 'normal' | 'caution' | 'danger'): Tone {
	switch (risk) {
		case 'safe':
			return 'success';
		case 'caution':
			return 'caution';
		case 'danger':
			return 'danger';
		case 'normal':
			return 'neutral';
	}
}

function letterOf(name: string): string {
	return (name.trim()[0] ?? '?').toUpperCase();
}

/** A decoded field as the row it draws, keeping the core's flags verbatim. */
function fieldRow(field: ClearSignField): KeyValueRow {
	return {
		label: field.label,
		value: field.value,
		valueTone: field.warning ? 'danger' : field.unverified ? 'caution' : undefined,
		mono: field.address !== null || field.token_address !== null
	};
}

/** The amount a decoded field carries, when it is the one the eye should land on. */
function amountLine(field: ClearSignField, outgoing: boolean): AmountLine {
	return {
		sign: outgoing ? '-' : '+',
		value: field.value,
		symbol: '',
		fiat: field.usd_value === null ? undefined : `≈ $${field.usd_value.toFixed(2)}`,
		tone: field.warning ? 'danger' : outgoing ? 'neutral' : 'success'
	};
}

/**
 * The allowance editor's chips.
 *
 * The never-unlimited mandate is the core's: an unbounded request offers its
 * `requested` chip DISABLED and hands back no choice, which is what keeps the
 * slider shut until a finite cap is picked. The words are the corpus's; which
 * chip is selectable is `GuardView`'s.
 */
function allowanceChips(guard: GuardView, m: SigningMessages): AllowanceChip[] {
	const editor = guard.editor;
	if (!editor) return [];
	const state = (mode: string): AllowanceChip['state'] => {
		if (editor.mode === mode) return 'selected';
		// An unbounded request cannot be granted as-is: its own chip is dead
		// until the person deliberately chooses to grant it.
		if (mode === 'requested' && !editor.requested_finite) return 'disabled';
		if (mode === 'balance' && !editor.has_balance_cap) return 'disabled';
		return 'idle';
	};
	const chips: AllowanceChip[] = [
		{ id: 'requested', label: m.chipRequested, state: state('requested') },
		{ id: 'balance', label: m.chipBalance, state: state('balance') },
		{ id: 'custom', label: m.chipCustom, state: state('custom') },
		{ id: 'revoke', label: m.chipRevoke, state: state('revoke') }
	];
	return chips;
}

function guardBlock(guard: GuardView, m: SigningMessages): Block | null {
	if (guard.surface !== 'approval_editor' || !guard.editor) return null;
	const editor = guard.editor;
	const symbol = guard.meta.loading ? '…' : guard.meta.symbol;
	return {
		kind: 'allowance',
		label: fill(m.labelSpendingCap, { symbol }),
		value: editor.display_amount_raw ?? m.valueUnlimited,
		valueTone: editor.requested_finite ? 'neutral' : 'danger',
		chips: allowanceChips(guard, m),
		note: guard.decimals_unverified ? m.warnUnverifiedAmount : undefined,
		resultingTotal:
			guard.increase_total === null || guard.increase_total.total === null
				? undefined
				: { label: m.labelResultingTotal, value: guard.increase_total.total },
		// The field appears only on the chip that needs one, and it carries the
		// CORE's text: a keystroke the machine rejected must not sit on screen
		// as though it had been taken.
		custom:
			editor.mode === 'custom'
				? {
						value: editor.custom_text,
						symbol,
						placeholder: '0',
						error:
							editor.error === 'invalid_amount'
								? m.invalidAmount
								: editor.error === 'unlimited_disabled'
									? m.unlimitedDisabled
									: undefined
					}
				: undefined
	};
}

/**
 * The request as blocks.
 *
 * The order is the drawn one: what it does, then how much, then to whom, then
 * every warning the core raised, then the facts. A rung further down the
 * ladder simply emits more warnings and fewer decoded rows — the ladder is the
 * core's, and this reads it rather than re-deriving it.
 */
/** The calldata's length in bytes — what the two "unable to decode" lines name. */
function calldataBytes(paramsJson: string): number {
	try {
		const params = JSON.parse(paramsJson) as unknown[];
		const data = (params[0] as { data?: string } | undefined)?.data;
		if (typeof data !== 'string') return 0;
		return Math.max(0, Math.floor((data.replace(/^0x/, '').length || 0) / 2));
	} catch {
		return 0;
	}
}

function blocksFor(inputs: SigningLiveInputs): Block[] {
	const { sign, clear, guard, m } = inputs;
	const bytes = calldataBytes(sign.request?.params_json ?? '[]');
	const blocks: Block[] = [];

	if (clear.surface === 'loading' || clear.resolving) {
		blocks.push({ kind: 'sentence', text: m.choosePrompt, tone: 'neutral' });
		return blocks;
	}

	const result = clear.result;
	if (result) {
		blocks.push({ kind: 'intent', text: result.intent, tone: toneOf(result.risk) });

		const send = result.fields.find((f) => f.role === 'send_amount');
		const receive = result.fields.find((f) => f.role === 'receive_amount');
		if (send && receive) {
			blocks.push({
				kind: 'swap',
				pay: amountLine(send, true),
				receive: amountLine(receive, false)
			});
		} else if (send) {
			blocks.push({ kind: 'amount', line: amountLine(send, true) });
		} else if (receive) {
			blocks.push({ kind: 'amount', line: amountLine(receive, false) });
		}

		for (const field of result.fields) {
			if (field.role !== 'recipient' && field.role !== 'spender') continue;
			blocks.push({
				kind: 'party',
				label: field.label,
				name: field.value,
				address: field.address ?? undefined,
				badge: field.unverified ? { text: m.tagUnverified, tone: 'caution' } : undefined
			});
		}

		// The guard's editor sits with the approval it caps.
		const allowance = guardBlock(guard, m);
		if (allowance) blocks.push(allowance);

		// Whatever the core flagged, said once, in its own words.
		if (result.to_own_token) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnDrain });
		}
		if (!result.verified) {
			blocks.push({
				kind: 'warning',
				tone: 'caution',
				text: fill(m.warnSelectorNotListed, { bytes })
			});
		}
		if (result.partial) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnBestEffort });
		}
		if (result.best_effort) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.summaryBestEffort });
		}

		const rest = result.fields.filter((f) => f.role === 'generic' && f !== send && f !== receive);
		if (rest.length > 0) blocks.push({ kind: 'rows', rows: rest.map(fieldRow) });
		return blocks;
	}

	// No decode at all — the deepest rung. The core said so; the sheet says so.
	if (clear.surface === 'blind_transaction' || clear.surface === 'blind_typed_data') {
		blocks.push({ kind: 'intent', text: m.intentBlind, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: fill(m.warnBlindDecode, { bytes }) });
		return blocks;
	}

	if (clear.surface === 'eth_sign') {
		blocks.push({ kind: 'intent', text: m.warnEthSign, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: m.bodyEthSign });
		return blocks;
	}

	if (clear.surface === 'message_sign' && clear.message) {
		const message = clear.message;
		blocks.push({ kind: 'intent', text: m.intentMessage, tone: 'neutral' });
		blocks.push({
			kind: 'code',
			lines: (message.decoded_text ?? message.binary_preview ?? message.payload).split('\n'),
			note: message.non_printable ? m.warnHexMessage : undefined
		});
		if (message.binding === 'mismatch') {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnSiweMismatch });
		}
		return blocks;
	}

	return blocks;
}

/** The fee, in the shape the drawn row renders. Off-chain requests have none. */
function feeModel(inputs: SigningLiveInputs): FeeModel {
	const { sign, fee, m } = inputs;
	const kind = sign.request?.kind;
	if (kind === 'personal_sign' || kind === 'typed_data') {
		return { kind: 'offchain', note: m.okNoNetworkFee };
	}
	if (!fee.fee) return { kind: 'hidden' };
	const asset = fee.fee.fee_asset;
	const value =
		asset.type === 'erc20'
			? `${Number(asset.amount) / 10 ** asset.decimals} ${asset.symbol ?? ''}`.trim()
			: `${Number(fee.fee.total_wei) / 1e18} ${chainName(fee.fee.chain_id)}`;
	return { kind: 'onchain', label: m.feeLabel, value };
}

function techModel(inputs: SigningLiveInputs): TechModel {
	const { sign, clear, m } = inputs;
	const request = sign.request;
	const result = clear.result;
	return {
		title: m.advancedToggle,
		summary: result?.contract_name ?? undefined,
		fn: undefined,
		params: [],
		identities: result?.contract_address
			? [
					{
						role: m.labelInteracting,
						name: result.contract_name ?? shortenAddress(result.contract_address),
						address: result.contract_address
					}
				]
			: [],
		raw: request ? { label: m.techRawData, hex: request.params_json } : undefined,
		copyLabel: m.copyValue,
		explorerLabel: m.viewOnExplorer
	};
}

/**
 * The whole sheet. `null` while the core is showing nothing — the route
 * renders no sheet at all then, rather than an empty one.
 */
export function buildSigningModel(inputs: SigningLiveInputs): SigningModel | null {
	const { sign, clear, guard, fee, m, identity, identicon } = inputs;
	if (sign.surface === 'hidden' || !sign.request) return null;

	const request = sign.request;
	const dapp = request.dapp;
	const name = dapp?.name ?? new URL(request.origin).host;

	// Rule 1: the gate is an AND. The core may allow the request; the guard may
	// still be waiting for a cap; the fee may still be in flight.
	const feeReady = feeModel(inputs).kind !== 'onchain' || fee.confirm_fee_ready;
	const enabled = sign.confirm_gate_open && guard.confirm_allowed && feeReady && !sign.is_signing;

	return {
		id: 'cs1',
		dapp: { name, host: new URL(request.origin).host, letter: letterOf(name), tint: 'neutral' },
		network: { name: chainName(request.chain_id), dot: 'neutral' },
		blocks: blocksFor(inputs),
		tech: techModel(inputs),
		techOpen: false,
		fee: feeModel(inputs),
		signer: {
			label: m.signingAccount,
			name: identity.name,
			identiconSvg: identicon(identity.address),
			address: identity.address
		},
		confirm: {
			hint: m.slideToConfirm,
			/*
			 * The drawn control renders `hint · action`, so `action` is a PHRASE
			 * ("Confirm send"), not a sentence. Falling back to
			 * `slideConfirmAction` put its raw template on screen — the person read
			 * "Slide to confirm · Slide to confirm · {{action}}" (spec 027 T340,
			 * found the first time a real request reached the sheet, and the same
			 * class as 026's `{{bytes}}`). With no intent from the core, the
			 * generic word is the honest one.
			 */
			action: clear.confirm.type === 'confirm_intent' ? clear.confirm.intent : m.confirmPlain,
			enabled
		},
		panelTitle: m.panelTitle
	};
}
