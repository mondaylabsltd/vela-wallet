/**
 * The send flow's live overlays (spec 026 T233).
 *
 * Siblings of the fixture builders, exactly as 025's are: the drawn models
 * keep their shape, and every field below is filled from `SendView` /
 * `FeeView` — the core's. Nothing here decides anything. The amounts are the
 * core's strings, the gate is `can_continue` / `can_confirm`, the stage is
 * `stage`, and the words are the corpus's.
 *
 * What this file DOES own is wording and formatting: which template a value
 * goes into, and how a fee reads as "0.0021 ETH · ≈$0.55". That is the same
 * division 025 drew for the home.
 */
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeSpeedView } from '$lib/core/generated/FeeSpeedView';
import type { FeeTier } from '$lib/core/generated/FeeTier';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendAlertKind } from '$lib/core/generated/SendAlertKind';
import type { SendAmountWarning } from '$lib/core/generated/SendAmountWarning';
import type { SendSplitRowIssue } from '$lib/core/generated/SendSplitRowIssue';
import type { SendView } from '$lib/core/generated/SendView';
import { isStable } from '$lib/services/activity';
import { chainName, nativeSymbol } from '$lib/services/networks';
import { chainColor } from '$lib/wallet/fixtures';
import type { WalletIdentity } from '$lib/wallet/identity';
import { shortenAddress } from '$lib/wallet/identity';
import { amountToInput } from '$lib/services/locale-format';
import { fromBaseUnits } from '$lib/services/eip681';
import { exactAmount, moneyText, trimBalance, unitAdornment } from '$lib/wallet/live';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import {
	feeAmountText,
	feeLine,
	feeLineParts,
	feeOptionPriceUsd,
	feeParts,
	feeSymbol
} from './fee-line';
import { chainMark, tokenMarkFor } from './marks';
import { speedControlModel, type OfferedTier, type SpeedWords } from './speed-control';
import type {
	FactRowModel,
	FeeRowModel,
	FeeSpeedModel,
	FlowHeaderModel,
	FeeTokenPickModel,
	SendConfirmModel,
	SendFormModel,
	SendPickModel,
	SendReceiptModel,
	TokenMarkModel,
	BreakdownRowModel
} from './model';
import type { AssetRowModel } from '$lib/wallet/model';

export interface SendLiveInputs {
	send: SendView;
	fee: FeeView;
	m: WalletFlowMessages;
	currency: CurrencyView;
	identity: WalletIdentity;
	/** `name` feeds the initials style; the identicon style ignores it. */
	identicon: (seed: string, name?: string) => string;
	/**
	 * The picker is choosing SEVERAL tokens (spec 028 T440). A shell flag, on
	 * purpose and by precedent: the core's `multi_select_mode` flips only when
	 * the selection is CONFIRMED, and the phone keeps the same pre-confirm
	 * flag as its chain filter (`TokenSelector.tsx: sweepActive`). Which
	 * tokens may be picked, what "all valuable" means and what a sweep moves
	 * are the core's; whether the checkboxes are showing is the screen's.
	 */
	sweepPicking?: boolean;
	/**
	 * The picker's two narrowings (spec 028 Phase 10): the sidebar's network
	 * filter (the phone's pill), and the token-class chips 021 drew and
	 * nothing wired. Shell render state, like the home's `chainFilter`: which
	 * rows are on screen is the screen's; what a tap on a row means stays the
	 * core's. Every index the picker emits is into `visibleSendTokens`.
	 */
	chainFilter?: number | null;
	classFilter?: SendClassFilter;
	/**
	 * The core's last `ShowAlert`, kept by the page until the person edits
	 * or moves on (spec 038 #D4). The phone raised these natively; the web
	 * had a `console.warn` where the sentence should have been.
	 */
	alert?: SendAlertKind | null;
	/**
	 * The speed control (spec 068), as the `fee_speed` core decided it (spec
	 * 069). Absent ⇒ no control is drawn, which is what a surface with no quote
	 * sessions behind it must do rather than offer a choice it cannot honour.
	 *
	 * Every decision is the core's — the tier in force, the free upgrade, the
	 * one-speed statement, which option is measuring, each gas bid as text.
	 * This file only puts words and the fee line on them.
	 */
	speed?: {
		view: FeeSpeedView;
		/**
		 * The fee-coin options of the session pricing `tier` — what an option's
		 * fee is formatted with, exactly as the fee row formats its own: the
		 * symbol and the price come off the same view as the amount, the rule
		 * `fee-line.ts` exists to keep.
		 */
		feeOptions(tier: FeeTier): FeeView['options'];
	};
}

export type { OfferedTier } from './speed-control';

/**
 * What each offered tier is CALLED — the speed itself, never a number. The
 * effective gas price a tier buys is drawn BESIDE the name (issue 684); it is
 * supporting information, and naming a tier by it would answer a question the
 * control's own label did not ask.
 */
const TIER_LABEL_KEY = {
	fast: 'send.gasTier.fast',
	standard: 'send.gasTier.standard',
	slow: 'send.gasTier.slow'
} as const satisfies Record<OfferedTier, keyof WalletFlowMessages>;

/**
 * …and what each one BUYS, the line under the name (spec 068, the owner's
 * ruling). Separate from the name on purpose: the heading asks about speed, so
 * the name has to be a speed and the advantage has to be somewhere else.
 */
const TIER_HINT_KEY = {
	fast: 'send.gasTierHintFast',
	standard: 'send.gasTierHintStandard',
	slow: 'send.gasTierHintSlow'
} as const satisfies Record<OfferedTier, keyof WalletFlowMessages>;

/** SD1's chips: all, the stables, the chains' own coins, the rest. */
export type SendClassFilter = 'all' | 'stable' | 'gas' | 'other';
export const SEND_CLASS_FILTERS: readonly SendClassFilter[] = ['all', 'stable', 'gas', 'other'];

/**
 * Which chip a token answers to. A chain's native coin is what pays its gas;
 * a stable is one by the symbol table `activity_feed.rs` mirrors; everything
 * else is "other". One rule, so the chip and the row can never disagree.
 */
export function sendTokenClass(token: SendToken): Exclude<SendClassFilter, 'all'> {
	if (token.token_address === null) return 'gas';
	if (isStable(token.symbol)) return 'stable';
	return 'other';
}

/** The picker's rows after both narrowings, in the core's order. */
export function visibleSendTokens(
	send: SendView,
	filters: { chainFilter?: number | null; classFilter?: SendClassFilter }
): SendToken[] {
	const chain = filters.chainFilter ?? null;
	const cls = filters.classFilter ?? 'all';
	return send.tokens.filter(
		(token) =>
			(chain === null || token.chain_id === chain) &&
			(cls === 'all' || sendTokenClass(token) === cls)
	);
}

