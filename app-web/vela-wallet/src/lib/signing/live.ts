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
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { GuardEditorView } from '$lib/core/generated/GuardEditorView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignView } from '$lib/core/generated/SignView';
import {
	feeAmountText,
	feeLine,
	feeLineParts,
	feeOptionPriceUsd,
	feeParts
} from '$lib/flows/fee-line';
import { offeredTier, speedControlModel } from '$lib/flows/speed-control';
import type { FeeSpeedModel } from '$lib/flows/model';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { chainLogoURL } from '$lib/services/tokens-model';
import { exactAmount, trimBalance } from '$lib/wallet/live';
import { fromBaseUnits } from '$lib/services/eip681';
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
	/** The fee-coin selector is open (live only): the row becomes the list, as on Send. */
	feeOpen?: boolean;
	/** The display currency the fee's "≈" half is written in (issue 201). */
	currency: CurrencyView;
	/**
	 * The speed control (spec 069), as the `fee_speed` core decided it — the
	 * send form's, so the two surfaces choose a speed the same way. Absent
	 * where there is no fee session behind the sheet.
	 */
	speed?: { view: FeeSpeedView; feeOptions(tier: FeeTier): FeeView['options'] };
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

/**
 * The tint of a mark nobody has a brand colour for. It used to be the string
 * `'neutral'`, which is not a colour: the letter disc and the network dot both
 * drew as nothing at all.
 */
const NEUTRAL_TINT = 'var(--color-fg-muted)';

/**
 * Where a site's icon conventionally lives, best first. Only for an `https:`
 * origin: the request is made by the person's own browser, with no referrer
 * (`RemoteLogo`), to a site they are already on — and never over plain http,
 * where anybody on the path could answer with somebody else's brand.
 */
