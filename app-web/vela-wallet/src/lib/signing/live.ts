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
import type { ClearLineSource } from '$lib/core/generated/ClearLineSource';
import type { ClearMessageLine } from '$lib/core/generated/ClearMessageLine';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { GuardEditorView } from '$lib/core/generated/GuardEditorView';
import type { GuardView } from '$lib/core/generated/GuardView';
import type { SignApproveOpts } from '$lib/core/generated/SignApproveOpts';
import type { SignView } from '$lib/core/generated/SignView';
import {
	feeAmountText,
	feeLineParts,
	feeOptionPriceUsd,
	feeParts,
	NATIVE_FEE_PLACES,
	TOKEN_FEE_PLACES
} from '$lib/flows/fee-line';
import { offeredTier, speedControlModel } from '$lib/flows/speed-control';
import type { FeeSpeedModel } from '$lib/flows/model';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import { chainLogoURL, tokenMarkFor } from '$lib/flows/marks';
import { browserSiteLabel, checksumAddress, signConfirmState } from '$lib/core/kernels';
import { estimateRevertsFor } from '$lib/services/estimate-verdict';
import { exactAmount, moneyText, trimBalance } from '$lib/wallet/live';
import { showsFiat } from '$lib/network-space/fiat';
import type { NetworkSpace } from '$lib/network-space/network-space.svelte';
import { fromBaseUnits } from '$lib/services/eip681';
import { chainName, nativeSymbol } from '$lib/services/networks';
import { shortenAddress } from '$lib/wallet/identity';
import type { WalletIdentity } from '$lib/wallet/identity';
import { fill } from '$lib/wallet/messages';
import { venueBlockText } from '$lib/settings/venue';
import { getEthereumDataURL } from '$lib/services/endpoints';
import { txParams } from './core/tx-params';
import { rawWords } from './raw-json';
import { failedFeeTappable } from '$lib/flows/fee-failure';
import { resolvedFormatKeys } from '$lib/services/locale-format';
import type { SigningMessages } from './messages';
import {
	checkingCard,
	couldNotCheck,
	landedVerdict,
	verdictRooms,
	type SimEnded,
	type VerdictSaid
} from './verdict';
import type {
	AllowanceChip,
	AmountLine,
	Block,
	CodeLine,
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
	 * r18-9 (spec 105): the space in front — the testnet space shows no fiat
	 * on the sheet, neither beside an amount nor beside the fee
	 * (`showsFiat`). Absent: the device's space.
	 */
	space?: NetworkSpace;
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
	progress?: Pick<ApprovalProgress, 'signed' | 'ceremonyUp'> & {
		/** Round 7 (S4): the account's trusted signing page has the request now. */
		onPage?: boolean;
	};
	/**
	 * The sheet's own simulation of THIS request, once it has ended: the
	 * core's reading of the node's reply (`simOutcome`) and, for a check, the
	 * trust machine's judged view of what it moves (`verdict.ts`). It is the
	 * host's one `eth_simulateV1` read, which also tells the fee machine what
	 * the calls move. Absent while the simulation is out, for a request
	 * nobody simulated (a message), and for any other request than the one
	 * it was run for.
	 */
	sim?: SimEnded | null;
	/**
	 * dapp-15: the simulation of THIS request is being asked again — its node
	 * is back, or the person pressed "Check again". `sim` is the last
	 * verdict, drawn stale until the new one lands.
	 */
	simRechecking?: boolean;
	/**
	 * cs-10: the message on the sheet runs past its window and its last line
	 * has not been in sight yet — the sheet's observation (`CodeBlock`), for
	 * the core's gate to hold the confirm on (`ConfirmBlock::MessageUnread`).
	 * Absent: nothing observed, nothing held.
	 */
	messageUnread?: boolean;
	/**
	 * R22: the person's "Refuse blind signing" (`refuseBlindSigning`) — every
	 * request that signs only "… anyway" is refused by the core's gate.
	 */
	refuseBlind?: boolean;
	/**
	 * R22: where a network's RPC settings open ("Only use my node"), for the
	 * hint under a simulation nobody independent confirmed. Absent: none.
	 */
	ownNodeHref?: (chainId: number) => string;
	/**
	 * R22: whether a network is read through the person's own node alone
	 * ("Only use my node") — its checks carry the own-node line, and the
	 * verdict's place keeps room for it. Absent: none is.
	 */
	ownNodeOnly?: (chainId: number) => boolean;
	m: SigningMessages;
	identity: WalletIdentity;
	identicon: (seed: string) => string;
	/**
	 * The descriptor service's host, as the provenance line names it
	 * ("… via ERC-7730 · ethereum-data.getvela.app · unauthenticated").
	 * Absent: read from the service settings.
	 */
	descriptorHost?: string;
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
function fieldRow(field: ClearSignField, inputs?: SigningLiveInputs): KeyValueRow {
	const source = inputs ? labelSource(field.source, inputs) : undefined;
	return {
		label: field.label,
		value: field.this_account && inputs ? inputs.m.certain.thisAccount : field.value,
		valueTone: field.warning ? 'danger' : field.unverified ? 'caution' : undefined,
		// An address reads as monospace; a contract the core names
		// ("PancakeSwap Permit2", 096 F5) is a name, in the text face.
		mono:
			!(field.this_account && inputs) &&
			((field.address !== null && field.value.startsWith('0x')) || field.token_address !== null),
		...(source ? { source } : {})
	};
}

/**
 * The descriptor service's host, for the provenance line: the one the
 * person's settings point at (which they can edit), never a name the
 * descriptor gives itself.
 */
function descriptorHostOf(inputs: SigningLiveInputs): string {
	if (inputs.descriptorHost !== undefined) return inputs.descriptorHost;
	try {
		return new URL(getEthereumDataURL()).host;
	} catch {
		return '';
	}
}

/** Who a line's label is from, in the words a row's marker says it. */
/** What a reading says when nothing graded it (`ClearReading::default`). */
const UNGRADED_READING: ClearSignResult['reading'] = {
	source: 'vela',
	descriptor_owner: null,
	descriptor_path: null,
	contract_named_by: null,
	contradictions: [],
	selector_name: null,
	goes_elsewhere: false,
	dust_minimum: false,
	allowance: false,
	allowance_adds: false,
	shape_intent: null,
	collection_name: null
};

function sourceName(source: ClearLineSource | undefined, inputs: SigningLiveInputs): string | null {
	const c = inputs.m.certain;
	switch (source ?? 'vela') {
		case 'vela':
			return null;
		case 'built_in':
			return c.sourceBuiltIn;
		case 'fetched':
			return fill(c.sourceFetched, { host: descriptorHostOf(inputs) });
		case 'selector_db':
			return c.sourceSelectorDb;
		case 'shape':
			return c.sourceShape;
	}
}

/**
 * Who read a call, said as the sheet's "Read by" row says it: Vela, from
 * the transaction — or whose description, or a name database, or a shape.
 */
export function readBy(source: ClearLineSource, m: SigningMessages): string {
	return sourceName(source, { m } as SigningLiveInputs) ?? m.certain.byVela;
}

/** "Label from …" — the faint marker on a row a description labelled (cs-14). */
function labelSource(
	source: ClearLineSource | undefined,
	inputs: SigningLiveInputs
): string | undefined {
	const name = sourceName(source, inputs);
	return name === null ? undefined : fill(inputs.m.certain.labelFrom, { source: name });
}

/**
 * The one small line under the intent that says where the reading came from
 * (cs-14): what Vela read from the bytes, or whose description it is — and a
 * description is not a guarantee.
 */
function provenanceBlock(result: ClearSignResult, inputs: SigningLiveInputs): Block {
	const c = inputs.m.certain;
	const reading = result.reading;
	const owner = reading.descriptor_owner;
	switch (reading.source) {
		case 'vela':
			return {
				kind: 'source',
				text: result.sign_type === 'signature' ? c.readByVelaSignature : c.readByVela,
				tone: 'neutral'
			};
		case 'built_in':
			return {
				kind: 'source',
				text: owner ? fill(c.describedBuiltIn, { owner }) : c.describedBuiltInPlain,
				tone: 'neutral'
			};
		case 'fetched': {
			const host = descriptorHostOf(inputs);
			return {
				kind: 'source',
				text: owner
					? fill(c.describedFetched, { owner, host })
					: fill(c.describedFetchedPlain, { host }),
				tone: 'neutral'
			};
		}
		case 'shape':
			return {
				kind: 'source',
				text: fill(c.shapeOnly, { intent: result.intent }),
				tone: 'neutral'
			};
		case 'selector_db':
			// A name anyone can register is a caution in itself (cs-7).
			return {
				kind: 'warning',
				tone: 'caution',
				text: fill(c.selectorName, { name: reading.selector_name ?? '' })
			};
	}
}

/**
 * The contract on the card's face (cs-8, cs-14): its address, and its name
 * with who called it so — or that nobody named it, or that nothing lives
 * there.
 */
function contractBlock(
	address: string | null,
	name: string | null,
	namedBy: ClearLineSource | null,
	noCode: boolean,
	inputs: SigningLiveInputs
): Block | null {
	if (!address) return null;
	const c = inputs.m.certain;
	const short = shortenAddress(address);
	if (noCode)
		return { kind: 'source', text: fill(c.contractNoCode, { address: short }), tone: 'danger' };
	if (!name)
		return { kind: 'source', text: fill(c.contractUnnamed, { address: short }), tone: 'neutral' };
	const source =
		namedBy === 'fetched'
			? fill(c.sourceFetched, { host: descriptorHostOf(inputs) })
			: ((namedBy && namedBy !== 'built_in' ? sourceName(namedBy, inputs) : null) ?? c.sourceList);
	return {
		kind: 'source',
		text: fill(c.contractNamed, { address: short, name, source }),
		tone: 'neutral'
	};
}

/**
 * An approve-all's collection: the name a list or a description gave its
 * contract; else its own `name()` beside its short address — the
 * contract's word, so never alone; else the short address.
 */
function collectionOf(result: ClearSignResult): string {
	if (result.contract_name) return result.contract_name;
	const short = result.contract_address ? shortenAddress(result.contract_address) : '';
	const own = result.reading.collection_name;
	return own ? `${own} (${short})` : short;
}

/** The symbol an amount or a Token row names: "250 USDC?" → "USDC?". */
function symbolIn(value: string): string {
	return value.split(' (')[0].trim().split(' ').pop() ?? value;
}

/**
 * The lines the certain decode raises about a reading (cs-1…cs-4): what a
 * description said against it, an output paid to a stranger, a dust minimum,
 * and each token whose name is only its own contract's word. `prefix` marks
 * a batch leg ("#3 ").
 */
function certainLines(result: ClearSignResult, inputs: SigningLiveInputs, prefix = ''): Block[] {
	const c = inputs.m.certain;
	const blocks: Block[] = [];
	for (const words of result.reading.contradictions) {
		blocks.push({
			kind: 'warning',
			tone: 'danger',
			text: prefix + fill(c.contradiction, { words })
		});
	}
	if (result.reading.goes_elsewhere) {
		blocks.push({ kind: 'warning', tone: 'danger', text: prefix + c.elsewhere });
	}
	if (result.reading.dust_minimum) {
		blocks.push({ kind: 'warning', tone: 'caution', text: prefix + c.dustMinimum });
	}
	const seen = new Set<string>();
	for (const field of result.fields) {
		if (!field.token_named_by_contract || !field.token_address || seen.has(field.token_address)) {
			continue;
		}
		seen.add(field.token_address);
		blocks.push({
			kind: 'source',
			tone: 'caution',
			text:
				prefix +
				fill(c.tokenByContract, {
					symbol: symbolIn(field.value),
					address: shortenAddress(field.token_address)
				})
		});
	}
	return blocks;
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
	currency: SigningLiveInputs['currency'],
	space?: NetworkSpace
): AmountLine {
	return {
		// Spec 097 N1: an amount nobody could scale (the core's em dash) has
		// no figure to sign — "+— 0x8ac7…" read as a typo, not as unknown.
		sign: field.unverified ? '' : outgoing ? MINUS : '+',
		value: field.value,
		symbol: '',
		// r18-9: no worth beside a test coin (the testnet space).
		fiat:
			field.usd_value === null || !showsFiat(space)
				? undefined
				: `≈ ${moneyText(field.usd_value, currency)}`,
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
function allowanceChips(
	editor: GuardEditorView,
	m: SigningMessages,
	boolean = false
): AllowanceChip[] {
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
	// cs-17: a grant-all has no amount to cap — its choice is to allow all
	// or to revoke, and the core waits for that deliberate tap.
	if (boolean) {
		return [
			{ id: 'grant', label: m.certain.chipGrantAll, state: state('grant') },
			{ id: 'revoke', label: m.chipRevoke, state: state('revoke') }
		];
	}
	const chips: AllowanceChip[] = [
		{ id: 'requested', label: m.chipRequested, state: state('requested') },
		{ id: 'balance', label: m.chipBalance, state: state('balance') },
		{ id: 'custom', label: m.chipCustom, state: state('custom') }
	];
	// cs-16: increaseAllowance can only re-encode its increment, so "Revoke"
	// would sign an increase of 0 — not offered, not drawn.
	if (editor.revoke_offered)
		chips.push({ id: 'revoke', label: m.chipRevoke, state: state('revoke') });
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
		blocks.push({ kind: 'warning', tone: 'danger', text: guardWarningText(guard, m) });
	if (guard.surface === 'permit_sign') {
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnPermitCantCap });
	}
	return blocks;
}

/**
 * §37 (review 1-12): the `unlimited_warning` sentence is the one the core
 * names (`unlimited_warning_key`) — "Unlimited — …", or "Over 100× your
 * balance — …" for an amount far above what the account holds. A key this
 * build has no words for says the unlimited line, never nothing.
 */
export function guardWarningText(guard: GuardView, m: SigningMessages): string {
	return m.guardWarnings?.[guard.unlimited_warning_key] || m.warnUnlimited;
}

function guardBlock(guard: GuardView, m: SigningMessages): Block | null {
	if (guard.surface !== 'approval_editor' || !guard.editor) return null;
	return allowanceBlock(guard.editor, guard.meta, m, {
		decimalsUnverified: guard.decimals_unverified,
		increaseTotal: guard.increase_total,
		boolean: guard.detected?.is_boolean_grant === true,
		adds: guard.detected?.kind === 'increase_allowance'
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
		/** A grant-all (setApprovalForAll): allow all or revoke, no amount. */
		boolean?: boolean;
		/** increaseAllowance: the amount ADDS to what is allowed (cs-16). */
		adds?: boolean;
	}
): Block {
	const symbol = meta.loading ? '…' : meta.symbol;
	if (extra.boolean) {
		const picked = editor.choice?.type;
		return {
			kind: 'allowance',
			leg: extra.leg,
			label: m.terms.labelApproved ?? m.labelOperator,
			value: picked === 'grant' ? m.certain.chipGrantAll : picked === 'revoke' ? m.chipRevoke : '—',
			valueTone: picked === 'grant' ? 'danger' : 'neutral',
			chips: allowanceChips(editor, m, true)
		};
	}
	// Only a chosen, finite cap reads as settled; the site's unlimited ask,
	// kept, reads as the danger it is (and `guardWarnings` adds the sentence).
	const settled = editor.choice !== null && editor.choice.type !== 'unlimited';
	const total = extra.increaseTotal ?? null;
	return {
		kind: 'allowance',
		leg: extra.leg,
		label: extra.adds
			? `${m.terms.labelAddsToAllowance ?? m.labelSpendingCap} · ${symbol}`
			: fill(m.labelSpendingCap, { symbol }),
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
		// In tokens, like the cap above it: "50000000" read as fifty million.
		resultingTotal:
			total === null || total.total === null
				? undefined
				: {
						label: m.labelResultingTotal,
						value: `${exactAmount(fromBaseUnits(BigInt(total.total), meta.decimals))} ${symbol}`
					},
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

/**
 * r18-23: a Safe as the parent sentence names it — its EIP-55 checksummed
 * form, shortened (the core's `checksumAddress`), never the lowercase a
 * request carried: the letters' case is what a person checks it by.
 */
function safeShort(address: string): string {
	try {
		return shortenAddress(checksumAddress(address));
	} catch {
		return shortenAddress(address);
	}
}

/**
 * The sentence under a refused request (spec 081). §31: one that would have
 * made this account act for a Safe it owns (`blocked.parent`) says that
 * Safe and what was asked of it — or, when whether it owns it could not be
 * read, that Vela refused rather than guess.
 */
export function selfCallBlockedText(
	blocked: NonNullable<SignView['blocked']>,
	m: SigningMessages
): string {
	if (blocked.parent != null) {
		return fill(blocked.reason === 'unchecked' ? m.parentSafeUnchecked : m.parentSafeBlocked, {
			function: blocked.function,
			safe: safeShort(blocked.parent)
		});
	}
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
 * §37 (review 1-11): a refused sign-in's sentence — the core's key, filled
 * with the core's named params (`domain`, `origin`, `uri`, `chain`,
 * `connected`, `field`). A key this build has no words for says the title.
 */
export function signInRefusedText(
	signIn: NonNullable<NonNullable<SignView['blocked']>['sign_in']>,
	m: SigningMessages
): string {
	const words = m.signIn?.[signIn.key];
	return words ? fill(words, signIn.params as Record<string, string>) : m.signInRefusedTitle;
}

/**
 * 089 S1: one call of a batch, as its own card — the same words its call
 * would get alone. A decoded call is its intent and its fields, then the coin
 * it moves and whom it calls; a plain send is "Send", the exact amount and
 * the recipient; a call nobody could read says so in its title, with whom it
 * calls and what coin it moves. Never omitted: the core hands a row for every
 * call.
 */
function batchCallCard(
	call: ClearBatchCall,
	symbol: string,
	m: SigningMessages,
	inputs?: SigningLiveInputs
): Block {
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
			: call.no_code
				? [
						{
							label: m.labelInteracting,
							value: fill(m.certain.contractNoCode, { address: shortenAddress(call.to) }),
							valueTone: 'danger'
						}
					]
				: call.to_name
					? [{ label: m.labelInteracting, value: call.to_name }]
					: [{ label: m.labelInteracting, value: call.to, mono: true }];
	if (call.surface === 'clear_sign' && call.result) {
		// cs-14: every leg says whose reading it is.
		const source = inputs ? sourceName(call.result.reading.source, inputs) : null;
		const readBy: KeyValueRow[] = inputs
			? [{ label: m.certain.labelSource, value: source ?? m.certain.byVela }]
			: [];
		return {
			kind: 'card',
			title: step(
				call.result.reading.selector_name !== null ? m.terms.intentContractCall : call.result.intent
			),
			tone,
			rows: [
				...call.result.fields.filter((field) => !field.detail).map((f) => fieldRow(f, inputs)),
				...coin,
				...target,
				...readBy
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
		...batch.calls.map((call) => batchCallCard(call, symbol, m, inputs))
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
	// cs-14: what the certain decode says against a leg's description, by leg.
	for (const call of batch.calls) {
		const prefix = `#${call.index} `;
		if (call.no_code && sign.request) {
			blocks.push({
				kind: 'warning',
				tone: 'danger',
				text: prefix + fill(m.certain.noCode, { chain: chainName(sign.request.chain_id) })
			});
		}
		if (call.result) blocks.push(...certainLines(call.result, inputs, prefix));
	}
	const results = batch.calls.flatMap((call): ClearSignResult[] =>
		call.result ? [call.result] : []
	);
	if (results.some((r) => r.to_own_token)) {
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnDrain });
	}
	// A fetched description is the service's word; a shape match is no one's
	// (cs-15), so it is not blamed on the service.
	if (results.some((r) => r.reading.source === 'fetched')) {
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
	for (const call of batch.calls) {
		const name = call.result?.reading.selector_name;
		if (name) {
			blocks.push({
				kind: 'warning',
				tone: 'caution',
				text: `#${call.index} ${fill(m.certain.selectorName, { name })}`
			});
		}
	}
	if (results.some((r) => r.fields.some((f) => f.unverified))) {
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnUnverifiedAmount });
	}
	if (inputs.clear.has_invisible_chars) {
		blocks.push({ kind: 'warning', tone: 'danger', text: m.warnHiddenChars });
	}
	return blocks;
}

/**
 * Spec 096 F7: the request is still being read. Its blocks are not there
 * yet — "Loading…" stands in their place — and the sheet is told so
 * (`SigningModel.reading`).
 */
const stillReading = (clear: ClearSigningView): boolean =>
	clear.surface === 'loading' || clear.resolving;

/**
 * cs-12: the request's own blocks and nothing around them — no header, fee,
 * verdict or confirm — read through the same two passes `buildSigningModel`
 * gives them. A multisig member reviewing a call someone else proposed sees
 * exactly what this sheet would have drawn for that call.
 */
export function requestBlocks(inputs: SigningLiveInputs): Block[] {
	return blocksFor({
		...inputs,
		clear: cappedApproval(localizedTerms(inputs.clear, inputs.m), inputs.guard)
	});
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
		// §37 (review 1-11): a sign-in that does not bind to the site asking
		// and its network — refused, and said as the sign-in it is, never as
		// a self-call.
		const signIn = sign.blocked.sign_in;
		if (signIn) {
			blocks.push({ kind: 'intent', text: m.signInRefusedTitle, tone: 'danger' });
			blocks.push({ kind: 'warning', tone: 'danger', text: signInRefusedText(signIn, m) });
			return blocks;
		}
		blocks.push({ kind: 'intent', text: m.selfCallBlockedTitle, tone: 'danger' });
		blocks.push({ kind: 'warning', tone: 'danger', text: selfCallBlockedText(sign.blocked, m) });
		return blocks;
	}

	// Spec 096 F7: still reading — a neutral "Loading…", and the confirm stays
	// shut (`buildSigningModel`). This said the cap editor's "Set a finite
	// amount to continue." over an Aave supply for seconds.
	if (stillReading(clear)) {
		blocks.push({ kind: 'sentence', text: m.loading, tone: 'neutral' });
		return blocks;
	}

	if (clear.surface === 'batch' && clear.batch) {
		return batchBlocks(inputs, clear.batch);
	}

	const result = clear.result;
	if (result) {
		const reading = result.reading;
		const c = m.certain;
		// cs-7: a name from the public database is no headline.
		const intent =
			reading.selector_name !== null
				? m.terms.intentContractCall
				: result.intent_term === 'intentApproveAllNfts' && reading.source === 'vela'
					? fill(c.approveAllYour, {
							collection: collectionOf(result)
						})
					: result.intent;
		blocks.push({ kind: 'intent', text: intent, tone: toneOf(result.risk) });
		blocks.push(provenanceBlock(result, inputs));

		const send = result.fields.find((f) => f.role === 'send_amount');
		const receive = result.fields.find((f) => f.role === 'receive_amount');
		const spender = result.fields.find((f) => f.role === 'spender');
		const currency = inputs.currency;
		// An output paid to a stranger is not "+" this account's (cs-2).
		const received = (field: ClearSignField): AmountLine => {
			const line = amountLine(field, false, currency, inputs.space);
			return reading.goes_elsewhere ? { ...line, sign: '', tone: 'neutral' } : line;
		};
		if (reading.allowance && send) {
			// cs-18: an approval's amount is a ceiling somebody else may spend,
			// never an amount leaving now.
			blocks.push({
				kind: 'sentence',
				text: fill(reading.allowance_adds ? c.allowsMore : c.allowsUpTo, {
					spender: spender ? (spender.this_account ? c.thisAccount : spender.value) : '—',
					amount: send.value
				}),
				tone: send.warning ? 'danger' : 'neutral'
			});
		} else if (send && receive) {
			blocks.push({
				kind: 'swap',
				pay: amountLine(send, true, currency, inputs.space),
				receive: received(receive)
			});
		} else if (send) {
			blocks.push({ kind: 'amount', line: amountLine(send, true, currency, inputs.space) });
		} else if (receive) {
			blocks.push({ kind: 'amount', line: received(receive) });
		}
		// Spec 096 F4: the coin the call sends, when its reading does not say
		// it — the hero when nothing else is, else the batch call's own row.
		const coin = nativeValueBlock(inputs, !send && !receive);
		if (coin) blocks.push(coin);

		for (const field of result.fields) {
			if (field.role !== 'recipient' && field.role !== 'spender') continue;
			const source = labelSource(field.source, inputs);
			blocks.push({
				kind: 'party',
				label: field.label,
				name: field.this_account ? c.thisAccount : field.value,
				address: field.address ?? undefined,
				badge: field.unverified ? { text: m.tagUnverified, tone: 'caution' } : undefined,
				...(source ? { source } : {})
			});
		}

		// The guard's editor sits with the approval it caps.
		const allowance = guardBlock(guard, m);
		if (allowance) blocks.push(allowance);
		blocks.push(...legBlocks(guard, m));
		blocks.push(...guardWarnings(guard, m));

		// Whatever the core flagged, said once, in its own words.
		blocks.push(...certainLines(result, inputs));
		if (result.to_own_token) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnDrain });
		}
		// cs-11: a value (a typed-data string, a call's) carried characters
		// that draw as nothing or turn the text around; the core drew each as
		// its ⟨U+…⟩ marker, and the sheet says why those are there.
		if (clear.has_invisible_chars) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnHiddenChars });
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
		if (reading.source === 'fetched') {
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
		// A best-effort reading said where its name came from in its own line
		// (`provenanceBlock`, cs-7).

		const rest = result.fields.filter((f) => f.role === 'generic' && f !== send && f !== receive);
		if (rest.length > 0) blocks.push({ kind: 'rows', rows: rest.map((f) => fieldRow(f, inputs)) });
		// cs-8/cs-14: the contract, on the face.
		const contract = contractBlock(
			result.sign_type === 'transaction' ? result.contract_address : result.contract_address,
			result.contract_name,
			reading.contract_named_by,
			false,
			inputs
		);
		if (contract) blocks.push(contract);
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

	/*
	 * dapp-9: typed data no description reads. It is no blind signature of "0
	 * bytes": the message is structured, so it is drawn as one — what it is
	 * signed for (the domain, its type, the site), the contract it names, and
	 * every field with its value as the payload wrote them (the core's
	 * `blind_typed`, display-safe) — under the caution that nobody described
	 * what it means.
	 */
	if (clear.surface === 'blind_typed_data' && clear.blind_typed) {
		const typed = clear.blind_typed;
		blocks.push({ kind: 'intent', text: m.intentTypedData, tone: 'caution' });
		blocks.push({ kind: 'warning', tone: 'caution', text: m.warnBlindTyped });
		const facts: KeyValueRow[] = [
			...(typed.domain_name ? [{ label: m.labelTypedDomain, value: typed.domain_name }] : []),
			...(typed.primary_type ? [{ label: m.labelType, value: typed.primary_type }] : []),
			...(sign.request ? [{ label: m.labelSigningFor, value: hostOf(sign.request.origin) }] : [])
		];
		if (facts.length > 0) blocks.push({ kind: 'rows', rows: facts });
		const target = contractBlock(typed.verifying_contract, null, null, false, inputs);
		if (target) blocks.push(target);
		if (typed.fields.length > 0) {
			blocks.push({
				kind: 'rows',
				rows: typed.fields.map((field) => ({
					label: field.key,
					value: field.value,
					mono: /^0x[0-9a-fA-F…]+$/.test(field.value)
				}))
			});
		}
		if (typed.has_invisible_chars) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnHiddenChars });
		}
		// A permit nobody described still gets its cap and its warning.
		const allowance = guardBlock(guard, m);
		if (allowance) blocks.push(allowance);
		blocks.push(...guardWarnings(guard, m));
		return blocks;
	}

	// No decode at all — the deepest rung. The core said so; the sheet says so.
	if (clear.surface === 'blind_transaction' || clear.surface === 'blind_typed_data') {
		blocks.push({ kind: 'intent', text: m.intentBlind, tone: 'danger' });
		// Nobody could read the call — what coin it sends is still known.
		const coin = nativeValueBlock(inputs, true);
		if (coin) blocks.push(coin);
		if (clear.no_code && sign.request) {
			// cs-13: nothing lives at the target on this chain.
			blocks.push({
				kind: 'warning',
				tone: 'danger',
				text: fill(m.certain.noCode, { chain: chainName(sign.request.chain_id) })
			});
		}
		blocks.push({ kind: 'warning', tone: 'danger', text: fill(m.warnBlindDecode, { bytes }) });
		// cs-8: a call nobody could read still names its contract on the face.
		if (clear.surface === 'blind_transaction' && sign.request) {
			const target = contractBlock(
				txParams(sign.request.params_json)?.to ?? null,
				null,
				null,
				clear.no_code,
				inputs
			);
			if (target) blocks.push(target);
		}
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
			// cs-10/cs-11: the lines the core decided to draw — hidden
			// characters as markers, long runs of empty lines folded — and
			// all of them read before the confirm opens.
			lines: message.display_lines.map((line) => messageLine(line, m)),
			note: message.non_printable ? m.warnHexMessage : undefined,
			readToEnd: true
		});
		if (message.has_invisible_chars) {
			blocks.push({ kind: 'warning', tone: 'danger', text: m.warnHiddenChars });
		}
		if (message.binding === 'mismatch') {
			// cs-9: both names in the sentence — the host the sign-in names, as
			// the core compared it, and the host that asked.
			const siwe = message.siwe;
			const origin = sign.request ? hostOf(sign.request.origin) : '';
			blocks.push({
				kind: 'warning',
				tone: 'danger',
				text: fill(m.warnSiweMismatch, {
					domain: siwe?.domain_host ?? siwe?.domain ?? '',
					origin
				})
			});
		}
		return blocks;
	}

	return blocks;
}