/**
 * The header pill, live: the drawn cluster of dots and "all networks" until
 * a chain is chosen, then that chain's own dot and name. Only screens the
 * drawings gave a pill to get one — the template says which.
 */
export function liveNetworkPill(
	chainFilter: number | null | undefined,
	pillAll: string,
	template: FlowHeaderModel['pill']
): FlowHeaderModel['pill'] {
	if (template === undefined) return undefined;
	if (chainFilter === null || chainFilter === undefined) return { ...template, label: pillAll };
	return { dots: [chainColor(chainFilter)], label: chainName(chainFilter) };
}

/**
 * Ids for recipient rows the SHELL adds (a blank row, a row a contact is
 * picked into). The `s` marks the minter: the core seeds its own rows from a
 * `rcpt_{n}` counter this module cannot see, and two counters sharing one
 * namespace would eventually hand two rows the same id — a duplicate key,
 * and a picker that fills both. Ids are opaque everywhere they are read.
 */
let recipientSeq = 0;
export function makeRecipientId(): string {
	recipientSeq += 1;
	return `rcpt_s${recipientSeq}`;
}

/**
 * The core's `SendToken::id()`, byte for byte — every multi-select event names
 * a token by this string, and a shell id that drifted from the core's would
 * select nothing and say nothing.
 */
export function sendTokenId(token: SendToken): string {
	return `${token.network}_${token.token_address ?? 'native'}_${token.symbol}`;
}

// ---------------------------------------------------------------------------
// Shared wording
// ---------------------------------------------------------------------------

function mark(token: SendToken): TokenMarkModel {
	return tokenMarkFor(token.chain_id, token.symbol, token.token_address, token.logo_urls);
}

/** A token row for the picker: the balance the core carries, priced by it too. */
function tokenRow(token: SendToken, currency: CurrencyView): AssetRowModel {
	const art = mark(token);
	return {
		ticker: token.symbol,
		chain: chainName(token.chain_id),
		badgeColor: chainColor(token.chain_id),
		logoUrls: art.logoUrls,
		badgeLogoUrl: art.badgeLogoUrl,
		badgeHidden: art.badgeHidden,
		balance: trimBalance(token.balance),
		fiat:
			token.price_usd === null
				? { kind: 'no-price', text: '—' }
				: {
						kind: 'value',
						text: moneyText((parseFloat(token.balance) || 0) * token.price_usd, currency)
					},
		masked: false
	};
}

/**
 * The unit price, in USD, of the coin a quote is denominated in.
 *
 * The relay's own published row first — it priced the quote, so its number is
 * the one the estimate itself converted through — then the balances feed the
 * amount's own "≈" line reads. `null` when neither knows the coin: a fee row
 * that invents a price is worse than one that shows only the coin.
 */
function feeUnitPriceUsd(
	contract: string | null,
	chainId: number,
	inputs: SendLiveInputs
): number | null {
	const published = feeOptionPriceUsd(contract, inputs.fee.options);
	if (published !== null) return published;
	const sameCoin = (other: string | null) =>
		contract === null ? other === null : other?.toLowerCase() === contract.toLowerCase();
	const held = [inputs.send.selected_token, ...inputs.send.tokens].find(
		(token) => token !== null && token.chain_id === chainId && sameCoin(token.token_address)
	);
	const price = held?.price_usd ?? null;
	return price !== null && price > 0 ? price : null;
}

/**
 * A settled estimate as one line — "0.0021 ETH · ≈$0.55" (issue 201).
 *
 * The fee was the one figure on the send screens with no money beside it, so a
 * person who does not track the coin's price could not tell what a transfer
 * cost. The words and the threshold are `fee-line.ts`'s, shared with the
 * signing sheet; what this adds is the send form's own second price source.
 */
function feeText(fee: FeeEstimateView | null, inputs: SendLiveInputs): string {
	if (!fee) return '—';
	const parts = feeParts(fee, inputs.fee.options);
	return feeLine(parts, feeUnitPriceUsd(parts.contract, fee.chain_id, inputs), inputs.currency);
}

function feeRow(inputs: SendLiveInputs, template: FeeRowModel): FeeRowModel {
	const { send, fee, m } = inputs;
	const inHand = send.fee ?? fee.fee;
	// NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681).
	//
	// On the one path a pick still re-quotes — a speed tapped before its own
	// preview had settled — the estimate in hand belongs to the tier that was
	// just left. Nothing clears it: `send.fee` survives a tier change (the send
	// core has no event for one), and `fee.fee` holds the old answer until the
	// new one lands. So the row went on showing 超快's money under 较慢's name
	// for a whole round trip, which is the reported defect one size smaller —
	// "我选择的价格和展示的价格不一致了". "…" is the honest thing to say: this
	// speed's figure is being measured. Showing nothing for a second beats
	// relabelling a number.
	const ofAnotherTier =
		inHand !== null && inputs.speed !== undefined && inHand.tier !== inputs.speed.view.tier;
	const quote = ofAnotherTier ? null : inHand;
	// The same words and the same price source as `feeText`, in two pieces
	// (issue 231): the row lets the money drop to a second line whole when it
	// is tight, and never breaks its label to make room.
	const quoteParts = quote ? feeParts(quote, fee.options) : null;
	const line =
		quote && quoteParts
			? feeLineParts(
					quoteParts,
					feeUnitPriceUsd(quoteParts.contract, quote.chain_id, inputs),
					inputs.currency
				)
			: null;
	const chainId = send.selected_token?.chain_id ?? quote?.chain_id ?? 1;
	const symbol = quote ? feeSymbol(quote, fee.options) : nativeSymbol(chainId);
	return {
		// A figure the relay did not quote — a local fallback from defaults —
		// is an ESTIMATE and is labelled as one (spec 038 Part B, finding 14):
		// the core carries the fact as `quoted`; the label is where it shows.
		label: quote && !quote.quoted ? m['send.feeTokenEstimate'] : m['componentsUi.gas.networkFee'],
		mark: tokenMarkFor(
			chainId,
			symbol,
			quote?.fee_asset.type === 'erc20' ? quote.fee_asset.token : null
		),
		// A figure in hand stays on screen while a re-quote is out (spec 028
		// Phase 10): the warm quote lands before the form is complete, and the
		// payee-aware re-ask must not blank the row it just filled. "…" is for
		// the frame where there is nothing to show yet — and for the frame where
		// what is in hand answers about a speed nobody is on any more, which is
		// a measurement waiting to happen rather than an answer of "none".
		value: line ? line.coin : send.fee_busy || fee.busy || ofAnotherTier ? '…' : '—',
		valueFiat: line?.fiat ?? undefined,
		openLabel: template.openLabel,
		refreshLabel: m['send.feeRefresh'],
		// A MEASUREMENT IS OUT — not "somebody tapped ⟳". It is deliberately the
		// generic busy flag, the same fact the "…" above reads from, so the row
		// can never claim to be both settled and measuring: a second tap would
		// only supersede a run already in flight, whoever started it. The cost
		// is that the icon also turns for the form's own re-quotes; that is the
		// truth about the row, and a still icon over a moving number would not be.
		refreshing: send.fee_busy || fee.busy,
		// `FeeView.stale` had NO consumer in this shell (spec 068): the 30s TTL
		// elapsed and nothing on screen said so, while the person spent their
		// drift budget on think-time. Not said while a fresh measurement is out,
		// because "this is old" is about to stop being true. Nor over a row with
		// no figure on it: "from a while ago" is a fact about a number, and the
		// tier just changed under this one (issue 681).
		staleNote:
			fee.stale && !fee.busy && !send.fee_busy && !ofAnotherTier ? m['send.feeStale'] : undefined
	};
}