export function siteIconUrls(origin: string): string[] {
	try {
		const url = new URL(origin);
		if (url.protocol !== 'https:') return [];
		return [`${url.origin}/apple-touch-icon.png`, `${url.origin}/favicon.ico`];
	} catch {
		return [];
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
 * Which chip is selectable is `GuardView`'s; the words are the corpus's. An
 * unbounded request opens on its own `requested` chip (the core's 2026-09-26
 * ruling: Permit2 bundles revert when the wallet re-encodes the approve), so
 * the site's ask is what goes out unless the person picks a cap.
 */
function allowanceChips(editor: GuardEditorView, m: SigningMessages): AllowanceChip[] {
	const state = (mode: string): AllowanceChip['state'] => {
		if (editor.mode === mode) return 'selected';
		// A request of 0 has no amount of its own to keep.
		if (mode === 'requested' && !editor.requested_finite && !editor.requested_unlimited)
			return 'disabled';
		if (mode === 'balance' && !editor.has_balance_cap) return 'disabled';
		// increaseAllowance: "revoke" would sign an increase of 0, not a revoke.
		if (mode === 'revoke' && !editor.revoke_offered) return 'disabled';
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

/**
 * The request will go out granting an unbounded allowance, as the site asked:
 * the single approval kept on its Requested chip, or any batch leg left so.
 * Allowed since 2026-09-26 — never unsaid; each leg's own card is drawn by
 * `legBlocks`, and this is the sentence under them.
 */
function keepsUnlimited(guard: GuardView): boolean {
	if (guard.surface === 'batch') return guard.batch?.any_uncapped ?? false;
	return guard.editor?.choice?.type === 'unlimited';
}

function guardBlock(guard: GuardView, m: SigningMessages): Block | null {
	if (guard.surface !== 'approval_editor' || !guard.editor) return null;
	return allowanceBlock(guard.editor, guard.meta, m, {
		decimalsUnverified: guard.decimals_unverified,
		increaseTotal: guard.increase_total
	});
}

/**
 * A batch's own cap editors — one card per leg the core mounts an editor for
 * (an unbounded or grant-all approval), each with its leg's spender under it,
 * the phones' layout. Before this the web drew none: a Permit2 bundle's
 * unlimited leg could be seen in red but not capped.
 */
function legBlocks(guard: GuardView, m: SigningMessages): Block[] {
	if (guard.surface !== 'batch' || !guard.batch) return [];
	return guard.batch.legs.flatMap((leg, index) => {
		if (!leg.needs_editor || !leg.editor) return [];
		const card = allowanceBlock(leg.editor, leg.meta, m, { leg: index });
		if (card.kind !== 'allowance') return [];
		const blocks: Block[] = [{ ...card, label: `#${index + 1} ${card.label}` }];
		if (leg.approval) {
			blocks.push({
				kind: 'party',
				label: m.labelSpender,
				name: shortenAddress(leg.approval.spender),
				address: leg.approval.spender
			});
		}
		return blocks;
	});
}

function allowanceBlock(
	editor: GuardEditorView,
	meta: GuardView['meta'],
	m: SigningMessages,
	extra: {
		decimalsUnverified?: boolean;
		increaseTotal?: GuardView['increase_total'];
		leg?: number;
	}
): Block {
	const symbol = meta.loading ? '…' : meta.symbol;
	// Only a chosen, finite cap reads as settled; the site's unlimited ask,
	// kept, reads as the danger it is (and `keepsUnlimited` adds the sentence).
	const settled = editor.choice !== null && editor.choice.type !== 'unlimited';
	const total = extra.increaseTotal ?? null;
	return {
		kind: 'allowance',
		leg: extra.leg,
		label: fill(m.labelSpendingCap, { symbol }),
		// In tokens, never base units: a 5 USDC cap drawn as "5000000" reads
		// as five million. The send screens' exact figure, at the guard's
		// decimals (which `decimals_unverified` flags when they are a guess).
		value:
			editor.display_amount_raw === null
				? m.valueUnlimited
				: `${exactAmount(fromBaseUnits(BigInt(editor.display_amount_raw), meta.decimals))} ${symbol}`,
		valueTone: settled ? 'neutral' : 'danger',
		chips: allowanceChips(editor, m),
		note: extra.decimalsUnverified ? m.warnUnverifiedAmount : undefined,
		resultingTotal:
			total === null || total.total === null
				? undefined
				: { label: m.labelResultingTotal, value: total.total },
		// The field appears only on the chip that needs one, and it carries the
		// CORE's text: a keystroke the machine rejected must not sit on screen
		// as though it had been taken.
		custom:
			editor.mode === 'custom'
				? {
						value: editor.custom_text,
						symbol,
						placeholder: '0',
						// A typed "cap" of 10^60 is no cap — an amount the field
						// cannot take. Keeping the site's unlimited ask is the
						// Requested chip, so "unlimited is disabled" would be false.
						error:
							editor.error === 'invalid_amount' || editor.error === 'unlimited_disabled'
								? m.invalidAmount
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
/**
 * The calldata's length in bytes — what the two "unable to decode" lines name.
 * A batch counts its FIRST leg, the one its decode describes (the desktop's
 * and the phones' `first_call`); `params[0]` of a bundle has no `data`, and a
 * bundle read as "(0 bytes)".
 */
export function calldataBytes(paramsJson: string): number {
	try {
		const params = JSON.parse(paramsJson) as unknown[];
		const first = params[0] as { data?: string; calls?: { data?: string }[] } | undefined;
		const data = Array.isArray(first?.calls) ? first.calls[0]?.data : first?.data;
		if (typeof data !== 'string') return 0;
		return Math.max(0, Math.floor((data.replace(/^0x/, '').length || 0) / 2));
	} catch {
		return 0;
	}
}

/** The sentence under a refused request (spec 081). */
function selfCallBlockedText(
	blocked: NonNullable<SignView['blocked']>,
	m: SigningMessages
): string {
	if (blocked.function === 'SafeTx') return m.selfCallBlockedSafeTx;
	if (blocked.leg_index != null) {
		return fill(m.selfCallBlockedLegBody, {
			index: String(blocked.leg_index),
			function: blocked.function
		});
	}
	return fill(m.selfCallBlockedBody, { function: blocked.function });
}

function blocksFor(inputs: SigningLiveInputs): Block[] {
	const { sign, clear, guard, m } = inputs;
	const bytes = calldataBytes(sign.request?.params_json ?? '[]');
	const blocks: Block[] = [];

	/*
	 * Spec 081: the core refused this request outright — it would have changed
	 * who controls the account. Say so and stop: the decoded intent below would
	 * describe a transaction nobody can sign, and reading it as an option is
	 * exactly the confusion the refusal exists to prevent. The slider is closed
	 * by `confirm_gate_open`, which the core leaves false for a blocked request.
	 */
	if (sign.blocked) {
		blocks.push({ kind: 'intent', text: m.selfCallBlockedTitle, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: selfCallBlockedText(sign.blocked, m) });
		return blocks;
	}

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
		blocks.push(...legBlocks(guard, m));
		if (keepsUnlimited(guard)) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
		}

		// Whatever the core flagged, said once, in its own words.
		if (result.to_own_token) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnDrain });
		}
		/*
		 * Spec 081 FR-008: where the description came from, in the one case
		 * the person can act on. This used to read "no ERC-7730 descriptor,
		 * selector not listed" for every unverified result — said over a
		 * descriptor the wallet had just fetched and decoded, and over an
		 * ordinary ERC-20 transfer, both of which listed the selector fine.
		 * The other values say nothing here: built in and pinned are the
		 * verified ones, a token-standard shape is the standard doing its
		 * job, the 4-byte database has `best_effort` below, and a deployment
		 * claims nothing to warn about.
		 */
		if (result.provenance === 'fetched') {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnDescriptorFetched });
		}
		if (result.partial) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnBestEffort });
		}
		if (result.best_effort) {
			// `summaryBestEffort` carries a `{{fn}}` slot the core hands nothing
			// for, and it was drawn unfilled ("Calling {{fn}} — …"). The other
			// three shells say the placeholder-free sentence; so does this one.
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnBestEffort });
		}

		const rest = result.fields.filter((f) => f.role === 'generic' && f !== send && f !== receive);
		if (rest.length > 0) blocks.push({ kind: 'rows', rows: rest.map(fieldRow) });
		return blocks;
	}

	// No decode at all — the deepest rung. The core said so; the sheet says so.
	if (clear.surface === 'blind_transaction' || clear.surface === 'blind_typed_data') {
		blocks.push({ kind: 'intent', text: m.intentBlind, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: fill(m.warnBlindDecode, { bytes }) });
		// The approval guard reads the raw calldata, not the descriptor — an
		// approve nobody described (Permit2's own `approve`, say) still gets
		// its cap editor, and an undecodable bundle can still be known to
		// grant unlimited.
		const allowance = guardBlock(guard, m);
		if (allowance) blocks.push(allowance);
		blocks.push(...legBlocks(guard, m));
		if (keepsUnlimited(guard)) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
		}
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

