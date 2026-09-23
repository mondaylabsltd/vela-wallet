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
import { trimBalance } from '$lib/wallet/live';
import { chainName } from '$lib/services/networks';
import { shortenAddress } from '$lib/wallet/identity';
import type { WalletIdentity } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import { encodeQr } from '$lib/wallet/qr';
import type { SignMethod } from '$lib/onboarding/core/passkey';
import type { ClearSignerNotice } from './clear-signer';
import type { ClearSignerWords, SigningMessages } from './messages';
import type {
	AllowanceChip,
	AmountLine,
	Block,
	ClearSignerModel,
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
	if (!fee.fee || ofAnotherTier) {
		// Asked and not answered yet, or asked and refused: say so in the fee's
		// own row. A sheet that drew nothing here let a person slide on a
		// mainnet transaction without ever being told what it costs — and the
		// slide stays shut in both states, as it does on the phones.
		if (fee.busy || ofAnotherTier) {
			return { kind: 'onchain', label: m.feeLabel, value: m.feeEstimating, speed };
		}
		if (fee.failed) return { kind: 'onchain', label: m.feeLabel, value: m.feeRetry, speed };
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
	return { kind: 'onchain', label: m.feeLabel, value, selector, speed, warning };
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

export function buildSigningModel(raw: SigningLiveInputs): SigningModel | null {
	if (raw.sign.surface === 'hidden' || !raw.sign.request) return null;
	const ownRequest =
		typeof window !== 'undefined' && raw.sign.request.origin === window.location.origin;
	const inputs = { ...raw, clear: localizedOwnBackup(raw.clear, ownRequest, raw.m) };
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

/**
 * "Sign with" on the sheet (spec 071): every method the `sign_pref` core
 * offers, in its order, named as the create flow and Settings name them; the
 * Clear Signer with its one line. The request starts at Settings' default
 * and shows this request's own pick once there is one — the pick never goes
 * back to the preference (contract §6). A name this build has no words for
 * is not drawn, and is never in force: the default falls back to `auto`.
 */
export function signWithModel(input: {
	offered: readonly string[];
	defaultMethod: string;
	picked: string | null;
	open: boolean;
	m: SigningMessages;
}): { method: SignMethod; row: NonNullable<SigningModel['signWith']> } {
	const { m } = input;
	const titles: Record<SignMethod, string> = {
		auto: m.signWithAuto,
		platform: m.signWithPlatform,
		hybrid: m.signWithHybrid,
		security_key: m.signWithSecurityKey,
		clear_signer: m.signWithClearSigner
	};
	const offered = input.offered.filter((id): id is SignMethod => id in titles);
	const inForce = (id: string | null): id is SignMethod =>
		id !== null && offered.includes(id as SignMethod);
	const method: SignMethod = inForce(input.picked)
		? input.picked
		: inForce(input.defaultMethod)
			? input.defaultMethod
			: 'auto';
	return {
		method,
		row: {
			label: m.signWithLabel,
			value: titles[method],
			open: input.open,
			options: offered.map((id) => ({
				id,
				title: titles[id],
				detail: id === 'clear_signer' ? m.signWithClearSignerBody : undefined,
				selected: id === method
			}))
		}
	};
}

/**
 * The Clear Signer's sheet (spec 071, extended by 075) — one sheet, four
 * moments, in the order they happen:
 *
 * 1. **where is it?** this device, or another one (075: it is a passkey route,
 *    and a route can be somewhere else);
 * 2. **pairing** with that other device: the link as a code to scan, and the
 *    six digits to compare once it arrives — nothing is sent before the person
 *    says they match;
 * 3. **waiting** on the page: the hint, open it again, cancel;
 * 4. the one **sentence** its ending gets (contract §5), until it is closed.
 *
 * `null` when there is nothing to say: nothing open, or the person cancelled
 * and already knows.
 */
export function clearSignerModel(
	state: {
		asking?: boolean;
		pairing?: { link: string; code: string | null } | null;
		waiting: boolean;
		notice: ClearSignerNotice | null;
	},
	m: ClearSignerWords
): ClearSignerModel | null {
	if (state.asking === true) {
		return {
			waiting: false,
			title: m.clearSignerWhere,
			where: { thisDevice: m.clearSignerThisDevice, otherDevice: m.clearSignerOtherDevice },
			dismiss: m.clearSignerCancel
		};
	}
	const pairing = state.pairing ?? null;
	if (pairing !== null) {
		return {
			// Waiting on the OTHER DEVICE's person, not on a page here: the sheet
			// says what to do with the code, and only then that it is waiting.
			waiting: false,
			title: pairing.code === null ? m.clearSignerPair : m.clearSignerWaiting,
			pair: {
				hint: m.clearSignerPairHint,
				link: pairing.link,
				qr: encodeQr(pairing.link),
				copy: m.clearSignerCopyLink,
				waiting: m.clearSignerPairWaiting
			},
			code:
				pairing.code === null
					? undefined
					: {
							text: fill(m.clearSignerCode, { code: pairing.code }),
							confirm: m.clearSignerCodeConfirm
						},
			dismiss: m.clearSignerCancel
		};
	}
	if (state.waiting) {
		return {
			waiting: true,
			title: m.clearSignerWaiting,
			hint: m.clearSignerWaitingHint,
			reopen: m.clearSignerReopen,
			dismiss: m.clearSignerCancel
		};
	}
	if (state.notice === null) return null;
	const endings: Record<ClearSignerNotice, string> = {
		closed: m.clearSignerClosed,
		refused: m.clearSignerRefused,
		mismatch: m.clearSignerMismatch,
		timeout: m.clearSignerTimeout,
		tunnel: m.clearSignerTunnelDown
	};
	return { waiting: false, title: endings[state.notice], dismiss: m.close };
}