/**
 * The folded speed control (spec 068), drawn from the `fee_speed` core's view
 * (spec 069) by the builder the signing sheet shares (`speed-control.ts`).
 * Each option's fee goes through THIS screen's fee line, priced as the fee row
 * above it is priced.
 */
function feeSpeed(inputs: SendLiveInputs): FeeSpeedModel | undefined {
	const speed = inputs.speed;
	if (speed === undefined) return undefined;
	const m = inputs.m;
	return speedControlModel(speed.view, speedWords(m), (quote, tier) => {
		const parts = feeParts(quote, speed.feeOptions(tier));
		return feeLineParts(
			parts,
			feeUnitPriceUsd(parts.contract, quote.chain_id, inputs),
			inputs.currency
		);
	});
}

/** The speed control's words, from the send screens' catalog. */
export function speedWords(m: WalletFlowMessages): SpeedWords {
	return {
		label: m['send.feeSpeedLabel'],
		once: m['send.feeSpeedOnce'],
		free: m['send.feeSpeedFree'],
		single: m['send.feeSpeedSingle'],
		gasPriceLabel: m['send.gasPriceLabel'],
		names: {
			fast: m[TIER_LABEL_KEY.fast],
			standard: m[TIER_LABEL_KEY.standard],
			slow: m[TIER_LABEL_KEY.slow]
		},
		hints: {
			fast: m[TIER_HINT_KEY.fast],
			standard: m[TIER_HINT_KEY.standard],
			slow: m[TIER_HINT_KEY.slow]
		}
	};
}

// ---------------------------------------------------------------------------
// The screens
// ---------------------------------------------------------------------------

/**
 * SD1 / SD1b — which token to send, or which several.
 *
 * In sweep mode every fact on the screen is the core's projection: the tick
 * per row is `multi_selected_ids`, the greying is `multi_chain_id` (a batch
 * is one chain, and a row on another chain stays visible but unpickable —
 * the person still owns it), and the CTA counts the same ids the confirm
 * will sweep. The shell narrows nothing and re-decides nothing.
 */
export function liveSendPick(model: SendPickModel, inputs: SendLiveInputs): SendPickModel {
	const { send, currency, m } = inputs;
	const visible = visibleSendTokens(send, inputs);
	const rows = visible.map((token) => tokenRow(token, currency));
	const classFilter = inputs.classFilter ?? 'all';
	const filters = SEND_CLASS_FILTERS.map((id) => ({
		id,
		label:
			id === 'all'
				? m['history.filterAll']
				: id === 'stable'
					? m['send.filterStable']
					: id === 'gas'
						? m['send.filterGas']
						: m['send.filterOther'],
		selected: id === classFilter
	}));
	const pill = liveNetworkPill(
		inputs.chainFilter,
		m['componentsUi.networkFilter.pillAll'],
		model.header.pill
	);
	// An empty list says WHY it is empty (issue 209). The account that holds
	// nothing is where a hand-off from the address book now lands, and a panel
	// with a search box and no rows explains itself to nobody; a filter that
	// hid everything is a different sentence, and the core's own token list is
	// what tells the two apart.
	const empty =
		send.tokens.length === 0 ? m['send.noTokensWithBalance'] : m['send.noMatchingTokens'];
	if (!inputs.sweepPicking) {
		return {
			...model,
			header: { ...model.header, title: m['send.selectTokenTitle'], pill },
			filters,
			notice: undefined,
			selection: undefined,
			rows,
			empty,
			cta: { label: m['send.multiSendTitle'], accent: false }
		};
	}
	const chain = send.multi_chain_id;
	const picked = send.multi_selected_ids;
	const count = picked.length;
	return {
		...model,
		header: { ...model.header, title: m['send.multiSendTitle'], pill },
		filters,
		empty,
		notice:
			chain === null
				? undefined
				: {
						mark: chainMark(chain),
						text: fill(m['send.multiSendChainNotice'], { network: chainName(chain) })
					},
		rows,
		selection: {
			selected: visible.map((token) => picked.includes(sendTokenId(token))),
			dimmed: visible.map((token) => chain !== null && token.chain_id !== chain),
			selectAll: m['send.selectAllValuable']
		},
		cta:
			count > 0
				? {
						label: fill(m['send.multiSendContinue'], {
							n: count,
							chain: chain === null ? '' : chainName(chain)
						}),
						accent: true
					}
				: { label: m['send.multiSendTitle'], accent: false }
	};
}

/** The tokens a sweep will move, in the order the picker lists them. */
function pickedTokens(send: SendView): SendToken[] {
	return send.tokens.filter((token) => send.multi_selected_ids.includes(sendTokenId(token)));
}

/**
 * The amount a sweep moves for one token: the core's reserved spec when it
 * has computed one (net of the gas the fee coin pays), else the full balance
 * the spec will become. Both are the core's numbers and both are HUMAN
 * decimal strings — `MultiTokenSpec.amount` is `full_balance()` less the
 * reserve, exactly as the phone hands it to its batch builder, which is what
 * turns it into base units. The first version of this file converted it a
 * second time and the confirm page priced a 100 USDC sweep at $0.00; the e2e
 * caught it. This only chooses which of the two is on screen at this instant.
 */
