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
 *    chosen; `ClearSigningView.resolving` says the request is still being
 *    read (spec 096 F7). The confirm opens only when all agree (and the fee,
 *    when the request has one, is ready). Its own doc in the drawn component
 *    says the shell must AND them — this is that place.
 * 2. **The ✕ is the refusal.** The 022 interaction contract draws no reject
 *    button: closing the sheet IS the refusal, and the route answers the
 *    requester with 4001 — but since spec 079 only the header's ✕ closes it
 *    (no scrim, drag or Escape: a stray touch lost the owner a request).
 */
import type { ClearBatchCall } from '$lib/core/generated/ClearBatchCall';
import type { ClearBatchView } from '$lib/core/generated/ClearBatchView';
import type { ClearSignField } from '$lib/core/generated/ClearSignField';
import type { ClearSignResult } from '$lib/core/generated/ClearSignResult';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { GuardEditorView } from '$lib/core/generated/GuardEditorView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignApproveOpts } from '$lib/core/generated/SignApproveOpts';
import type { SignView } from '$lib/core/generated/SignView';
import type { SimVerdict } from '$lib/core/generated/SimVerdict';
import { feeAmountText, feeLineParts, feeOptionPriceUsd, feeParts } from '$lib/flows/fee-line';
import { offeredTier, speedControlModel } from '$lib/flows/speed-control';
import type { FeeSpeedModel } from '$lib/flows/model';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { chainLogoURL, tokenMarkFor } from '$lib/flows/marks';
import { browserSiteLabel, signConfirmState } from '$lib/core/kernels';
import { estimateRevertsFor } from '$lib/services/estimate-verdict';
import { exactAmount, moneyText, trimBalance } from '$lib/wallet/live';
import { fromBaseUnits } from '$lib/services/eip681';
import { chainName, nativeSymbol } from '$lib/services/networks';
import { shortenAddress } from '$lib/wallet/identity';
import type { WalletIdentity } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import { venueBlockText } from '$lib/settings/venue';
import { failedFeeTappable } from '$lib/flows/fee-failure';
import { SIM_COULD_NOT_CHECK_KEY, type SigningMessages } from './messages';
import type {
	AllowanceChip,
	AmountLine,
	Block,
	FeeModel,
	FeeTokenOption,
	KeyValueRow,
	SigningModel,
	SigningStatus,
	TechModel,
	Tone
} from './model';
import type { ApprovalProgress } from './approval-progress';

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
	/**
	 * Spec 079: the approved request's progress — has its signature been made,
	 * is the passkey prompt up (`approval-progress.ts`). Absent: nothing known,
	 * so the ✕ stays shut while anything is in flight.
	 */
	progress?: Pick<ApprovalProgress, 'signed' | 'ceremonyUp'>;
	/**
	 * The sheet's own simulation of THIS request, as the core read it
	 * (`simOutcome`, PR 3 device round) — the host's one `eth_simulateV1`
	 * read, which also tells the fee machine what the calls move. Absent
	 * until the node answers, for a request nobody simulated (a message), and
	 * for any other request than the one it was measured for.
	 */
	sim?: SimVerdict | null;
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
		// An address reads as monospace; a contract the core names
		// ("PancakeSwap Permit2", 096 F5) is a name, in the text face.
		mono: (field.address !== null && field.value.startsWith('0x')) || field.token_address !== null
	};
}

/**
 * The sign an outgoing amount wears: U+2212, the minus the core's signed
 * amounts use (spec 082 RJ15) — never ASCII `-`, which reads as a hyphen.
 */
export const MINUS = '\u2212';

/**
 * The amount a decoded field carries, when it is the one the eye should land
 * on. Its fiat goes through the wallet's own money formatter (spec 082 G60):
 * the display currency, the person's number preset, and never exponent
 * notation — `toFixed` wrote a 10^30-unit transfer as "≈ $1e+24".
 */
function amountLine(
	field: ClearSignField,
	outgoing: boolean,
	currency: SigningLiveInputs['currency']
): AmountLine {
	return {
		// Spec 097 N1: an amount nobody could scale (the core's em dash) has
		// no figure to sign — "+— 0x8ac7…" read as a typo, not as unknown.
		sign: field.unverified ? '' : outgoing ? MINUS : '+',
		value: field.value,
		symbol: '',
		fiat: field.usd_value === null ? undefined : `≈ ${moneyText(field.usd_value, currency)}`,
		// Spec 097 N3: the hero drops the field's label, so an amount the core
		// marks as a bound keeps it as its caption — "You receive (min)", the
		// words a batch leg's row already says. The core decides; this draws.
		caption: field.bound ? field.label : undefined,
		// Spec 097 N1: an amount nobody could scale is no figure to celebrate.
		tone: field.warning ? 'danger' : field.unverified ? 'caution' : outgoing ? 'neutral' : 'success'
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
 * The guard's sentences under an approval (spec 094 S8): the danger line when
 * the request grants an unbounded allowance as it stands — the core's
 * `unlimited_warning`, which covers the single approval kept on its Requested
 * chip, any batch leg left so, and an off-chain permit for an unbounded
 * amount (allowed since 2026-09-26, never unsaid) — and, for every off-chain
 * permit, that its amount cannot be capped here: the dApp redeems its own
 * struct, so the phones' line is the honest one (before this the web drew an
 * unlimited Permit2 in red and said nothing at all, 089 F22).
 */
function guardWarnings(guard: GuardView, m: SigningMessages): Block[] {
	const blocks: Block[] = [];
	if (guard.unlimited_warning)
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnUnlimited });
	if (guard.surface === 'permit_sign') {
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnPermitCantCap });
	}
	return blocks;
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
	// kept, reads as the danger it is (and `guardWarnings` adds the sentence).
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
 * The calldata's length in bytes — what the "unable to decode" line names for
 * a lone call. A batch of ONE call is drawn as that call, so it counts that
 * call; a longer batch names each call's own bytes (`ClearBatchCall`).
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