/**
 * NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681): for the
 * moment between a speed being picked and its own figure landing, the fee in
 * hand is the previous speed's. The row says "estimating", and the slide stays
 * shut — the core's `confirm_fee_ready` is still true then, and would sign the
 * speed the person just walked away from.
 */
function feeOfAnotherTier({ fee, speed }: SigningLiveInputs): boolean {
	return (
		fee.fee !== null &&
		speed !== undefined &&
		offeredTier(fee.fee.tier) !== offeredTier(speed.view.tier)
	);
}

/** The fee, in the shape the drawn row renders. Off-chain requests have none. */
function feeModel(inputs: SigningLiveInputs): FeeModel {
	const { sign, fee, m } = inputs;
	const kind = sign.request?.kind;
	if (kind === 'personal_sign' || kind === 'typed_data') {
		return { kind: 'offchain', note: m.okNoNetworkFee };
	}
	const speed = speedModel(inputs);
	const ofAnotherTier = feeOfAnotherTier(inputs);
	// Exactly what `SigningHost`'s `onfee` will act on, decided once and drawn:
	// a failed quote can be asked again, and two or more coins open a list. One
	// coin and a quote is a fact with nothing behind it, and a row that says
	// otherwise is the tap that does nothing (spec 081, dead-controls #6).
	const tappable = fee.failed !== null || fee.options.length > 1;
	if (!fee.fee || ofAnotherTier) {
		// Asked and not answered yet, or asked and refused: say so in the fee's
		// own row. A sheet that drew nothing here let a person slide on a
		// mainnet transaction without ever being told what it costs — and the
		// slide stays shut in both states, as it does on the phones.
		if (fee.busy || ofAnotherTier) {
			return { kind: 'onchain', label: m.feeLabel, value: m.feeEstimating, speed, tappable };
		}
		if (fee.failed)
			return { kind: 'onchain', label: m.feeLabel, value: m.feeRetry, speed, tappable };
		return { kind: 'hidden' };
	}
	// The send screens' own line, through the send screens' own formatter: the
	// coin that is ACTUALLY paying, trimmed, and what it costs (issue 201).
	// This sheet used to print the estimate's NATIVE figure beside the CHAIN's
	// name — "0.0021 Ethereum" — and an in-band stablecoin fee came out as an
	// eighteen-decimal number under a coin nobody was spending. The design
	// sheet is explicit that these two surfaces must not drift.
	const parts = feeParts(fee.fee, fee.options);
	const value = feeLine(parts, feeOptionPriceUsd(parts.contract, fee.options), inputs.currency);
	// The coins the relay will take the fee in — the SAME rows, amounts and
	// "cannot pay" verdict the Send screen shows (founder, 2026-09-19: a fee a
	// person can switch when sending and not when signing is two products).
	const amount = (raw: string, decimals: number) =>
		trimBalance((Number(raw) / 10 ** decimals).toString(), 4);
	// The fee itself goes through the shared formatter, at the same decimal
	// budget as the row above it (issue 682): the four-decimal trim printed an
	// 0.000083 OKB fee as "~0 OKB", and a fee that reads as free is the one
	// thing this sheet may never say.
	const feeAmount = (raw: string, decimals: number, contract: string | null) =>
		feeAmountText(Number(raw) / 10 ** decimals, contract === null ? 6 : 4);
	const selector =
		inputs.feeOpen === true && fee.options.length > 1
			? {
					title: m.feeTokenTitle,
					options: fee.options.map((option) => ({
						id: option.contract ?? 'native',
						mark: { letter: option.symbol.slice(0, 1).toUpperCase(), tint: NEUTRAL_TINT },
						name: option.symbol,
						balance: `${amount(option.balance, option.decimals)} ${option.symbol}`,
						fee:
							option.amount === null
								? '—'
								: `~${feeAmount(option.amount, option.decimals, option.contract)} ${option.symbol}`,
						selected: option.selected,
						insufficient: option.insufficient
					}))
				}
			: undefined;
	// The core shut the gate because the selected coin cannot pay this fee
	// (issue 262); its row says `insufficient`. Said under the row, where the
	// other coins are one tap away — a dark slide with no reason is issue 204.
	const selected = fee.options.find((option) => option.selected);
	const warning =
		!fee.busy && fee.failed === null && !fee.confirm_fee_ready && selected?.insufficient === true
			? fill(m.feeShort, { sym: selected.symbol })
			: undefined;
	return { kind: 'onchain', label: m.feeLabel, value, selector, speed, warning, tappable };
}