function sweepAmount(send: SendView, token: SendToken): string {
	const spec = send.multi_specs.find((row) => row.token_address === token.token_address);
	return spec ? spec.amount : token.balance;
}

/** SD2 — recipient and amount. */
export function liveSendForm(model: SendFormModel, inputs: SendLiveInputs): SendFormModel {
	const { send, m, currency, identicon } = inputs;
	const token = send.selected_token;
	const usd =
		token?.price_usd != null ? (parseFloat(send.token_amount) || 0) * token.price_usd : null;

	const split = send.split_mode;
	// The split's sum in the display currency — the same arithmetic as the
	// single form's line above, on the core's own total.
	// Absent from a core built before the field existed — read as none.
	const issues = split ? (send.split_row_issues ?? []) : [];
	const splitSum = split ? parseFloat(send.confirm_amount) || 0 : 0;
	// Nothing typed yet is not "≈ $0.00" — it is nothing to say.
	const splitUsd = splitSum > 0 && token?.price_usd != null ? splitSum * token.price_usd : null;
	// Which unit the figure is being TYPED in. `amount_fiat_code` is the
	// figure's OWN code, never re-derived from the display context — the core
	// is emphatic about that, and this file only reads it.
	const inFiat = send.amount_fiat_code !== null;
	const amountBlock = {
		// The figure as typed, in the person's decimal mark (issues 204-206); the
		// page hands the core a dot (`amountFromInput`), the same round trip the
		// split rows and the importer's rate make.
		//
		// And the empty field is EMPTY (issue 231): no `|| '0'` fallback. That put a
		// real "0" into the input, so the placeholder never showed and typing 4 made
		// "04" — which the core stored and echoed back. `amountToInput('')` is ''.
		value: amountToInput(send.amount),
		placeholder: '0',
		// The unit, on the figure itself — keyed on the figure's OWN code, like
		// everything else in this block. `currency.code` is the DISPLAY currency
		// and may already have moved on; reading it here would draw "$" over
		// digits typed in yuan.
		adornment: unitAdornment(send.amount_fiat_code, token?.symbol ?? ''),
		// The line under the figure is the OTHER denomination (spec 021
		// component 8; `05-screens-wallet.md:168` — "≈ $12.34" or "0.0042 ETH").
		// It used to be the fiat value in BOTH modes, so a fiat-denominated
		// figure was restated beneath itself and a working swap would have
		// looked like it had done nothing at all.
		fiat: inFiat
			? token === null
				? ''
				: `≈ ${trimBalance(send.token_amount || '0')} ${token.symbol}`
			: usd === null
				? ''
				: `≈ ${moneyText(usd, currency)}`,
		denomLabel: send.amount_fiat_code ?? token?.symbol ?? '',
		// The ⇄ row exists only where the core offers it, and is live only
		// where pressing it would change something. Issue 197: this shell drew
		// the control unconditionally and wired it to nothing — the one event
		// (`toggle_fiat_input`) and all three view fields went unread, so the
		// tap was swallowed whether or not the swap was possible.
		denomToggle: send.denom_toggle_shown ? { enabled: send.denom_toggle_enabled } : undefined
	};
	const recipientBlock = {
		label: m['send.recipientLabel'],
		lines: recipientLines(send),
		address: send.recipient || undefined,
		identiconSvg: send.recipient ? identicon(send.recipient) : '',
		pickLabel: m['send.recipientPickAria'],
		scanLabel: m['send.scanAria'],
		// The trust line the core resolved: a name when it knows one, and the
		// first-interaction note when it does not.
		note: recipientNote(send, m)
	};

	// SD2d — the sweep: several tokens, one recipient, one operation. The
	// rows are the core's picks and the amounts are its reserved specs.
	if (send.multi_select_mode) {
		const picked = pickedTokens(send);
		const chainId = send.multi_chain_id ?? token?.chain_id ?? 1;
		return {
			...model,
			mode: 'sweep',
			header: { ...model.header, title: m['send.multiSendTitle'] },
			token: undefined,
			sweepSummary: fill(m['send.multiSendSummary'], {
				n: picked.length,
				chain: chainName(chainId)
			}),
			sweepRows: picked.map((row) => ({
				mark: mark(row),
				symbol: row.symbol,
				balanceLabel: fill(m['send.balanceLabel'], { amount: trimBalance(row.balance) }),
				amount: trimBalance(sweepAmount(send, row)),
				max: m['send.maxBtn']
			})),
			amount: undefined,
			addRecipient: undefined,
			recipients: undefined,
			recipientActions: undefined,
			summary: undefined,
			recipient: { ...recipientBlock, note: m['send.multiSendSameRecipient'] },
			fee: feeRow(inputs, model.fee),
			speed: feeSpeed(inputs),
			cta: m['send.continueBtn']
		};
	}

	return {
		...model,
		mode: split ? 'split' : 'single',
		// No token, no token card, and no token in the title (issue 209).
		//
		// The drawn SD2 arrives with a card already in it — the mocks' "USDT ·
		// Ethereum · Balance 53.4836" — and the form used to keep that card
		// whenever the core had not named a token. But `selected_token` is
		// null for a REACHABLE reason: the address book hands off a recipient
		// and the core opens the form for them before the token list answers,
		// and on an account that holds nothing it never can. So the fixture
		// WAS the fallback: a wallet showing $0.00 and an empty Assets list
		// opened a Send panel quoting somebody else's balance. The drawn card
		// is a picture of a token; without one there is nothing to draw, which
		// is what the desktop's `send_form` has always done.
		header: {
			...model.header,
			title: token ? fill(m['send.sendTitle'], { symbol: token.symbol }) : m['tokenDetail.send']
		},
		token: token
			? {
					mark: mark(token),
					symbol: token.symbol,
					detail: `${chainName(token.chain_id)} · ${fill(m['send.balanceLabel'], {
						amount: trimBalance(token.balance)
					})}`,
					// `Max` fills the SINGLE amount. In a split there is no single
					// amount — the button wrote a field nobody could see and changed
					// nothing on screen — so a split does not offer it.
					max: split ? undefined : m['send.maxBtn']
				}
			: undefined,
		// Split mode is the core's: it decides when one recipient becomes many,
		// and the rows below are its drafts, not a list this file keeps.
		addRecipient: split ? undefined : m['send.addRecipient'],
		recipients: split
			? send.recipients.map((draft, index) => {
					// A row from the importer or the book knows who it pays. The
					// editable card used to drop that and show a clipped address,
					// so a payroll read as a column of hex; a named row now says
					// both — the name a person recognises, the address that is paid.
					const named = draft.name !== null && draft.name !== '' && draft.address !== '';
					const issue = issues.find((row) => row.id === draft.id);
					return {
						id: draft.id,
						ordinal: fill(m['send.recipientN'], { n: index + 1 }),
						name: named ? (draft.name as string) : shortenAddress(draft.address),
						addressShort: named ? shortenAddress(draft.address) : undefined,
						address: draft.address,
						// Artwork is for an ADDRESS. A half-typed "0x1234" drew somebody's
						// face for nobody; the core says when the field is not one yet.
						identiconSvg:
							draft.address && issue?.address !== 'invalid'
								? identicon(draft.address, draft.name ?? undefined)
								: '',
						amount: `${exactAmount(draft.amount)} ${token?.symbol ?? ''}`.trim(),
						// The field shows the core's figure in the person's decimal
						// mark and hands back a dot (`amountFromInput`, in the page):
						// "1,5" typed raw counted as valid and then summed to nothing.
						amountValue: amountToInput(draft.amount),
						addressLabel: m['send.recipientLabel'],
						addressPlaceholder: m['send.recipientPlaceholder'],
						pickLabel: m['send.recipientPickAria'],
						removeLabel: m['send.removeRecipient'],
						// The core flags a row that repeats an earlier payee and says
						// WHICH row it repeats; this file only picks the template
						// (issue 203). A shell that compared the addresses itself
						// would be a second rule to keep in step with the importer's.
						duplicateNote: duplicateNote(send, draft.id, m),
						// Only a field with something IN it can be wrong; an empty one
						// is unfinished, and its prompt already says what it wants.
						addressNote: issue?.address === 'invalid' ? m['send.batchBadAddress'] : undefined,
						amountNote: issue?.amount === 'invalid' ? m['send.badAmount'] : undefined
					};
				})
			: undefined,
		recipientActions: split
			? [
					{ id: 'add' as const, label: m['send.addRecipient'] },
					{ id: 'contacts' as const, label: m['send.fromContacts'] },
					{ id: 'import' as const, label: m['send.batchImport'] }
				]
			: undefined,
		// Split shows the total above the fee; single's amount is the hero.
		// The total is the core's SUM of the rows (`confirm_amount`) —
		// `token_amount` is the single field, empty in a split, which is why
		// the row read "Total · XDAI" with no figure (spec 038 #D4).
		summary: split
			? {
					// "Total · 3 recipients", as the drawn SD2b has always said and
					// the desktop and Android compose it.
					label: `${m['send.splitTotalLabel']} · ${fill(
						send.recipients.length === 1
							? m['send.recipientCount_one']
							: m['send.recipientCount_other'],
						{ count: send.recipients.length }
					)}`,
					// Empty when a row cannot be summed yet — a dash, not a bare
					// symbol with no figure in front of it.
					value:
						send.confirm_amount === ''
							? '—'
							: `${exactAmount(send.confirm_amount)} ${token?.symbol ?? ''}`.trim(),
					detail: splitUsd === null ? undefined : `≈ ${moneyText(splitUsd, currency)}`,
					over: send.split_over_balance,
					remaining:
						send.split_remaining == null
							? undefined
							: fill(m['send.splitRemaining'], {
									amount: `${trimBalance(send.split_remaining)} ${token?.symbol ?? ''}`.trim()
								})
				}
			: undefined,
		amount: split ? undefined : amountBlock,
		recipient: split ? undefined : recipientBlock,
		fee: feeRow(inputs, model.fee),
		speed: feeSpeed(inputs),
		// The core's live verdict on the figure, and its last refusal. The
		// warning is the one that arrives WITHOUT a tap (issue 211: the send
		// screen said nothing at all about a gas coin the account did not
		// hold; issue 210: `Max` correctly filling 0 because the fee outran
		// the whole balance), so the alert — which only exists after a
		// refused Continue — wins when both are present. Last comes the ⇄
		// control's own refusal (issue 197): a dimmed toggle with no sentence
		// is a refusal nobody can act on, which is exactly what the desktop
		// says here too (`send_notice`).
		//
		// A split has its own live verdict, `split_over_balance` — the same
		// predicate Continue refuses on, which this shell only ever showed AFTER
		// the refusal. And it does not take `amount_warning`: that one is derived
		// from the single form's figure, which a split leaves behind, so it kept
		// judging a number that was no longer on the screen.
		//
		// Ahead of both: the same-asset ceiling (`same_asset_fee_issue`), when
		// the coin being sent also pays the fee. The core measures it against
		// the split's TOTAL too, and it is the more specific sentence — it says
		// the most that can be sent, where "exceeds your balance" does not. The
		// order is the other three shells': ceiling, then the mode's own verdict.
		alert:
			alertWords(inputs.alert, m) ??
			sameFeeWords(send, m) ??
			(split
				? send.split_over_balance
					? m['send.alertInsufficientBalanceBody']
					: undefined
				: (liveWarning(send, m) ?? denomReason(send, m))),
		// In a split the gate closes for two reasons: the pre-check is out
		// (`estimating_gas` — the button turns busy), or a row is unfinished — and
		// the core says WHICH row and which field, so the sentence names it.
		hint: split && !send.estimating_gas ? splitHint(issues, m) : undefined,
		fillEmpty: split ? fillEmpty(send, issues, m, token?.symbol ?? '') : undefined,
		cta: m['send.continueBtn']
	};
}