/**
 * 089 S1: one call of a batch, as its own card — the same words its call
 * would get alone. A decoded call is its intent and its fields, then the coin
 * it moves and whom it calls; a plain send is "Send", the exact amount and
 * the recipient; a call nobody could read says so in its title, with whom it
 * calls and what coin it moves. Never omitted: the core hands a row for every
 * call.
 */
function batchCallCard(call: ClearBatchCall, symbol: string, m: SigningMessages): Block {
	const step = (action: string) => fill(m.batchStep, { index: String(call.index), action });
	const tone = toneOf(call.risk);
	// What the call moves of the chain's own coin, and whom it calls: inside a
	// batch nothing else on the sheet says it for this call.
	const coin: KeyValueRow[] =
		call.amount !== null && call.value_wei !== '0'
			? [{ label: m.labelAmount, value: `${MINUS}${call.amount} ${symbol}` }]
			: [];
	// A contract the wallet knows on this chain is named (096 F5); any other
	// is its full address.
	const target: KeyValueRow[] =
		call.to === null
			? []
			: call.to_name
				? [{ label: m.labelInteracting, value: call.to_name }]
				: [{ label: m.labelInteracting, value: call.to, mono: true }];
	if (call.surface === 'clear_sign' && call.result) {
		return {
			kind: 'card',
			title: step(call.result.intent),
			tone,
			rows: [
				...call.result.fields.filter((field) => !field.detail).map(fieldRow),
				...coin,
				...target
			]
		};
	}
	if (call.surface === 'plain_send' && call.plain_send) {
		const plain = call.plain_send;
		return {
			kind: 'card',
			title: step(m.intentSend),
			tone,
			rows: [
				{ label: m.labelAmount, value: `${plain.no_value ? '' : MINUS}${plain.amount} ${symbol}` },
				{ label: m.labelRecipient, value: plain.to, mono: true }
			]
		};
	}
	return {
		kind: 'card',
		title: step(fill(m.warnBlindDecode, { bytes: String(call.data_bytes) })),
		tone,
		rows: [...target, ...coin]
	};
}

/**
 * 089 S1: a batch as the sheet draws it — "Batch", how many transactions are
 * signed together, then EVERY call as its own card (the drawn CS26), the coin
 * the whole batch moves, the guard's per-call cap cards, and every flag any
 * call raised, said once. The headline is never call 1's: a batch of
 * `[1 wei → A, 1 xDAI → B]` read "Send 0.000…1 xDAI" and signed both.
 */
function batchBlocks(inputs: SigningLiveInputs, batch: ClearBatchView): Block[] {
	const { sign, guard, m } = inputs;
	const symbol = sign.request ? nativeSymbol(sign.request.chain_id) : '';
	const blocks: Block[] = [
		{ kind: 'intent', text: m.intentBatch, tone: toneOf(batch.risk) },
		{
			kind: 'sentence',
			text: fill(m.summaryBatch, { count: String(batch.calls.length) }),
			tone: 'accent'
		},
		...batch.calls.map((call) => batchCallCard(call, symbol, m))
	];
	if (batch.total_amount !== null && batch.total_value_wei !== '0') {
		blocks.push({
			kind: 'rows',
			rows: [{ label: m.labelTotal, value: `${MINUS}${batch.total_amount} ${symbol}` }]
		});
	}
	// The guard reads every call's raw calldata: a cap card per unbounded call.
	blocks.push(...legBlocks(guard, m));
	blocks.push(...guardWarnings(guard, m));
	const results = batch.calls.flatMap((call): ClearSignResult[] =>
		call.result ? [call.result] : []
	);
	if (results.some((r) => r.to_own_token)) {
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnDrain });
	}
	if (results.some((r) => r.provenance === 'fetched')) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnDescriptorFetched });
	}
	if (results.some((r) => r.terms_off_chain)) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnOrderTerms });
	}
	// Spec 097 N1: an incomplete reading says it is incomplete — the phones'
	// and desktop's line — not that it came from a function signature.
	if (results.some((r) => r.partial)) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnPartial });
	}
	if (results.some((r) => r.best_effort)) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnBestEffort });
	}
	if (results.some((r) => r.fields.some((f) => f.unverified))) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnUnverifiedAmount });
	}
	return blocks;
}