/**
 * One line of a message as the core drew it (`ClearMessageView.display_lines`):
 * its text, with each hidden character's ⟨U+…⟩ marker muted; a folded run of
 * empty lines as one muted line that counts them.
 */
function messageLine(line: ClearMessageLine, m: SigningMessages): CodeLine {
	if (line.type === 'blank') {
		return [{ text: fill(m.messageBlankLines, { lines: String(line.count) }), muted: true }];
	}
	if (!line.runs.some((run) => run.marker)) return line.runs.map((run) => run.text).join('');
	return line.runs.map((run) =>
		run.marker ? { text: run.text, muted: true } : { text: run.text }
	);
}

/** The host of a request's origin, or the origin itself when it is not a URL. */
function hostOf(origin: string): string {
	try {
		return new URL(origin).host;
	} catch {
		return origin;
	}
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
	// r18-9: the testnet space has no money to withhold — none is drawn.
	const withheld =
		inputs.currency.committed || !showsFiat(inputs.space) ? {} : { valueFiatWithheld: true };
	// Device walk of 2026-10-11, item 2: the sheet has its final geometry for
	// everything the fee card can say from its first settled frame. It is
	// anchored at the bottom, so a line that ARRIVES in the card moved the
	// sheet's top, its header and its ✕ (measured here on a 390 × 844 screen:
	// 16 px for the speed's note, 47 with "No token can pay this fee", 27 for
	// a failure's reason). Said in every state of the fee — estimating, landed,
	// failed — so the room is the same before and after:
	// - the money keeps a line of its own under the coin, whatever it will
	//   read (side by side, a landed figure wrapped at 360 and grew the row);
	// - what a landed fee says when it cannot be paid has its line held under
	//   the card: no coin can pay, or the coin that pays is short. A failure's
	//   reason is said in that same place and brings no room of its own: the
	//   standing line under it is blank whenever a fee has failed.
	const paying =
		fee.options.find((option) => option.selected)?.symbol ??
		(sign.request ? nativeSymbol(sign.request.chain_id) : '');
	const still = {
		moneyLine: true,
		rooms: [m.feeNoCoinPays, fill(m.feeShort, { sym: paying })]
	};
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
			...still,
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
				...still,
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
	const line = feeLineParts(
		parts,
		feeOptionPriceUsd(parts.contract, fee.options),
		inputs.currency,
		inputs.space
	);
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
			? // r10-3: short only once the request's own amount of that coin is
				// taken first — said as amount + fee, the send form's sentence.
				fill(selected.short_with_amount === true ? m.feeShortWithAmount : m.feeShort, {
					sym: selected.symbol
				})
			: !measuring && selected?.spent_by_operation === true
				? fill(m.feeCoinSpent, { sym: selected.symbol })
				: undefined;
	return {
		kind: 'onchain',
		label: m.feeLabel,
		value: line.coin,
		...(line.fiat === null ? {} : { valueFiat: line.fiat }),
		...withheld,
		...still,
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
		feeAmountText(raw, decimals, contract === null ? NATIVE_FEE_PLACES : TOKEN_FEE_PLACES);
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
	const model = speedControlModel(speed.view, inputs.m.speed, (quote, tier) => {
		const options = speed.feeOptions(tier);
		const parts = feeParts(quote, options);
		return feeLineParts(
			parts,
			feeOptionPriceUsd(parts.contract, options),
			inputs.currency,
			inputs.space
		);
	});
	// The speed's note ("Fast costs no more on this network…") is only known
	// once every speed is priced; its line is held under the summary until
	// then and after, so a fee that lands WITH it moves nothing (item 2 of the
	// 2026-10-11 walk — see `feeModel`).
	return { ...model, freeRoom: inputs.m.speed.free };
}

/**
 * What stands in the simulation verdict's place right now.
 *
 * - `none` — nothing is simulated and nothing is said: a message (it moves
 *   nothing, and a balance card on a signature would answer a question
 *   nobody asked), and a refused request, which says only its refusal.
 * - `out` — a transaction whose simulation has not ended. Also while the
 *   core still holds the confirm for it (`SignView.sim_checking`), even in
 *   the step where the host already has the answer: the verdict and the
 *   confirm that waited for it turn in one frame, never a verdict standing
 *   over a confirm that still says "Checking what this transaction does…"
 *   (Android's `verdictShown`, the same rule).
 * - `landed` — the verdict (`landedVerdict`: the core's records, drawn), or
 *   the core's word that its deadline passed with none
 *   (`SignView.sim_waited_out_key` — the confirm waits four seconds at
 *   most, then opens beside this caution). While that key stands it is what
 *   is said: a verdict that lands later is drawn in the step the core takes
 *   the key back, in the same place.
 *
 * Nothing here shuts or opens the confirm: that is the core's gate, which
 * reads the same view.
 */
type VerdictShown = { stage: 'none' } | { stage: 'out' } | { stage: 'landed'; said: VerdictSaid };

function verdictShown(inputs: SigningLiveInputs): VerdictShown {
	const shown = verdictNow(inputs);
	return shown.stage === 'landed' ? { ...shown, said: withRecheck(shown.said, inputs) } : shown;
}

/**
 * dapp-15: a could-not-check line whose node nobody reached
 * (`SimVerdict.recheck`, the core's) offers "Check again" — and while a new
 * check is out, the line it is about to replace stays, marked stale, rather
 * than reading as the current verdict or vanishing.
 */
function withRecheck(said: VerdictSaid, inputs: SigningLiveInputs): VerdictSaid {
	if (said.kind !== 'notice' || said.block.tone !== 'caution') return said;
	const { m } = inputs;
	if (inputs.simRechecking) {
		return {
			...said,
			block: { ...said.block, recheck: { label: m.simRechecking, checking: true } }
		};
	}
	// Not while the core still holds the confirm or says its wait ran out:
	// the line the person reads is the one the core's step says.
	const { sign } = inputs;
	if (inputs.sim?.verdict?.recheck !== true || sign.sim_checking || sign.sim_waited_out_key) {
		return said;
	}
	return { ...said, block: { ...said.block, recheck: { label: m.simRecheck, checking: false } } };
}

function verdictNow(inputs: SigningLiveInputs): VerdictShown {
	const { sign, sim, m } = inputs;
	const request = sign.request;
	if (!request || sign.blocked) return { stage: 'none' };
	// dapp-15: asked again — the last verdict, stale, until the new one lands.
	if (inputs.simRechecking && sim && (request.kind === 'transaction' || request.kind === 'batch')) {
		return {
			stage: 'landed',
			said: landedVerdict(sim, {
				m,
				nativeSymbol: nativeSymbol(request.chain_id),
				preset: resolvedFormatKeys().number,
				...ownNodeHrefOf(inputs, request.chain_id)
			})
		};
	}
	// Whatever key the core names, what it means is this sentence — and the
	// confirm is open beside it, so the place never goes on saying "Checking…".
	if (sign.sim_waited_out_key) {
		return { stage: 'landed', said: { kind: 'notice', block: couldNotCheck(m) } };
	}
	if (request.kind !== 'transaction' && request.kind !== 'batch') return { stage: 'none' };
	if (sign.sim_checking || !sim) return { stage: 'out' };
	return {
		stage: 'landed',
		said: landedVerdict(sim, {
			m,
			nativeSymbol: nativeSymbol(request.chain_id),
			preset: resolvedFormatKeys().number,
			...ownNodeHrefOf(inputs, request.chain_id)
		})
	};
}

/** R22: the hint's way to this network's RPC settings, when the host offers one. */
function ownNodeHrefOf(inputs: SigningLiveInputs, chainId: number): { ownNodeHref?: string } {
	return inputs.ownNodeHref ? { ownNodeHref: inputs.ownNodeHref(chainId) } : {};
}

/** The verdict's place is itself saying that the transaction is expected to fail. */
function verdictSaysItFails(shown: VerdictShown): boolean {
	return (
		shown.stage === 'landed' && shown.said.kind === 'notice' && shown.said.block.tone === 'danger'
	);
}

/**
 * The simulation's verdict on the sheet — after the request's own blocks,
 * before the footer, where the phones and the desktop put it.
 *
 * **A site's transaction keeps the verdict's PLACE from its first frame**
 * (`kind: 'verdict'`): the balance card saying "Checking…" while the
 * simulation is out, then whatever it says, in a place at least as tall as
 * the verdicts a sheet usually ends on (`verdictRooms`). The sheet is
 * anchored at the bottom, so a card that arrived a moment late pushed the
 * header and its ✕ up by its own height; now it fills a place that was
 * there. A verdict taller than the place is drawn whole and the place grows.
 *
 * **The wallet's own request keeps no place** (the phones' rule): its usual
 * verdict is that nothing moves, and that only confirms what the wallet
 * itself wrote — a technical fact, folded into Technical details
 * (`techModel`, issue 314), not a bordered card weighing as much as the
 * outcome. Anything else the simulation has to say — a call that fails, a
 * node that could not check, a balance that would move — is on the sheet,
 * landing late (`verdict`), so the sheet keeps its confirm where it was and
 * brings it into sight.
 */
function withSimVerdict(
	blocks: Block[],
	shown: VerdictShown,
	inputs: SigningLiveInputs,
	own: boolean
): Block[] {
	if (shown.stage === 'none') return blocks;
	if (own) {
		if (shown.stage === 'out' || shown.said.kind === 'no_change') return blocks;
		return [...blocks, { ...shown.said.block, verdict: true }];
	}
	const inner = shown.stage === 'out' ? checkingCard(inputs.m) : shown.said.block;
	return [
		...blocks,
		{
			kind: 'verdict',
			inner: [inner],
			rooms: verdictRooms(
				inputs.m,
				inputs.sign.request ? inputs.ownNodeOnly?.(inputs.sign.request.chain_id) === true : false
			),
			landed: shown.stage === 'landed'
		}
	];
}

function techModel(inputs: SigningLiveInputs, own: boolean, shown: VerdictShown): TechModel {
	const { sign, clear, m } = inputs;
	// A refused request discloses nothing (spec 081). Android and iOS hide this
	// card entirely under a refusal; web was still putting the raw
	// `params_json` of the very request the wallet would not touch behind a
	// disclosure — the one shell that stayed lax about it.
	const request = sign.blocked ? null : sign.request;
	const result = sign.blocked ? null : clear.result;
	// "No asset changes", when the core's judged view says so and in its line.
	const simResult =
		shown.stage === 'landed' && shown.said.kind === 'no_change' ? shown.said.line : undefined;
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
		raw: request
			? { label: m.techRawData, hex: request.params_json, words: rawWords(m) }
			: undefined,
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
		// A reading from a core (or a record) that predates cs-14 grades
		// nothing: Vela's own, nothing raised.
		reading: result.reading ?? UNGRADED_READING,
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
	progress: (Pick<ApprovalProgress, 'signed' | 'ceremonyUp'> & { onPage?: boolean }) | undefined,
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
	// Round 7 (S4): the person signs on the account's trusted signing page —
	// no prompt is up here, and "Waiting for biometric…" was not true.
	const onPage = progress?.onPage === true && phase === 'awaiting_signature';
	if (!onChain) {
		return {
			stage: 'submitting',
			title: onPage ? m.status.pageSigning : m.status.messageSigning,
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
				title: onPage ? m.status.pageSigning : m.status.signing,
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
 * will revert — in the danger tone, under the intent. It was the one voice
 * that could say so while the web drew nothing of its simulation; it still
 * speaks whenever the simulation does not say the same (the estimate is of
 * the whole operation, its fee leg included, and can fail where the bare
 * calls do not). When the verdict's place is itself saying "expected to
 * fail", the sheet says it once, there — where every client says it.
 * The confirm stays live: a warning informs, it never blocks (L-D5); a submit
 * then meets the relay's refusal, answered as one (RJ3).
 */
function withEstimateVerdict(
	blocks: Block[],
	shown: VerdictShown,
	inputs: SigningLiveInputs
): Block[] {
	const { sign, m, identity } = inputs;
	const request = sign.request;
	if (!request || sign.blocked) return blocks;
	if (request.kind !== 'transaction' && request.kind !== 'batch') return blocks;
	if (verdictSaysItFails(shown)) return blocks;
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
	// R22 (review 1-3): and the sheet's own simulation verdict and the
	// person's "Refuse blind signing" — the core says from them how hard the
	// confirm is (`anyway`, `danger`, `hold_ms`) and refuses every "… anyway"
	// for a person who asked it to (`blind_refused`).
	const confirmState = signConfirmState(
		sign,
		guard,
		clear,
		fee,
		inputs.speed?.view.tier ?? null,
		inputs.messageUnread === true,
		inputs.sim?.verdict ?? null,
		inputs.refuseBlind === true
	);
	const enabled = confirmState?.enabled === true;
	// r18-6 (§31): the core is still reading whether this account owns the
	// Safe the request acts for (`confirm_block: reading`) — the request is
	// not judged yet, and may be refused with no way to sign. The confirm is
	// a shut, plain "Sign" under "Reading the request…": never "… anyway",
	// never the danger tone, whatever the clear-signing view says.
	const ownerReading = sign.confirm_block === 'reading';
	// R22: "… anyway" is the core's (`ConfirmState.anyway` — any check Vela
	// could not read: blind, a leg nobody reads, a simulation not back, not
	// offered, unconfirmed or failing), never a rule of this shell's.
	// While the core still holds the confirm for the sheet's own simulation
	// (`sim_checking`) the request is not judged yet either: the shut confirm
	// stays the plain label — it never flashes "… anyway" in red for the
	// verdict's few hundred ms and turns back, nothing can be pressed then.
	const judging = ownerReading || (sign.sim_checking && !enabled);
	const anyway = confirmState?.anyway === true && !judging;
	const danger = (anyway || confirmState?.danger === true) && !judging;
	const holdMs = judging ? 0 : (confirmState?.hold_ms ?? 0);
	// signer-11: an open confirm after a passkey prompt that signed nothing
	// says so — the confirm is the way to try again.
	const note =
		!enabled && confirmState?.key
			? m.confirmBlock[confirmState.key]
			: enabled && sign.passkey_note
				? m.passkeyNote[sign.passkey_note]
				: undefined;

	const shown = verdictShown(inputs);
	const drawn = withSimVerdict(
		withEstimateVerdict(blocksFor(inputs), shown, inputs),
		shown,
		inputs,
		own
	);
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
					iconUrls: siteIconUrls(request.origin),
					// cs-14: a name the site gave itself is the site's claim.
					...(label.host_line ? { claim: m.certain.siteClaim } : {})
				},
		network: {
			name: chainName(request.chain_id),
			dot: NEUTRAL_TINT,
			logoUrl: chainLogoURL(request.chain_id)
		},
		...(headline ? { headline } : {}),
		...(!own && request.space_note
			? {
					spaceNote: m.spaceMoved.replace('{{network}}', chainName(request.space_note.chain_id))
				}
			: {}),
		blocks,
		...(!sign.blocked && stillReading(clear) ? { reading: 'out' as const } : {}),
		tech: techModel(inputs, own, shown),
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
			action: ownerReading
				? m.signLabel
				: anyway
					? m.certain.signAnyway
					: clear.surface === 'plain_send' && clear.plain_send
						? clear.plain_send.no_value
							? m.confirmPlain
							: m.confirmSend
						: clear.confirm.type === 'confirm_intent'
							? clear.confirm.intent
							: m.confirmPlain,
			enabled,
			// cs-8 / R22: an "… anyway" or a danger — the danger style, held.
			...(danger ? { danger: true } : {}),
			...(holdMs > 0 ? { holdMs, holdLabel: m.holdToConfirm, againLabel: m.confirmAgain } : {}),
			...(note
				? {
						// cs-17: a grant-all waits for "Allow all" or "Revoke", not an amount.
						note:
							confirmState?.key?.endsWith('.approvalChoice') && guard.detected?.is_boolean_grant
								? m.certain.chooseGrant
								: note
					}
				: {})
		},
		closeLabel: m.close,
		revealCue: { below: m.verdictBelow, above: m.verdictAbove },
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
		// §37 (review 1-12): the batch legs kept unlimited on the screen, by
		// index — the core refuses a batch whose kept-unlimited leg is unnamed.
		unlimited_approved_legs: guard.unlimited_consented_legs,
		token_meta: guard.meta,
		// Spec 097: what the reading named — the contract and the coins — kept
		// with the record so Activity names them as this sheet did.
		reading: clear.record_reading ?? null
	};
}