/** The first unfinished recipient, and what it still needs. */
function splitHint(issues: SendSplitRowIssue[], m: WalletFlowMessages): string | undefined {
	const first = issues[0];
	if (first === undefined) return undefined;
	return fill(first.address === 'ok' ? m['send.splitNeedsAmount'] : m['send.splitNeedsAddress'], {
		n: first.ordinal
	});
}

/**
 * "Use 0.5 ETH for the empty rows": offered while one row has a figure the core
 * accepts and another has none. The figure is the first such row's, exactly as
 * typed — nothing is computed.
 */
function fillEmpty(
	send: SendView,
	issues: SendSplitRowIssue[],
	m: WalletFlowMessages,
	symbol: string
): { label: string; amount: string } | undefined {
	const empty = issues.some((row) => row.amount === 'empty');
	if (!empty) return undefined;
	const flagged = new Map(issues.map((row) => [row.id, row.amount]));
	const source = send.recipients.find(
		(row) => row.amount.trim() !== '' && (flagged.get(row.id) ?? 'ok') === 'ok'
	);
	if (source === undefined) return undefined;
	return {
		amount: source.amount,
		label: fill(m['send.splitFillEmpty'], {
			amount: `${exactAmount(source.amount)} ${symbol}`.trim()
		})
	};
}