function blocksFor(inputs: SigningLiveInputs): Block[] {
	const { sign, clear, guard, m } = inputs;
	const bytes = calldataBytes(sign.request?.params_json ?? '[]');
	const blocks: Block[] = [];

	/*
	 * Spec 081: the core refused this request outright — it would have changed
	 * who controls the account. Say so and stop: the decoded intent below would
	 * describe a transaction nobody can sign, and reading it as an option is
	 * exactly the confusion the refusal exists to prevent. The confirm is shut
	 * by `confirm_gate_open`, which the core leaves false for a blocked request.
	 */
	if (sign.blocked) {
		blocks.push({ kind: 'intent', text: m.selfCallBlockedTitle, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: selfCallBlockedText(sign.blocked, m) });
		return blocks;
	}

	// Spec 096 F7: still reading — a neutral "Loading…", and the confirm stays
	// shut (`buildSigningModel`). This said the cap editor's "Set a finite
	// amount to continue." over an Aave supply for seconds.
	if (clear.surface === 'loading' || clear.resolving) {
		blocks.push({ kind: 'sentence', text: m.loading, tone: 'neutral' });
		return blocks;
	}

	if (clear.surface === 'batch' && clear.batch) {
		return batchBlocks(inputs, clear.batch);
	}

	const result = clear.result;
	if (result) {
		blocks.push({ kind: 'intent', text: result.intent, tone: toneOf(result.risk) });

		const send = result.fields.find((f) => f.role === 'send_amount');
		const receive = result.fields.find((f) => f.role === 'receive_amount');
		const currency = inputs.currency;
		if (send && receive) {
			blocks.push({
				kind: 'swap',
				pay: amountLine(send, true, currency),
				receive: amountLine(receive, false, currency)
			});
		} else if (send) {
			blocks.push({ kind: 'amount', line: amountLine(send, true, currency) });
		} else if (receive) {
			blocks.push({ kind: 'amount', line: amountLine(receive, false, currency) });
		}
		// Spec 096 F4: the coin the call sends, when its reading does not say
		// it — the hero when nothing else is, else the batch call's own row.
		const coin = nativeValueBlock(inputs, !send && !receive);
		if (coin) blocks.push(coin);

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
		blocks.push(...guardWarnings(guard, m));

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
		if (result.terms_off_chain) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnOrderTerms });
		}
		// Spec 097 N1: an incomplete reading — an amount nobody could scale is
		// one — says so in the other shells' words, and the amount says why.
		if (result.partial) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnPartial });
		}
		if (result.fields.some((f) => f.unverified)) {
			blocks.push({ kind: 'warning', tone: 'caution', text: m.warnUnverifiedAmount });
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

	/*
	 * Spec 082 G14 (RC1–RC6): a call with no calldata is a SEND, whatever the
	 * recipient — the core decided it and wrote the exact amount. What the
	 * phones always drew: "Send", the amount card in the coin the fee row
	 * uses (RC5), and who it goes to. Never the red "Blind signature" over
	 * bytes that do not exist. A zero value keeps the card, with no minus
	 * sign (RC3): only a simulation may say that nothing leaves.
	 */
	if (clear.surface === 'plain_send' && clear.plain_send && sign.request) {
		const plain = clear.plain_send;
		blocks.push({ kind: 'intent', text: m.intentSend, tone: 'neutral' });
		blocks.push({
			kind: 'amount',
			line: {
				sign: plain.no_value ? '' : MINUS,
				value: plain.amount,
				symbol: nativeSymbol(sign.request.chain_id),
				tone: 'neutral'
			}
		});
		blocks.push({
			kind: 'party',
			label: m.labelRecipient,
			name: shortenAddress(plain.to),
			address: plain.to
		});
		return blocks;
	}

	// No decode at all — the deepest rung. The core said so; the sheet says so.
	if (clear.surface === 'blind_transaction' || clear.surface === 'blind_typed_data') {
		blocks.push({ kind: 'intent', text: m.intentBlind, tone: 'danger' });
		// Nobody could read the call — what coin it sends is still known.
		const coin = nativeValueBlock(inputs, true);
		if (coin) blocks.push(coin);
		blocks.push({ kind: 'warning', tone: 'danger', text: fill(m.warnBlindDecode, { bytes }) });
		// The approval guard reads the raw calldata, not the descriptor — an
		// approve nobody described (Permit2's own `approve`, say) still gets
		// its cap editor, and an undecodable bundle can still be known to
		// grant unlimited.
		const allowance = guardBlock(guard, m);
		if (allowance) blocks.push(allowance);
		blocks.push(...legBlocks(guard, m));
		blocks.push(...guardWarnings(guard, m));
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
 * Spec 096 F4: the chain's own coin a lone call sends (`native_value`), in
 * the words a batch call's row uses — "Amount −0.003 BNB" — or as the hero
 * amount when the reading has none. The coin symbol is the fee row's (RC5).
 */
function nativeValueBlock(inputs: SigningLiveInputs, hero: boolean): Block | null {
	const { sign, clear, m } = inputs;
	const native = clear.native_value;
	if (!native || !sign.request) return null;
	const symbol = nativeSymbol(sign.request.chain_id);
	if (hero) {
		return {
			kind: 'amount',
			line: { sign: MINUS, value: native.amount, symbol, tone: 'neutral' }
		};
	}
	return {
		kind: 'rows',
		rows: [{ label: m.labelAmount, value: `${MINUS}${native.amount} ${symbol}` }]
	};
}

/**
 * NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681): for the
 * moment between a speed being picked and its own figure landing, the fee in
 * hand is the previous speed's. The row says "estimating", and the confirm stays
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

/**
 * The row's figure on a failed fee, by the key the core names
 * (`FeeFailureView.figure_key`): "Tap to retry" when a tap is the one way,
 * "Pay with another coin" when the tap opens the fee coins (PR 2 polish). A
 * key this build does not carry draws the dash, never a dotted path.
 */
function feeFigures(m: SigningMessages): Readonly<Record<string, string>> {
	return {
		'componentsUi.gas.estimateFailed': m.feeRetry,
		'componentsUi.gas.payWithAnotherCoin': m.feePayWithAnotherCoin
	};
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
	// a failed quote can be asked again (below), and two or more coins open a
	// list. One coin and a quote is a fact with nothing behind it, and a row
	// that says otherwise is the tap that does nothing (spec 081, dead-controls
	// #6).
	const choosable = fee.options.length > 1;
	const tappable = choosable;
	// Spec 079 (the owner: "似乎没有刷新网络费的按钮呀"): the send form's own
	// refresh control, turning while a measurement is out — the same generic
	// busy flag the "estimating" value reads, so the row can never claim to be
	// both settled and measuring. The chevron only where a tap opens a list.
	// A figure switched to another coin and not yet measured with that coin's
	// fee leg (`provisional`, correctness batch item 4 — a chip tap or the
	// sheet's own re-pick) is drawn as it is, with the measuring sign, until
	// the new figure lands; the core's gate holds the confirm meanwhile.
	const measuring = fee.busy || fee.provisional;
	const refresh = { refreshLabel: m.feeRefresh, refreshing: measuring, chevron: choosable };
	// The display currency is not the person's yet: the fee's money is
	// withheld, and the row keeps the line it will land on from now — under
	// "Estimating…" too, so the fee landing moves nothing either (F12).
	const withheld = inputs.currency.committed ? {} : { valueFiatWithheld: true };
	// PR 2 note 1: the fee's failure, said ONCE by the core for this row and
	// the line under the confirm (`FeeView.failure`) — present while the run
	// failed AND through the re-ask that follows it, so nothing here holds a
	// failure of its own and nothing flips to "estimating" and back every few
	// seconds. The row's figure is the core's: "Tap to retry" only when a tap
	// is the one way (`figure_key`), else the dash — never a tap while the
	// core is asking again by itself. The reason line is the core's key too
	// (spec 082 RJ13: the relay out of reach, a rate-limited chain node, a
	// chain out of reach — `{{chain}}`, a fault inside Vela), drawn in the
	// line the row keeps, and kept through the re-ask beside the measuring
	// sign. A failure the network did not cause has no line.
	const failure = fee.failure;
	if (failure) {
		const words = failure.reason_key === null ? undefined : m.feeReasons[failure.reason_key];
		const chain = sign.request ? chainName(sign.request.chain_id) : '';
		// PR 2 polish: the row does exactly what its figure says
		// (`FeeFailureView.tap`) — "Tap to retry" asks again at once
		// (`requote`), and the row stays the same control through the re-ask,
		// so nothing under a thumb changes; "Pay with another coin" opens the
		// coins, behind the chevron that promises them, and the list is drawn
		// here while open; a dash with no coin left to try is no control.
		const tap = failure.tap;
		return {
			kind: 'onchain',
			label: m.feeLabel,
			value: failure.figure_key === null ? '—' : (feeFigures(m)[failure.figure_key] ?? '—'),
			speed,
			selector: tap === 'choose_coin' ? feeSelector(inputs) : undefined,
			tappable: failedFeeTappable(failure),
			warning: words === undefined ? undefined : fill(words, { chain }),
			...refresh,
			// Turning while the core's re-ask (or a tap's) is out.
			refreshing: measuring || failure.retrying,
			chevron: tap === 'choose_coin'
		};
	}
	if (!fee.fee || ofAnotherTier) {
		// Asked and not answered yet: say so in the fee's own row. A sheet that
		// drew nothing here let a person confirm a mainnet transaction without
		// ever being told what it costs — and the confirm stays shut, as it
		// does on the phones.
		if (fee.busy || ofAnotherTier) {
			return {
				kind: 'onchain',
				label: m.feeLabel,
				value: m.feeEstimating,
				...withheld,
				speed,
				tappable,
				// A first figure the core already knows no coin can pay
				// (`nothing_to_pay_from`): its line's room is held from now, so
				// its landing does not move the confirm (iPhone pass 2026-10-09).
				warningReserved: fee.busy && fee.nothing_to_pay_from ? m.feeNoCoinPays : undefined,
				...refresh
			};
		}
		return { kind: 'hidden' };
	}
	// The send screens' own line, through the send screens' own formatter: the
	// coin that is ACTUALLY paying, trimmed, and what it costs (issue 201).
	// This sheet used to print the estimate's NATIVE figure beside the CHAIN's
	// name — "0.0021 Ethereum" — and an in-band stablecoin fee came out as an
	// eighteen-decimal number under a coin nobody was spending. The design
	// sheet is explicit that these two surfaces must not drift.
	const parts = feeParts(fee.fee, fee.options);
	// In the send form's two pieces — the coin, and what it costs — so the row
	// can stand the money under the coin WHOLE (F12). As one string, "0.0015
	// ETH · ≈…" became "0.0015 ETH · ≈₫112,500.00" at 320 px, "Network fee"
	// wrapped to make room, and the bottom-anchored sheet moved 18 px.
	const line = feeLineParts(parts, feeOptionPriceUsd(parts.contract, fee.options), inputs.currency);
	const selector = feeSelector(inputs);
	// The core shut the gate because the selected coin cannot pay this fee
	// (issue 262); its row says `insufficient`. Said under the row, where the
	// other coins are one tap away — a shut confirm with no reason is issue 204.
	// Otherwise, spec 096 F2: the coin in force is one the transaction itself
	// may spend, so what is left for the fee may be too little — the core's
	// `spent_by_operation`, said while that coin is the one paying.
	// Issue 408: and when not one coin on offer can pay, the core says so
	// (`no_coin_pays`), and so does this line — naming the coin in force read
	// as if another could stand in ("Insufficient ETH" over a wallet whose
	// USDT was short too).
	const selected = fee.options.find((option) => option.selected);
	const warning = fee.no_coin_pays
		? m.feeNoCoinPays
		: !measuring && fee.failed === null && !fee.confirm_fee_ready && selected?.insufficient === true
			? fill(m.feeShort, { sym: selected.symbol })
			: !measuring && selected?.spent_by_operation === true
				? fill(m.feeCoinSpent, { sym: selected.symbol })
				: undefined;
	return {
		kind: 'onchain',
		label: m.feeLabel,
		value: line.coin,
		...(line.fiat === null ? {} : { valueFiat: line.fiat }),
		...withheld,
		selector,
		speed,
		warning,
		tappable,
		...refresh,
		// The send form's rule (spec 068): a quote past its TTL is OLD, not
		// wrong — said calmly, and not while a fresh measurement is out.
		staleNote: fee.stale && !measuring ? m.feeStale : undefined
	};
}

/**
 * The fee coins, drawn in the sheet while their list is open (`feeOpen`) and
 * there is more than one to choose — the normal row's list, and (PR 2
 * polish) the one "Pay with another coin" opens after the relay answered
 * that the operation would fail with the coin in force.
 */
function feeSelector(
	inputs: SigningLiveInputs
): { title: string; options: FeeTokenOption[] } | undefined {
	const { sign, fee, m } = inputs;
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
	const chainId = sign.request?.chain_id ?? 1;
	return inputs.feeOpen === true && fee.options.length > 1
		? {
				title: m.feeTokenTitle,
				options: fee.options.map((option) => ({
					id: option.contract ?? 'native',
					// The REQUEST's chain: the coin is paid on the chain this
					// transaction runs on, whatever the estimate says or lacks.
					mark: tokenMarkFor(chainId, option.symbol, option.contract),
					name: option.symbol,
					balance: `${amount(option.balance, option.decimals)} ${option.symbol}`,
					fee:
						option.amount === null
							? '—'
							: `~${feeAmount(option.amount, option.decimals, option.contract)} ${option.symbol}`,
					selected: option.selected,
					insufficient: option.insufficient,
					// Issue 408: a refused coin says why — the core's numbers.
					reason:
						option.insufficient && option.short !== null && option.short !== undefined
							? fill(m.feeRowShort, { need: option.short.need, have: option.short.have })
							: undefined
				}))
			}
		: undefined;
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

/**
 * PR 3 device round, item 3 — "No asset changes" is the core's line, and the
 * web says it too.
 *
 * The sheet's own simulation was a check, and nothing of the person's moves:
 * the core names the line (`SimVerdict.no_change_key`) and this looks its
 * words up. Which case this is, and the sentence, are never decided here —
 * no key, no line. Only for a transaction that may still be signed: a
 * message moves nothing by nature (and nobody simulates one), and a refused
 * request says only its refusal.
 *
 * Two things of the simulation's answer are still NOT drawn here (spec 082
 * RG6 stands): the balance rows of a check that moves something, and a
 * revert's line — the relay's own estimate stays the one voice that says
 * "will fail" (`withEstimateVerdict`).
 *
 * Nothing while the core says nothing could be checked (`couldNotCheckLine`):
 * that sentence has the verdict's place then, and never both at once.
 */
function noChangeLine(inputs: SigningLiveInputs): string | undefined {
	const { sign, sim, m } = inputs;
	const request = sign.request;
	if (!request || sign.blocked) return undefined;
	if (request.kind !== 'transaction' && request.kind !== 'batch') return undefined;
	if (couldNotCheckLine(inputs) !== undefined) return undefined;
	const key = sim?.no_change_key ?? null;
	return key === null ? undefined : m.simSaid[key] || undefined;
}

/**
 * PR 3 — nothing could be checked, and the verdict's place says so, as a
 * caution: "Vela couldn't check what this transaction does…". A person who
 * confirms then does it knowing — never under an empty place that reads as
 * "nothing to report".
 *
 * The core says it two ways, and names the sentence both times:
 *
 * - the simulation's deadline passed with no verdict at all
 *   (`SignView.sim_waited_out_key` — the confirm waits four seconds at most,
 *   then opens beside this line);
 * - this request's own verdict is a not-checked answer
 *   (`SimVerdict.notice_risk === 'caution'`, its `notice_key`): the node does
 *   not offer the simulation, answered what nobody can read, or no node
 *   answered.
 *
 * The first was once the only one drawn, and the line then left the sheet the
 * moment the pool gave up: "could not check" taken back, over an open
 * confirm, when "could not check" was the final verdict. Both are the same
 * card, so the one becoming the other moves nothing and lands nothing anew.
 *
 * No key, no line; a key this build has no words for is never drawn as a
 * dotted path; a revert's notice (`danger`) is not this one. A refused
 * request says only its refusal.
 */
function couldNotCheckLine({ sign, sim, m }: SigningLiveInputs): string | undefined {
	const request = sign.request;
	if (!request || sign.blocked) return undefined;
	const waitedOut = cautionWords(sign.sim_waited_out_key, m);
	if (waitedOut !== undefined) return waitedOut;
	if (request.kind !== 'transaction' && request.kind !== 'batch') return undefined;
	return sim?.notice_risk === 'caution' ? cautionWords(sim.notice_key, m) : undefined;
}

/** The words of the caution the core named for the verdict's place, by its key — or none. */
function cautionWords(key: string | null | undefined, m: SigningMessages): string | undefined {
	return key === SIM_COULD_NOT_CHECK_KEY ? m.warnSimUnavailable || undefined : undefined;
}

/**
 * The verdict's place: the balance-changes card with no rows, where the apps
 * put the verdict — after the request's own blocks, before the footer. What
 * it says lands after the sheet has opened (`verdict`), so the sheet keeps
 * its confirm where it was and brings the card into sight.
 *
 * "No asset changes" is a site's request's alone here — the wallet's own
 * folds it into Technical details (`techModel`, issue 314). The caution that
 * nothing could be checked is drawn for both alike: it is the one thing the
 * person must see before a confirm that just opened, so it is never folded
 * away.
 */
function withSimVerdict(blocks: Block[], inputs: SigningLiveInputs, own: boolean): Block[] {
	const title = inputs.m.balancesTitle;
	const caution = couldNotCheckLine(inputs);
	if (caution !== undefined) {
		return [
			...blocks,
			{ kind: 'balances', title, rows: [], note: caution, noteTone: 'caution', verdict: true }
		];
	}
	const note = own ? undefined : noChangeLine(inputs);
	if (note === undefined) return blocks;
	return [...blocks, { kind: 'balances', title, rows: [], note, verdict: true }];
}

function techModel(inputs: SigningLiveInputs, own: boolean): TechModel {
	const { sign, clear, m } = inputs;
	// A refused request discloses nothing (spec 081). Android and iOS hide this
	// card entirely under a refusal; web was still putting the raw
	// `params_json` of the very request the wallet would not touch behind a
	// disclosure — the one shell that stayed lax about it.
	const request = sign.blocked ? null : sign.request;
	const result = sign.blocked ? null : clear.result;
	const simResult = noChangeLine(inputs);
	return {
		title: m.advancedToggle,
		// The contract's name beside the toggle tells a site's request apart;
		// on the wallet's own backup "· Vela passkey registry" was a stranger's
		// term over a sheet that is otherwise plain words. It stays inside.
		summary: own ? undefined : (result?.contract_name ?? undefined),
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
		// Issue 314: the wallet's own request is plain rows, and a card saying
		// nothing moves would be the only "simulation" on it — so the line
		// folds in here, as it does on the phones.
		...(own && simResult !== undefined
			? { simResult: { label: m.techSimResult, value: simResult } }
			: {}),
		raw: request ? { label: m.techRawData, hex: request.params_json } : undefined,
		copyLabel: m.copyValue,
		explorerLabel: m.viewOnExplorer
	};
}

/**
 * The core's words in the reader's language. A clear-signing result is
 * English — a descriptor's intent and labels, the "Unlimited" a threshold
 * prints — and the core names the ones it recognises (`intent_term`,
 * `label_term`, `value_term`, the confirm's `intent_term`). Each named word is
 * swapped for this locale's; anything unnamed stays as the descriptor wrote
 * it. Runs before `cappedApproval`, which replaces the warning value with the
 * cap anyway.
 */
export function localizedTerms(clear: ClearSigningView, m: SigningMessages): ClearSigningView {
	const word = (term: string | null | undefined, text: string) =>
		(term ? m.terms[term] : undefined) || text;
	const localize = (result: ClearSignResult): ClearSignResult => ({
		...result,
		intent: word(result.intent_term, result.intent),
		fields: result.fields.map((field) => ({
			...field,
			label: word(field.label_term, field.label),
			value: word(field.value_term, field.value)
		}))
	});
	const result = clear.result;
	return {
		...clear,
		result: result && localize(result),
		// Every call of a batch, in the same words a lone call gets.
		batch: clear.batch && {
			...clear.batch,
			calls: clear.batch.calls.map((call) => ({
				...call,
				result: call.result && localize(call.result)
			}))
		},
		confirm:
			clear.confirm.type === 'confirm_intent'
				? { ...clear.confirm, intent: word(clear.confirm.intent_term, clear.confirm.intent) }
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
	const cap = (result: ClearSignResult, text: string | null): ClearSignResult => {
		if (text === null) return result;
		const fields = result.fields.map((field) =>
			field.warning && field.format === 'tokenAmount'
				? { ...field, value: text, warning: false }
				: field
		);
		const risk =
			result.risk === 'danger' && !fields.some((f) => f.warning) ? 'caution' : result.risk;
		return { ...result, fields, risk };
	};
	// A batch: each call's "Unlimited" is replaced by ITS OWN leg's cap — the
	// guard's legs are the calls, in order.
	if (clear.batch && guard.surface === 'batch') {
		const calls = clear.batch.calls.map((call, index) => {
			if (!call.result) return call;
			const result = cap(call.result, capText(guard, index));
			// Capped, the call is what its decode now says — unless it burns.
			const risk =
				call.risk === 'danger' && result.risk !== 'danger' && !result.to_own_token
					? result.risk
					: call.risk;
			return { ...call, result, risk };
		});
		const order = ['safe', 'normal', 'caution', 'danger'] as const;
		const risk = calls.reduce<ClearBatchView['risk']>(
			(worst, call) => (order.indexOf(call.risk) > order.indexOf(worst) ? call.risk : worst),
			'safe'
		);
		return { ...clear, batch: { ...clear.batch, calls, risk } };
	}
	const result = clear.result;
	const text = capText(guard, 0);
	if (result === null || text === null) return clear;
	return { ...clear, result: cap(result, text) };
}

/**
 * The guard's finite choice on an unlimited request, as the cap row prints it
 * — the single approval's, or batch leg `leg`'s: each call of a batch carries
 * its own decode, so each "Unlimited" is replaced by its own leg's cap.
 */
function capText(guard: GuardView, leg: number): string | null {
	const single = guard.surface === 'approval_editor';
	const batchLeg = guard.surface === 'batch' ? (guard.batch?.legs[leg] ?? null) : null;
	const detected = single ? guard.detected : (batchLeg?.approval ?? null);
	const editor = single ? guard.editor : (batchLeg?.editor ?? null);
	const meta = single ? guard.meta : (batchLeg?.meta ?? null);
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

/**
 * The request in one line, from the blocks the sheet already drew: what it is
 * and its figure ("Send · -1 xDAI") — so the status still says WHAT is on its
 * way once the form has gone (Android's `summaryOf`).
 */
export function summaryOf(blocks: Block[]): string | undefined {
	const intent = blocks.find((b) => b.kind === 'intent');
	const figure = blocks.flatMap((block) => {
		if (block.kind === 'amount') {
			return [`${block.line.sign}${block.line.value} ${block.line.symbol}`.trim()];
		}
		if (block.kind === 'swap') {
			return [
				`${block.pay.value} ${block.pay.symbol}`.trim() +
					' → ' +
					`${block.receive.value} ${block.receive.symbol}`.trim()
			];
		}
		return [];
	})[0];
	const line = [intent?.kind === 'intent' ? intent.text : '', figure ?? '']
		.filter((part) => part !== '')
		.join(' · ');
	return line === '' ? undefined : line;
}

/**
 * The sentence for the core's refusal key (`SignView.failure_refusal_key`,
 * PR 2 note 9): a refusal by its reason (`receipt.refusals`) — the held
 * nonce at submit among them (`componentsUi.signing.notSentBody`, PR 2
 * polish) — or a confirm block's line. A key this build does not carry is
 * the plain refusal — never a dotted path; no key, no sentence.
 */
export function refusalWords(
	key: string | null | undefined,
	m: SigningMessages
): string | undefined {
	if (key === null || key === undefined) return undefined;
	return m.receipt.refusals[key] ?? m.confirmBlock[key] ?? m.receipt.refused;
}

/**
 * Spec 079 (F11): once the person has approved, the sheet is a STATUS — never
 * the form with a greyed confirm (the owner: "可信签名器签完后，回到签名提示框，
 * 似乎没有任何提示"). Spec 082 (RA9, G22): its words follow the core's
 * `SignView.phase`, the stage the pipeline is really in:
 *
 * - `preparing` — the precheck, the sponsor, the relay's estimate:
 *   "Preparing transaction…" (`send.txPreparing`). Before 082 this said
 *   "Waiting for biometric…" through ~40 s of network work, while no prompt
 *   was up;
 * - `awaiting_signature` — the passkey prompt is up: "Waiting for
 *   biometric…" (`send.txSigning`);
 * - `submitting` — signed, going to the relay: "Submitting to network…" +
 *   "closing keeps it running" (`send.txSubmitting`, `send.txBackgroundHint`);
 * - a message never submits: "Signing…" throughout;
 * - the submission failed: the receipt's "Failed" + "your funds are safe"
 *   (`send.txErrorGeneric`) — the page was told already.
 *
 * Once the relay accepts the operation (or may have: MaybeSent) the host's
 * landing takes over. `null` = still a request.
 */
export function signingStatus(
	sign: SignView,
	progress: Pick<ApprovalProgress, 'signed' | 'ceremonyUp'> | undefined,
	summary: string | undefined,
	m: SigningMessages
): SigningStatus | null {
	const request = sign.request;
	if (!request || sign.surface === 'hidden') return null;
	const lines = (...parts: (string | undefined)[]) =>
		parts.filter((part): part is string => part !== undefined && part !== '');
	const error = sign.error;
	// Spec 099 R8: a passkey that failed is a failure of the request too,
	// said as the signer's. Spec 102 (P2b-W1): so is an account that cannot
	// sign here — said with the core's reason (`venue_block`), in the
	// person's words.
	const signerReason =
		error === null
			? undefined
			: error.kind === 'venue_blocked' && error.venue_block
				? venueBlockText(error.venue_block, m.venueBlock)
				: m.signerReasons[error.kind];
	if (
		error !== null &&
		error.kind !== 'user_rejected' &&
		(sign.pending_op_hash !== null || error.kind === 'submit_failed' || signerReason !== undefined)
	) {
		// PR 2 polish: the relay turned it back because the account's previous
		// operation on this network still holds the nonce. Nothing was sent and
		// nothing went wrong — "Not sent yet", calmly (the waiting disc, never
		// the failure's red), over the core's sentence, with Try again.
		const notSent = sign.failure_not_sent;
		return {
			stage: notSent ? 'not_sent' : 'failed',
			title: notSent ? m.status.notSentTitle : m.receipt.failed,
			// Spec 082 RJ3: the relay refused it — say so, with no "try
			// again": the same op is refused the same way. PR 2 note 9: and say
			// WHY, by the core's one sentence for both ways a refusal arrives
			// (`failure_refusal_key`): at submit (another operation of the
			// account holds the nonce — that one IS retryable), or by the
			// tracker's verdict after it (the fee sentence only for a fee
			// refusal, "went first" for a spent nonce).
			captions: lines(
				summary,
				refusalWords(sign.failure_refusal_key, m) ??
					(sign.failure_refused ? m.receipt.refused : (signerReason ?? m.status.failedHint))
			),
			closable: true,
			// Spec 096 F8: the page waits for this close to hear the failure;
			// "Try again" only when the core says nothing was sent and it was
			// no refusal (`failure_retryable`).
			actions: {
				close: m.receipt.done,
				...(sign.failure_retryable ? { retry: m.status.retry } : {})
			}
		};
	}
	const phase = sign.phase;
	if (phase === 'idle') return null;
	// Past the signature, and no prompt still up (a Trusted Signer page can
	// outlive its answer by a beat): a plain close, the operation carries on.
	const closable = phase === 'submitting' && progress?.ceremonyUp !== true;
	const onChain = request.kind === 'transaction' || request.kind === 'batch';
	if (!onChain) {
		return {
			stage: 'submitting',
			title: m.status.messageSigning,
			captions: lines(summary),
			closable
		};
	}
	switch (phase) {
		case 'preparing':
			return {
				stage: 'submitting',
				title: m.status.preparing,
				captions: lines(summary),
				closable: false
			};
		case 'awaiting_signature':
			return {
				stage: 'submitting',
				title: m.status.signing,
				captions: lines(summary),
				closable: false
			};
		case 'submitting':
			return {
				stage: 'submitting',
				title: m.status.submitting,
				captions: lines(summary, m.status.backgroundHint),
				closable
			};
	}
}

/**
 * What the sheet's ✕ tells the core (spec 079). Before the approval it is the
 * refusal (`reject_tapped` → 4001). After it, a plain close (`dismiss_tapped`):
 * the operation carries on, is tracked, and the page still gets its answer —
 * and only when the status says it may (`closable`); otherwise nothing.
 */
export function signingCloseEvent(
	status: SigningStatus | null | undefined
): 'reject_tapped' | 'dismiss_tapped' | null {
	if (!status) return 'reject_tapped';
	return status.closable ? 'dismiss_tapped' : null;
}

/**
 * Spec 082 RJ19 (G57): the relay's own estimate of this operation says it
 * will revert. The web runs no simulation (RG6), so this is the one voice
 * that can say it before the confirm — in the danger tone, under the intent.
 * The confirm stays live: a warning informs, it never blocks (L-D5); a submit
 * then meets the relay's refusal, answered as one (RJ3).
 */
function withEstimateVerdict(blocks: Block[], inputs: SigningLiveInputs): Block[] {
	const { sign, m, identity } = inputs;
	const request = sign.request;
	if (!request || sign.blocked) return blocks;
	if (request.kind !== 'transaction' && request.kind !== 'batch') return blocks;
	const reverts = estimateRevertsFor(request.chain_id, request.signer_address ?? identity.address);
	if (!reverts) return blocks;
	const text = reverts.reason
		? fill(m.warnWillFailReason, { reason: reverts.reason })
		: m.warnWillFail;
	const at = blocks.findIndex((block) => block.kind === 'intent');
	const next = [...blocks];
	next.splice(at + 1, 0, { kind: 'warning', tone: 'danger', text, verdict: true });
	return next;
}

/**
 * The whole sheet. `null` while the core is showing nothing — the route
 * renders no sheet at all then, rather than an empty one.
 */
export function buildSigningModel(raw: SigningLiveInputs): SigningModel | null {
	if (raw.sign.surface === 'hidden' || !raw.sign.request) return null;
	const inputs = {
		...raw,
		clear: cappedApproval(localizedTerms(raw.clear, raw.m), raw.guard)
	};
	const { sign, clear, guard, fee, m, identity, identicon } = inputs;
	if (sign.surface === 'hidden' || !sign.request) return null;

	const request = sign.request;
	const dapp = request.dapp;
	const host = new URL(request.origin).host;
	// Spec 082 RE7 (F14): the core's `site_label` — a name that IS its host
	// is said once, and the host line stays whenever it adds something.
	const label = browserSiteLabel(dapp?.name ?? '', host);
	const name = label.name;
	// The wallet's own request (the key backup) is the shell's say-so, carried
	// by the core (`first_party`) — never the origin: a page served from the
	// wallet's own host could otherwise borrow the wallet's name.
	const own = request.first_party;

	// Rule 1: the gate is an AND — the request, the guard, the reading and the
	// fee at the speed in force. Spec 099 R7: the AND is the core's
	// (`sign_confirm::confirm_state`), the same on every client, and it says
	// which part is shut; the line under a shut confirm is its corpus key.
	// PR 3: and, last, the sheet's own simulation — while its verdict is out
	// (`sign.sim_checking`) the core holds the confirm and names the line
	// ("Checking what this transaction does…"). Nothing is ANDed on here.
	const confirmState = signConfirmState(sign, guard, clear, fee, inputs.speed?.view.tier ?? null);
	const enabled = confirmState?.enabled === true;
	const note = !enabled && confirmState?.key ? m.confirmBlock[confirmState.key] : undefined;

	const drawn = withSimVerdict(withEstimateVerdict(blocksFor(inputs), inputs), inputs, own);
	const status = sign.blocked ? null : signingStatus(sign, inputs.progress, summaryOf(drawn), m);
	// The wallet's own request names no requester: its intent is the header's
	// headline, beside the ✕, and is not said a second time below it. While
	// the reading is still out there is no intent yet; the sheet's own title
	// holds the line until it lands.
	const lead = own && drawn[0]?.kind === 'intent' ? drawn[0] : undefined;
	const blocks = lead ? drawn.slice(1) : drawn;
	const headline = own
		? lead
			? { text: lead.text, tone: lead.tone }
			: { text: m.panelTitle, tone: 'neutral' as const }
		: undefined;

	return {
		id: 'cs1',
		// The wallet's own request (the key backup) is not a site, and its
		// header says no site (`headline`): `localhost:5173` under "Vela" read
		// as a stranger borrowing the brand (founder, 2026-09-19), and the
		// wallet's own mark and name over its own request said nothing either.
		dapp: own
			? { name: 'Vela Wallet', host: '', letter: 'V', tint: NEUTRAL_TINT }
			: {
					name,
					host: label.host_line ?? '',
					letter: letterOf(name),
					tint: NEUTRAL_TINT,
					iconUrls: siteIconUrls(request.origin)
				},
		network: {
			name: chainName(request.chain_id),
			dot: NEUTRAL_TINT,
			logoUrl: chainLogoURL(request.chain_id)
		},
		...(headline ? { headline } : {}),
		blocks,
		tech: techModel(inputs, own),
		techOpen: false,
		fee: sign.blocked ? { kind: 'hidden' } : feeModel(inputs),
		signer: {
			label: m.signingAccount,
			name: identity.name,
			identiconSvg: identicon(identity.address),
			address: identity.address
		},
		// Spec 081: refused — no fee, no confirm, one way out.
		dismissOnly: sign.blocked ? m.close : undefined,
		confirm: {
			/*
			 * The control's whole label is the action, a PHRASE ("Confirm
			 * send", "Copy this wallet's record"), never a sentence or a template: a
			 * template fallback once put "{{action}}" on screen (spec 027 T340).
			 * With no intent from the core, the generic word is the honest one.
			 */
			action:
				clear.surface === 'plain_send' && clear.plain_send
					? clear.plain_send.no_value
						? m.confirmPlain
						: m.confirmSend
					: clear.confirm.type === 'confirm_intent'
						? clear.confirm.intent
						: m.confirmPlain,
			enabled,
			...(note ? { note } : {})
		},
		closeLabel: m.close,
		...(status ? { status } : {}),
		panelTitle: m.panelTitle
	};
}

/**
 * What the approve carries — every field a copy of a machine's view, none
 * decided here.
 *
 * The fee is the one this sheet DISPLAYED (`fee_policy`'s view), signed
 * verbatim — amount, recipient and the speed it was priced at (spec 069). The
 * params override is the GUARD's rewrite: the capped approval, not the
 * requested one (passing the original would defeat the never-unlimited
 * mandate at the last step), and `unlimited_approved` its one waiver.
 *
 * Spec 093: the intent the record keeps is `clear_signing`'s `record_intent`
 * (`null` when nothing may be recorded — a best-effort guess, a batch with no
 * shared verb), and `token_meta` the guard's token as it resolved, so the
 * record can say "100 USDC" rather than a bare number. Whether a token that
 * has not resolved counts is the core's to say, not this side's. Spec 097:
 * `reading` is `clear_signing`'s `record_reading`, verbatim.
 */
export function approveOptsOf(
	fee: FeeView | null | undefined,
	clear: ClearSigningView,
	guard: GuardView
): SignApproveOpts {
	const quote = fee?.fee ?? null;
	return {
		max_fee_per_gas: quote ? quote.max_fee_per_gas : null,
		bundler_cost_wei: null,
		gas_fee_token: fee?.fee_token ?? null,
		// No recipient, no in-band quote: the core's own rule.
		quoted_fee:
			quote && quote.fee_recipient
				? {
						amount: quote.fee_asset.type === 'erc20' ? quote.fee_asset.amount : quote.total_wei,
						recipient: quote.fee_recipient,
						tier: quote.tier
					}
				: null,
		fee_collector: null,
		params_override_json: guard.rewritten_params_json,
		intent: clear.record_intent ?? null,
		unlimited_approved: guard.unlimited_consented,
		token_meta: guard.meta,
		// Spec 097: what the reading named — the contract and the coins — kept
		// with the record so Activity names them as this sheet did.
		reading: clear.record_reading ?? null
	};
}