/**
 * The speed control under the fee (spec 069), through the builder the send
 * form uses — each option's fee in this sheet's own fee line.
 */
function speedModel(inputs: SigningLiveInputs): FeeSpeedModel | undefined {
	const speed = inputs.speed;
	if (speed === undefined) return undefined;
	return speedControlModel(speed.view, inputs.m.speed, (quote, tier) => {
		const options = speed.feeOptions(tier);
		const parts = feeParts(quote, options);
		return feeLineParts(parts, feeOptionPriceUsd(parts.contract, options), inputs.currency);
	});
}

function techModel(inputs: SigningLiveInputs): TechModel {
	const { sign, clear, m } = inputs;
	// A refused request discloses nothing (spec 081). Android and iOS hide this
	// card entirely under a refusal; web was still putting the raw
	// `params_json` of the very request the wallet would not touch behind a
	// disclosure — the one shell that stayed lax about it.
	const request = sign.blocked ? null : sign.request;
	const result = sign.blocked ? null : clear.result;
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
/** `registry_backup::REGISTRY` — the one contract the wallet's own backup request calls. */
const PASSKEY_REGISTRY = '0x94fd1a891eb6c5f340622baf2f3a0cb70a941ea9';

/**
 * The wallet's own key backup, in the person's language.
 *
 * The core's built-in results are English, like the ERC-7730 descriptors they
 * sit beside — "the words stay in the shell". For a third-party contract that is
 * the descriptor author's text and stays as written. This one is OURS, raised by
 * the wallet itself, and a sheet that was Chinese everywhere except its three
 * most important lines read as half-finished (founder, 2026-09-19). Matched on
 * the request being first-party AND the verified registry address, never on the
 * English words.
 */
function localizedOwnBackup(clear: ClearSigningView, own: boolean, m: SigningMessages) {
	const result = clear.result;
	if (!own || !result?.verified || result.contract_address?.toLowerCase() !== PASSKEY_REGISTRY)
		return clear;
	const labels = [m.backupRegisteredAs, m.backupAddress, m.backupPublicKeys];
	return {
		...clear,
		result: {
			...result,
			intent: m.backupIntent,
			fields: result.fields.map((field, index) => ({
				...field,
				label: labels[index] ?? field.label
			}))
		},
		confirm:
			clear.confirm.type === 'confirm_intent'
				? { ...clear.confirm, intent: m.backupIntent }
				: clear.confirm
	};
}

/**
 * The cap the person chose, where the decode still says "Unlimited".
 *
 * The clear-signing result describes the REQUEST, and an unlimited approve
 * decodes as "Unlimited" in the danger tone. Once the guard holds a finite
 * choice for it (a cap, or revoke), that would describe bytes that are no
 * longer the ones being signed: the approval's warning amount field reads the
 * cap and stops being a warning, and if it was the only warning the risk falls
 * to what an approve is anyway — caution (`clear_signing::assess_risk`). The
 * same rule in every shell.
 */
export function cappedApproval(clear: ClearSigningView, guard: GuardView): ClearSigningView {
	const result = clear.result;
	const cap = capText(guard);
	if (result === null || cap === null) return clear;
	const fields = result.fields.map((field) =>
		field.warning && field.format === 'tokenAmount'
			? { ...field, value: cap, warning: false }
			: field
	);
	const risk = result.risk === 'danger' && !fields.some((f) => f.warning) ? 'caution' : result.risk;
	return { ...clear, result: { ...result, fields, risk } };
}

/**
 * The guard's finite choice on an unlimited request, as the cap row prints it
 * — the single approval's, or a batch's FIRST leg's: a bundle decodes from its
 * first leg, so that is the line the decode's "Unlimited" sits on.
 */
function capText(guard: GuardView): string | null {
	const single = guard.surface === 'approval_editor';
	const leg = guard.surface === 'batch' ? (guard.batch?.legs[0] ?? null) : null;
	const detected = single ? guard.detected : (leg?.approval ?? null);
	const editor = single ? guard.editor : (leg?.editor ?? null);
	const meta = single ? guard.meta : (leg?.meta ?? null);
	if (
		!detected?.is_unbounded ||
		editor === null ||
		meta === null ||
		editor.display_amount_raw === null ||
		(editor.choice?.type !== 'amount' && editor.choice?.type !== 'revoke')
	) {
		return null;
	}
	return `${exactAmount(fromBaseUnits(BigInt(editor.display_amount_raw), meta.decimals))} ${meta.symbol}`;
}

export function buildSigningModel(raw: SigningLiveInputs): SigningModel | null {
	if (raw.sign.surface === 'hidden' || !raw.sign.request) return null;
	const ownRequest =
		typeof window !== 'undefined' && raw.sign.request.origin === window.location.origin;
	const inputs = {
		...raw,
		clear: cappedApproval(localizedOwnBackup(raw.clear, ownRequest, raw.m), raw.guard)
	};
	const { sign, clear, guard, fee, m, identity, identicon } = inputs;
	if (sign.surface === 'hidden' || !sign.request) return null;

	const request = sign.request;
	const dapp = request.dapp;
	const name = dapp?.name ?? new URL(request.origin).host;
	const own = ownRequest;

	// Rule 1: the gate is an AND. The core may allow the request; the guard may
	// still be waiting for a cap; the fee may still be in flight.
	const feeReady =
		feeModel(inputs).kind !== 'onchain' || (fee.confirm_fee_ready && !feeOfAnotherTier(inputs));
	const enabled = sign.confirm_gate_open && guard.confirm_allowed && feeReady && !sign.is_signing;

	return {
		id: 'cs1',
		// The wallet's own request (the key backup) is not a site: it wears the
		// wallet's mark and name, and no host — `localhost:5173` under "Vela" read
		// as a stranger borrowing the brand (founder, 2026-09-19).
		dapp: own
			? { name: 'Vela Wallet', host: '', letter: 'V', tint: NEUTRAL_TINT, own: true }
			: {
					name,
					host: new URL(request.origin).host,
					letter: letterOf(name),
					tint: NEUTRAL_TINT,
					iconUrls: siteIconUrls(request.origin)
				},
		network: {
			name: chainName(request.chain_id),
			dot: NEUTRAL_TINT,
			logoUrl: chainLogoURL(request.chain_id)
		},
		blocks: blocksFor(inputs),
		tech: techModel(inputs),
		techOpen: false,
		fee: sign.blocked ? { kind: 'hidden' } : feeModel(inputs),
		signer: {
			label: m.signingAccount,
			name: identity.name,
			identiconSvg: identicon(identity.address),
			address: identity.address
		},
		// Spec 081: refused — no fee, no slider, one way out.
		dismissOnly: sign.blocked ? m.close : undefined,
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