/**
 * Why ⇄ is inert, when it is inert (`denom_toggle_reason`, `Some` exactly
 * when the row is shown and disabled). The core decides THAT there is no rate
 * to enter this currency against; the corpus says it.
 */
function denomReason(send: SendView, m: WalletFlowMessages): string | undefined {
	const issue = send.denom_toggle_reason;
	return issue === null
		? undefined
		: fill(m['send.denomToggleNoRate'], { code: issue.code, symbol: issue.symbol });
}

/**
 * The same-asset ceiling, worded: what the transfer and its fee need together,
 * what there is, and the most that can be sent. Every figure arrives in base
 * units; the shell only formats them (the phones' `formWarning`).
 */
function sameFeeWords(send: SendView, m: WalletFlowMessages): string | undefined {
	const issue = send.same_asset_fee_issue;
	if (issue == null) return undefined;
	const decimals = send.selected_token?.decimals ?? 18;
	const human = (base: string) => exactAmount(fromBaseUnits(BigInt(base), decimals));
	const body = fill(m['send.sameFeeTokenBody'], {
		amount: human(issue.transfer_amount),
		fee: human(issue.fee_amount),
		total: human(issue.total),
		symbol: issue.symbol,
		balance: human(issue.balance)
	});
	const most = fill(m['send.sameFeeTokenMax'], {
		amount: human(issue.max_transfer_amount),
		symbol: issue.symbol
	});
	return `${body} ${most}`;
}

/**
 * The core's live amount verdict (`amount_warning`), worded. Android draws
 * this under the form as `formWarning`; on web the field was never read, so a
 * person typing an amount their gas coin cannot pay for saw nothing until they
 * pressed Continue — and before the core measured a native fee at all, not
 * even then.
 */
function liveWarning(send: SendView, m: WalletFlowMessages): string | undefined {
	return send.amount_warning === null
		? undefined
		: warningWords(send.amount_warning, m, send.selected_token?.chain_id);
}

/**
 * One sentence per `SendAmountWarning`, the same mapping Android's
 * `SendLive.warningText` and the desktop's `warn_*` strings draw. The core
 * decides THAT the money does not add up and which shape the refusal has; the
 * corpus says it.
 */
export function warningWords(
	warning: SendAmountWarning,
	m: WalletFlowMessages,
	/**
	 * The network the figure belongs to. A `null` symbol is the core saying
	 * "the chain's own coin, which your registry knows and mine may not" — so
	 * a caller that has the chain resolves it here rather than printing a
	 * sentence with a hole in it. Callers without one keep the phones' `""`.
	 */
	chainId?: number
): string | undefined {
	const gasCoin = (symbol: string | null) =>
		symbol ?? (chainId === undefined ? '' : nativeSymbol(chainId));
	switch (warning.type) {
		case 'not_enough_token':
			return fill(m['send.warnNotEnoughToken'], { symbol: warning.symbol });
		case 'insufficient_for_gas':
			return fill(m['send.warnInsufficientForGas'], { sym: gasCoin(warning.symbol) });
		// The fee alone outruns the whole balance of the coin that pays it —
		// the state `Max` fills `0` for (issue 210).
		case 'insufficient_gas':
			return fill(m['send.warnInsufficientGas'], { sym: gasCoin(warning.symbol) });
		case 'need_gas':
			return fill(m['send.warnNeedGas'], { sym: gasCoin(warning.symbol) });
		case 'cannot_convert':
			return fill(m['send.warnCannotConvert'], { code: warning.code, symbol: warning.symbol });
	}
}

/**
 * The core's alert kind, in the corpus's words — semantic keys only, the same
 * mapping the desktop draws (`send_alert_words`). Title and body joined by a
 * middle dot where both exist; a kind with one sentence gets that sentence.
 */
export function alertWords(
	kind: SendAlertKind | null | undefined,
	m: WalletFlowMessages
): string | undefined {
	if (!kind) return undefined;
	switch (kind.type) {
		case 'invalid_address':
			return `${m['send.alertInvalidAddressTitle']} · ${m['send.alertInvalidAddressBody']}`;
		case 'invalid_amount':
			return `${m['send.alertInvalidAmountTitle']} · ${m['send.alertInvalidAmountBody']}`;
		case 'insufficient_balance':
			// The refusal the core carried and this shell used to drop (issue
			// 211): "The total exceeds your balance" is a lie about a send
			// whose token balance is fine and whose GAS coin is empty. Android
			// and iOS have always read the warning out; web said the generic
			// sentence and left the person to guess which balance.
			return `${m['send.alertInsufficientBalanceTitle']} · ${
				(kind.warning && warningWords(kind.warning, m)) ?? m['send.alertInsufficientBalanceBody']
			}`;
		case 'split_over_balance':
			return `${m['send.alertInsufficientBalanceTitle']} · ${m['send.alertInsufficientBalanceBody']}`;
		case 'load_tokens_failed':
			return m['send.alertLoadTokensError'];
		case 'estimate_failed':
			return `${m['send.alertEstimateFailedTitle']} · ${m['send.alertEstimateFailedBody']}`;
		case 'account_unavailable':
			return m['send.alertAccountUnavailableBody'];
	}
}

/** The address, split across the drawn two lines. Empty reads as the placeholder. */
function recipientLines(send: SendView): [string, string] {
	const address = send.recipient;
	if (!address) return ['', ''];
	const half = Math.ceil(address.length / 2);
	return [address.slice(0, half), address.slice(half)];
}

/**
 * "Same address as recipient 2", for a split row the core has flagged as a
 * repeat of an earlier one (issue 203). `undefined` for every other row —
 * including the FIRST occurrence, which is not the mistake.
 */
function duplicateNote(send: SendView, id: string, m: WalletFlowMessages): string | undefined {
	const repeat = send.split_duplicates.find((row) => row.id === id);
	return repeat ? fill(m['send.recipientDuplicate'], { n: repeat.first_ordinal }) : undefined;
}

function recipientNote(send: SendView, m: WalletFlowMessages): string | undefined {
	const identity = send.recipient_identity;
	if (identity?.name) {
		return identity.source ? `${identity.name} · ${identity.source}` : identity.name;
	}
	// The core's own verdict, in the corpus's words. A contract recipient has
	// no send-screen sentence written for it yet (recorded); the first-time
	// tell does, and it is the one that matters for a poisoned look-alike.
	if (send.recipient_risk?.first_time === true) return m['componentsUi.signing.firstTimeTag'];
	return undefined;
}

