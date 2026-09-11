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
import type { FeeView } from '$lib/core/generated/FeeView';
import type { SendToken } from '$lib/core/generated/SendToken';
import type { SendView } from '$lib/core/generated/SendView';
import { isStable } from '$lib/services/activity';
import { chainName, nativeSymbol } from '$lib/services/networks';
import { chainColor } from '$lib/wallet/fixtures';
import type { WalletIdentity } from '$lib/wallet/identity';
import { shortenAddress } from '$lib/wallet/identity';
import { moneyText, trimBalance } from '$lib/wallet/live';
import { fill } from '$lib/wallet/messages';
import type { WalletFlowMessages } from './messages';
import { chainMark, tokenMarkFor } from './marks';
import type {
	FactRowModel,
	FeeRowModel,
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
	identicon: (seed: string) => string;
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
}

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
 * A settled estimate as one line: the fee coin's amount and its fiat value.
 * `null` while the quote is in flight — the drawn row shows the label alone
 * rather than a number nobody has agreed to yet.
 */
function feeText(fee: FeeEstimateView | null): string {
	if (!fee) return '—';
	const asset = fee.fee_asset;
	if (asset.type === 'erc20') {
		const amount = Number(asset.amount) / 10 ** asset.decimals;
		return `${trimBalance(amount.toString(), 4)} ${asset.symbol ?? ''}`.trim();
	}
	const wei = Number(fee.total_wei) / 1e18;
	const symbol = nativeSymbol(fee.chain_id);
	return `${trimBalance(wei.toString(), 6)} ${symbol}`;
}

function feeRow(inputs: SendLiveInputs, template: FeeRowModel): FeeRowModel {
	const { send, fee, m } = inputs;
	const quote = send.fee ?? fee.fee;
	const chainId = send.selected_token?.chain_id ?? quote?.chain_id ?? 1;
	const symbol =
		quote?.fee_asset.type === 'erc20'
			? (quote.fee_asset.symbol ?? nativeSymbol(chainId))
			: nativeSymbol(chainId);
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
		// the frame where there is nothing to show yet.
		value: quote ? feeText(quote) : send.fee_busy || fee.busy ? '…' : '—',
		openLabel: template.openLabel
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
	if (!inputs.sweepPicking) {
		return {
			...model,
			header: { ...model.header, title: m['send.selectTokenTitle'], pill },
			filters,
			notice: undefined,
			selection: undefined,
			rows,
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
	const amountBlock = {
		value: send.amount || '0',
		fiat: usd === null ? '' : `≈ ${moneyText(usd, currency)}`,
		denomLabel: send.amount_fiat_code ?? token?.symbol ?? ''
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
			cta: m['send.continueBtn']
		};
	}

	return {
		...model,
		mode: split ? 'split' : 'single',
		header: {
			...model.header,
			title: token ? fill(m['send.sendTitle'], { symbol: token.symbol }) : model.header.title
		},
		token: token
			? {
					mark: mark(token),
					symbol: token.symbol,
					detail: `${chainName(token.chain_id)} · ${fill(m['send.balanceLabel'], {
						amount: trimBalance(token.balance)
					})}`,
					max: m['send.maxBtn']
				}
			: model.token,
		// Split mode is the core's: it decides when one recipient becomes many,
		// and the rows below are its drafts, not a list this file keeps.
		addRecipient: split ? undefined : m['send.addRecipient'],
		recipients: split
			? send.recipients.map((draft, index) => ({
					id: draft.id,
					ordinal: fill(m['send.recipientN'], { n: index + 1 }),
					name: draft.name ?? shortenAddress(draft.address),
					address: draft.address,
					identiconSvg: draft.address ? identicon(draft.address) : '',
					amount: `${draft.amount} ${token?.symbol ?? ''}`.trim(),
					amountValue: draft.amount,
					addressLabel: m['send.recipientLabel'],
					pickLabel: m['send.recipientPickAria'],
					removeLabel: m['send.removeRecipient']
				}))
			: undefined,
		recipientActions: split
			? [
					{ id: 'add' as const, label: m['send.addRecipient'] },
					{ id: 'contacts' as const, label: m['send.fromContacts'] },
					{ id: 'import' as const, label: m['send.batchImport'] }
				]
			: undefined,
		// Split shows the total above the fee; single's amount is the hero.
		summary: split
			? {
					label: m['send.splitTotalLabel'],
					value: `${send.token_amount} ${token?.symbol ?? ''}`.trim()
				}
			: undefined,
		amount: split ? undefined : amountBlock,
		recipient: split ? undefined : recipientBlock,
		fee: feeRow(inputs, model.fee),
		cta: m['send.continueBtn']
	};
}

/** The address, split across the drawn two lines. Empty reads as the placeholder. */
function recipientLines(send: SendView): [string, string] {
	const address = send.recipient;
	if (!address) return ['', ''];
	const half = Math.ceil(address.length / 2);
	return [address.slice(0, half), address.slice(half)];
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
			value: feeText(send.fee ?? inputs.fee.fee)
		}
	];

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
			amount: fill(m['componentsTx.receipt.assetsCount'], { n: picked.length }),
			subline: fill(m['send.confirmTotalLine'], {
				fiat: moneyText(totalUsd, currency),
				network: chainName(sweepChain)
			}),
			facts,
			breakdown,
			cta: m['send.confirmSendBtn']
		};
	}

	// SD3b — the split's confirm (spec 038 #D2): how many, and every one of
	// them by name and avatar, so what is about to be signed can be read in
	// full — not "3 recipients" and a total.
	if (send.split_mode && send.recipients.length > 0) {
		const symbol = token?.symbol ?? '';
		const breakdown = send.recipients.map((draft) => ({
			identiconSvg: draft.address ? identicon(draft.address) : undefined,
			address: draft.address || undefined,
			label: draft.name ?? shortenAddress(draft.address),
			value: `${draft.amount} ${symbol}`.trim()
		}));
		const countLine = fill(m['send.recipientCount_other'], { count: send.recipients.length });
		return {
			...model,
			amount: `${send.confirm_amount} ${symbol}`.trim(),
			subline: `${countLine} · ${chainName(chainId)}${usd === null ? '' : ` · ≈ ${moneyText(usd, currency)}`}`,
			facts: facts.filter((fact) => fact.label !== m['send.toLabel']),
			breakdown,
			cta: m['send.confirmSendBtn']
		};
	}

	return {
		...model,
		amount: `${send.confirm_amount} ${token?.symbol ?? ''}`.trim(),
		subline: usd === null ? '' : `≈ ${moneyText(usd, currency)}`,
		facts,
		breakdown: undefined,
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
			fee:
				option.amount === null
					? '—'
					: `~${trimBalance((Number(option.amount) / 10 ** option.decimals).toString(), 4)} ${option.symbol}`,
			selected: option.selected
		}))
	};
}