/** SD3 — the confirm screen: what is about to be signed. */
export function liveSendConfirm(model: SendConfirmModel, inputs: SendLiveInputs): SendConfirmModel {
	const { send, m, currency, identity, identicon } = inputs;
	const token = send.selected_token;
	const usd =
		token?.price_usd != null ? (parseFloat(send.confirm_amount) || 0) * token.price_usd : null;
	const chainId = token?.chain_id ?? 1;
	// The core's own verdict, resolved on this page only (`confirm_probes`) —
	// the form's note never sees it. One recipient only: a split's rows have
	// no per-row verdict.
	const recipientTag =
		!send.split_mode && send.recipient_risk?.first_time === true
			? m['componentsUi.signing.firstTimeTag']
			: undefined;

	// A sweep moves several coins; one mark would name the wrong one.
	const heroMark =
		send.multi_select_mode || token == null
			? undefined
			: tokenMarkFor(
					token.chain_id,
					token.symbol,
					token.token_address,
					token.logo_urls ?? undefined
				);

	const facts: FactRowModel[] = [
		{
			label: m['send.fromLabel'],
			value: identity.name,
			lead: { kind: 'identicon', svg: identicon(identity.address), address: identity.address }
		},
		{
			label: m['send.toLabel'],
			value: send.recipient_identity?.name ?? shortenAddress(send.recipient),
			lead: { kind: 'identicon', svg: identicon(send.recipient), address: send.recipient },
			mono: send.recipient_identity?.name == null
		},
		{
			label: m['componentsTx.detail.labelChain'],
			value: chainName(chainId),
			lead: { kind: 'token', mark: chainMark(chainId) }
		},
		{
			label:
				(send.fee ?? inputs.fee.fee)?.quoted === false
					? m['send.feeTokenEstimate']
					: m['send.estFeeLabel'],
			value: feeText(send.fee ?? inputs.fee.fee, inputs)
		}
	];

	// The speed, but only when it was CHOSEN for this send (spec 068). The
	// confirm is the last screen before a signature, and a payment deliberately
	// bumped off the usual pace should say so there rather than only on the
	// form two taps back — that is also where a mis-tap gets caught. A send at
	// the stored default adds no row: most sends need no decision about speed,
	// and a permanent line would ask everybody to make one.
	// A free upgrade (issue 686) is also a send running off the person's usual
	// pace, so the last screen before the signature names it the same way.
	const speed = inputs.speed?.view;
	if (speed?.picked || speed?.free) {
		facts.push({
			label: m['send.feeSpeedLabel'],
			value: m[TIER_LABEL_KEY[speed.tier === 'rapid' ? 'fast' : speed.tier]],
			// …and a free upgrade says WHY, here too (issue 686 rule 5): the
			// person's Settings name a slower speed, and this is the screen they
			// check before signing. The reason travels with the tier wherever
			// the tier is shown. A pick needs no reason — they made it.
			note: speed.picked ? undefined : m['send.feeSpeedFree']
		});
	}

	// SD3c — the sweep's confirm: N assets, one network, one operation. Each
	// breakdown row is a reserved spec, i.e. the exact amount the signature
	// will move (invariant ⑪) — never a number this file summed on its own.
	if (send.multi_select_mode) {
		const picked = pickedTokens(send);
		const sweepChain = send.multi_chain_id ?? chainId;
		let totalUsd = 0;
		const breakdown = picked.map((row) => {
			const amount = sweepAmount(send, row);
			const rowUsd = row.price_usd === null ? null : (parseFloat(amount) || 0) * row.price_usd;
			if (rowUsd !== null) totalUsd += rowUsd;
			const value = `${trimBalance(amount)} ${row.symbol}`;
			return {
				lead: mark(row),
				label: row.symbol,
				value: rowUsd === null ? value : `${value} · ≈${moneyText(rowUsd, currency)}`
			};
		});
		return {
			...model,
			mark: heroMark,
			amount: fill(m['componentsTx.receipt.assetsCount'], { n: picked.length }),
			subline: fill(m['send.confirmTotalLine'], {
				fiat: moneyText(totalUsd, currency),
				network: chainName(sweepChain)
			}),
			facts,
			breakdown,
			recipientTag,
			alert: alertWords(inputs.alert, m),
			cta: m['send.confirmSendBtn']
		};
	}

	// SD3b — the split's confirm (spec 038 #D2): how many, and every one of
	// them by name and avatar, so what is about to be signed can be read in
	// full — not "3 recipients" and a total.
	if (send.split_mode && send.recipients.length > 0) {
		const symbol = token?.symbol ?? '';
		const breakdown = send.recipients.map((draft) => ({
			identiconSvg: draft.address ? identicon(draft.address, draft.name ?? undefined) : undefined,
			address: draft.address || undefined,
			label: draft.name ?? shortenAddress(draft.address),
			// A name never stands in for the address on the page that signs.
			detail: draft.name ? shortenAddress(draft.address) : undefined,
			mono: !draft.name,
			value: `${exactAmount(draft.amount)} ${symbol}`.trim(),
			// The form's repeat warning, said again on the page that signs
			// (issue 203): two lines paying one payee are hardest to spot
			// exactly where the avatars are identical and the sum looks right.
			note: duplicateNote(send, draft.id, m)
		}));
		const countLine = fill(
			send.recipients.length === 1 ? m['send.recipientCount_one'] : m['send.recipientCount_other'],
			{ count: send.recipients.length }
		);
		return {
			...model,
			mark: heroMark,
			amount: `${exactAmount(send.confirm_amount)} ${symbol}`.trim(),
			subline: `${countLine} · ${chainName(chainId)}${usd === null ? '' : ` · ≈ ${moneyText(usd, currency)}`}`,
			facts: facts.filter((fact) => fact.label !== m['send.toLabel']),
			breakdown,
			alert: alertWords(inputs.alert, m),
			cta: m['send.confirmSendBtn']
		};
	}

	return {
		...model,
		mark: heroMark,
		amount: `${send.confirm_amount} ${token?.symbol ?? ''}`.trim(),
		subline: usd === null ? '' : `≈ ${moneyText(usd, currency)}`,
		facts,
		breakdown: undefined,
		recipientTag,
		alert: alertWords(inputs.alert, m),
		cta: m['send.confirmSendBtn']
	};
}

/**
 * SD4 — the receipt.
 *
 * The stage is the CORE's, and it comes from `receipt.status`, not from
 * `tx_status`: the core flips `tx_status` to `confirmed` the moment the
 * signature is a fact (that is what "the send screen is done" means to it),
 * while the receipt's own status is what tracks the chain — `submitted` until
 * a hash lands, `confirmed` when one does, `failed` only on a definitive
 * verdict. Reading the wrong one would tell a person their money had arrived
 * while it was still in the air.
 */
export function liveSendReceipt(model: SendReceiptModel, inputs: SendLiveInputs): SendReceiptModel {
	const { send, m, identicon } = inputs;
	const token = send.selected_token;
	const chainId = token?.chain_id ?? 1;
	const parts = receiptParts(send, m, token?.symbol ?? '', identicon);
	const header = {
		...model.header,
		title: token ? fill(m['send.sendTitle'], { symbol: token.symbol }) : model.header.title
	};
	const to = send.recipient_identity?.name ?? shortenAddress(send.recipient);
	const status = send.receipt?.status;

	if (status === 'failed' || send.tx_status === 'error') {
		return {
			...model,
			header,
			stage: 'failed',
			// The core chose the key; the shell only reads it out of the corpus.
			title: m['send.txErrorGeneric'],
			captions: [],
			hash: undefined,
			cta: m['componentsTx.receipt.done'],
			ctaAccent: false
		};
	}

	if (status === 'confirmed') {
		return {
			...model,
			header,
			...parts,
			stage: 'confirmed',
			title: fill(m['send.txConfirmedTitle'], {
				amount: send.receipt?.amount ?? send.confirm_amount,
				symbol: token?.symbol ?? ''
			}),
			// A split names its count here and its people below; "To " with
			// nobody after it was what the single-recipient line read as.
			captions: [
				`${parts.breakdownTitle ?? fill(m['history.toName'], { name: to })} · ${chainName(chainId)}`
			],
			hash: send.tx_hash
				? {
						label: m['componentsTx.receipt.txHash'],
						value: send.tx_hash,
						copyLabel: m['componentsUi.identiconViewer.copyAddress']
					}
				: undefined,
			cta: m['componentsTx.receipt.done'],
			ctaAccent: true
		};
	}

	if (status === 'submitted') {
		const receipt = send.receipt;
		const eta =
			receipt?.submitted_at_ms != null && receipt.typical_inclusion_s != null
				? {
						submittedAtMs: receipt.submitted_at_ms,
						typicalS: receipt.typical_inclusion_s,
						typicalLine: fill(m['send.txTypicalTime'], {
							chainName: chainName(chainId),
							estSecs: receipt.typical_inclusion_s
						}),
						remainingTemplate: m['send.txRemaining'],
						elapsedTemplate: m['send.txElapsed'],
						slowLine: m['send.txSlowConfirm']
					}
				: undefined;
		return {
			...model,
			header,
			...parts,
			stage: 'submitted',
			title: m['send.txSubmittedTitle'],
			captions: [m['send.txWaitingConfirm']],
			eta,
			hash: send.user_op_hash
				? {
						label: m['componentsTx.receipt.txHash'],
						value: send.tx_hash ?? send.user_op_hash,
						copyLabel: m['componentsUi.identiconViewer.copyAddress']
					}
				: undefined,
			cta: m['send.txCloseBackground'],
			ctaAccent: false
		};
	}

	// Signing or submitting: nothing has been accepted yet.
	return {
		...model,
		header,
		...parts,
		stage: 'submitting',
		title: m['send.txSubmitting'],
		captions: [m['send.txPreparingBiometric'], m['send.txBackgroundHint']],
		hash: undefined,
		cta: m['send.txCloseBackground'],
		ctaAccent: false
	};
}

/**
 * Spec 038 #D2: a split's parts on the receipt as on the confirm — from the
 * receipt's own transfers once the core froze them, from the drafts before
 * that. Nothing for a single send or a sweep (the sweep's parts are assets,
 * and its one recipient is already the caption).
 */
function receiptParts(
	send: SendView,
	m: WalletFlowMessages,
	symbol: string,
	identicon: (seed: string) => string
): { breakdownTitle?: string; breakdown?: BreakdownRowModel[] } {
	const frozen = send.receipt?.kind === 'split' ? send.receipt.transfers : [];
	const rows: BreakdownRowModel[] =
		frozen.length > 0
			? frozen.map((transfer) => ({
					identiconSvg: identicon(transfer.to),
					address: transfer.to,
					label: transfer.to_name ?? shortenAddress(transfer.to),
					value: `${transfer.amount} ${transfer.symbol}`.trim()
				}))
			: send.split_mode
				? send.recipients.map((draft) => ({
						identiconSvg: draft.address ? identicon(draft.address) : undefined,
						address: draft.address || undefined,
						label: draft.name ?? shortenAddress(draft.address),
						value: `${draft.amount} ${symbol}`.trim()
					}))
				: [];
	if (rows.length === 0) return { breakdownTitle: undefined, breakdown: undefined };
	return {
		breakdownTitle: fill(m['send.recipientCount_other'], { count: rows.length }),
		breakdown: rows
	};
}

/**
 * SD2f — the fee coin sheet.
 *
 * Every row the relay published, including the ones that cannot pay: which is
 * spendable is `insufficient`, the core's verdict, and hiding a row here would
 * be a second filter beside the core's own.
 */
export function liveFeeTokenPick(
	model: FeeTokenPickModel,
	inputs: SendLiveInputs
): FeeTokenPickModel {
	const { fee, send, m } = inputs;
	const chainId = send.selected_token?.chain_id ?? 1;
	return {
		...model,
		rows: fee.options.map((option) => ({
			mark: tokenMarkFor(chainId, option.symbol, option.contract),
			symbol: option.symbol,
			balanceLabel: fill(m['send.balanceLabel'], {
				amount: trimBalance((Number(option.balance) / 10 ** option.decimals).toString(), 4)
			}),
			// The SAME formatter and the same decimal budget as the fee row this
			// sheet opens from (issue 682): four decimals of its own hand-rolled
			// trim printed an 0.000083 OKB fee as "~0 OKB" under the word
			// "estimated", one tap below a row reading "0.000083 OKB · ≈ $0.01".
			fee:
				option.amount === null
					? '—'
					: `~${feeAmountText(Number(option.amount) / 10 ** option.decimals, option.contract === null ? 6 : 4)} ${option.symbol}`,
			selected: option.selected,
			// The verdict this file's own comment promised and never passed on
			// (issue 211): the native row is always offered, balance or not,
			// and the core refuses to select one that cannot pay. A row that
			// looks exactly like the others and silently does nothing is how a
			// person ends up paying gas in a coin they do not hold.
			insufficient: option.insufficient,
			insufficientNote: fill(m['send.warnInsufficientGas'], { sym: option.symbol })
		}))
	};
}
