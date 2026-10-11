/**
 * The third column's transaction detail, live (spec 021 A2 / DA2; wired
 * 2026-09-05). The flow fixtures draw a received USDT and a sent POL; a live
 * page has to show the row that was tapped, worded and formatted the way the
 * home's activity rows already are.
 *
 * Kept beside the wallet's live builders rather than in `flows/live.ts`: it
 * reads the FEED (a wallet resident) and the wallet's own formatters, and the
 * flow overlay only needs to be handed the finished model.
 */
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeedDapp } from '$lib/core/generated/FeedDapp';
import type { FeedDappChange } from '$lib/core/generated/FeedDappChange';
import type { FeedDappContent } from '$lib/core/generated/FeedDappContent';
import type { FeedBatchTransfer } from '$lib/core/generated/FeedBatchTransfer';
import type { FeedDappOperation } from '$lib/core/generated/FeedDappOperation';
import type { FeedFact } from '$lib/core/generated/FeedFact';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedTxStatus } from '$lib/core/generated/FeedTxStatus';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { TrackFailure } from '$lib/core/generated/TrackFailure';
import type { WalletFlowMessages } from '$lib/flows/messages';
import type {
	BreakdownRowModel,
	DesktopFlowModel,
	DesktopFlowStateId,
	FactRowModel,
	FlowScreenModel,
	FlowStateId,
	StatusChipModel,
	TokenMarkModel,
	TxDetailModel,
	TxTechnicalRow
} from '$lib/flows/model';
import { chainMark, tokenMarkFor } from '$lib/flows/marks';
import { maskedAmount } from '$lib/core/client';
import { dappRequestDisplay } from '$lib/core/kernels';
import { formatDate, formatTime } from '$lib/services/locale-format';
import { chainName, explorerTxURL, nativeSymbol } from '$lib/services/networks';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { MASK } from './fixtures';
import { shortenAddress } from './identity';
import { allowanceFigure, changeFigure, dappTitle, dayLabel, moneyText, trimBalance } from './live';
import { fill, type WalletMessages } from './messages';

/** The feed item a tap named, by the id the live rows carry. */
export function findFeedItem(
	feed: FeedView | null | undefined,
	id: string | null
): FeedItem | undefined {
	if (!feed || id === null) return undefined;
	for (const row of feed.rows) {
		if (row.type === 'item' && row.item.id === id) return row.item;
	}
	return undefined;
}

/**
 * The item at a (group, row) position — the history screen's own way of
 * naming a tap. The feed is walked exactly as `liveActivityGroups` groups it:
 * a header opens a group, an item before any header opens an unlabelled one.
 */
export function feedItemAt(
	feed: FeedView | null | undefined,
	group: number,
	row: number
): FeedItem | undefined {
	if (!feed) return undefined;
	let g = -1;
	let r = 0;
	for (const entry of feed.rows) {
		if (entry.type === 'header') {
			g += 1;
			r = 0;
			continue;
		}
		if (g === -1) g = 0;
		if (g === group && r === row) return entry.item;
		r += 1;
	}
	return undefined;
}

/**
 * The history screen's (group × 100 + row) index for a feed item id — the
 * inverse of `feedItemAt`, so a screen that lists a SUBSET of the feed (a
 * token's own rows, spec 038 #E2) can open the same transaction detail the
 * history opens, through the same navigation.
 */
export function feedPositionOf(feed: FeedView | null | undefined, id: string): number | undefined {
	if (!feed) return undefined;
	let g = -1;
	let r = 0;
	for (const entry of feed.rows) {
		if (entry.type === 'header') {
			g += 1;
			r = 0;
			continue;
		}
		if (g === -1) g = 0;
		if (entry.item.id === id) return g * 100 + r;
		r += 1;
	}
	return undefined;
}

export interface TxDetailContext {
	/** The flow corpus (flat), which names the facts. */
	m: WalletFlowMessages;
	/** The wallet corpus, for the day words the home already uses. */
	wm: WalletMessages;
	currency: CurrencyView;
	hidden: boolean;
	identicon: (seed: string) => string;
	now?: number;
	/**
	 * Spec 093: the stored request of a dApp record, by record id, as the
	 * params' JSON text (`storedRequestJson`, `''` for none) — asked only
	 * when its "Technical details" open. Absent: none to show.
	 */
	storedRequest?: (id: string) => string;
}

/**
 * A stored dApp record's request as the record kept it (spec 093): the
 * params' JSON text — the core's cut, which this shell keeps parsed as
 * `signedRequest.params` — or `''` when it kept none (an older row, or a
 * request whose shape alone was past the cut: the core keeps `""` then, stored
 * as no params). Unformatted: what Technical details shows of it is the
 * core's (`dappRequestDisplay`).
 */
export function storedRequestJson(tx: LocalTransaction | undefined): string {
	const params = tx?.signedRequest?.params;
	if (!Array.isArray(params) || params.length === 0) return '';
	try {
		return JSON.stringify(params);
	} catch {
		return '';
	}
}

/**
 * The chip for a lifecycle, in the corpus's words — the same three the desktop
 * draws (`app-desktop/.../flows/live.rs`), so a pending send reads the same on
 * both. `info` and `error` are the tones the design system already gives a
 * waiting and a refused state.
 */
function statusChip(status: FeedTxStatus, m: WalletFlowMessages): StatusChipModel {
	switch (status) {
		case 'pending':
			return { text: m['componentsTx.detail.statusPending'], tone: 'info' };
		case 'failed':
			return { text: m['componentsTx.detail.statusFailed'], tone: 'error' };
		case 'confirmed':
			return { text: m['componentsTx.receipt.statusConfirmed'], tone: 'success' };
		// 087 F04: pending, and nothing will settle it — not failed.
		case 'unknown':
			return { text: m['componentsUi.signing.intentUnknown'], tone: 'info' };
	}
}

/** Feed timestamps are Unix seconds; a millisecond value is tolerated. */
function whenMs(item: FeedItem): number {
	return item.timestamp < 1e12 ? item.timestamp * 1000 : item.timestamp;
}

/**
 * The chain fact, with its mark — a transfer's and a dApp record's alike. It
 * names a NETWORK, so it wears the network's own logo (`chain_mark`), as the
 * confirm page's does: never the coin's, which for ETH sent on Base would
 * put Ethereum's logo beside "Base".
 */
function chainFact(chainId: number, m: WalletFlowMessages): FactRowModel {
	return {
		label: m['componentsTx.detail.labelChain'],
		value: chainName(chainId),
		lead: { kind: 'token', mark: chainMark(chainId) }
	};
}

/**
 * A swept coin's mark. The record named its logos when it was written (none
 * on a phone's) but not its contract, so the core can add candidates only for
 * the chain's own coin; a token it cannot place keeps the logos it named, then
 * its glyph — never a logo guessed from its ticker.
 */
function sweptCoinMark(chainId: number, transfer: FeedBatchTransfer): TokenMarkModel {
	const own = transfer.symbol.toUpperCase() === nativeSymbol(chainId).toUpperCase();
	return tokenMarkFor(chainId, transfer.symbol, own ? null : '', transfer.logo_urls ?? []);
}

/** When it happened, in the home's day words and the person's time preset. */
function dateFact(item: FeedItem, ctx: TxDetailContext): FactRowModel {
	return {
		label: ctx.m['componentsTx.detail.labelDate'],
		value: `${dayLabel(item.day_start_ms, ctx.wm, ctx.now)} ${formatTime(new Date(whenMs(item)))}`
	};
}

/** A contract or a spender: its name when the wallet knows one, else the address. */
function partyFact(
	label: string,
	address: string,
	name: string | null,
	ctx: TxDetailContext
): FactRowModel {
	return {
		label,
		value: name ?? shortenAddress(address),
		lead: { kind: 'identicon', svg: ctx.identicon(address), address },
		mono: name === null,
		copy: ctx.m['componentsUi.identiconViewer.copyAddress'],
		copyValue: address
	};
}

/** A hash, shortened for reading and copied whole. */
function hashFact(label: string, hash: string, m: WalletFlowMessages): FactRowModel {
	return {
		label,
		value: shortenAddress(hash),
		mono: true,
		copy: m['componentsUi.identiconViewer.copyAddress'],
		copyValue: hash
	};
}

/**
 * The fiat beside a figure: only a price the core knows (spec 097 N7,
 * `FeedItem.priced`) — none at all when it knows none, never "≈ $0.00".
 */
function fiatText(item: FeedItem, ctx: TxDetailContext): string {
	if (!item.priced) return '';
	return ctx.hidden ? MASK : `≈ ${moneyText(item.usd_value, ctx.currency)}`;
}

/** Why a dApp operation failed, in the words its request ended with (spec 097 N4). */
function failureText(failure: TrackFailure, m: WalletFlowMessages): string {
	switch (failure) {
		case 'reverted':
			return m['componentsTx.receipt.failedHint'];
		case 'refused':
			return m['componentsUi.signing.refused'];
		case 'not_sent':
			return m['send.txErrorGeneric'];
	}
}

/** One balance change as a line (083 F1): the figure and its coin, or "Unverified token". */
function changeLine(change: FeedDappChange, ctx: TxDetailContext): string {
	if (!change.verified || change.value === null) {
		return `${changeFigure(change)} ${ctx.m['componentsUi.signing.balanceUnverifiedToken']}`;
	}
	return ctx.hidden
		? maskedAmount(change.symbol)
		: `${changeFigure(change)} ${change.symbol}`.trim();
}

/** The detail's facts (spec 093): the core's, in its order, labelled and formatted here. */
function dappFacts(item: FeedItem, dapp: FeedDapp, ctx: TxDetailContext): FactRowModel[] {
	const { m, wm, hidden } = ctx;
	return dapp.facts.flatMap((fact: FeedFact): FactRowModel[] => {
		switch (fact.type) {
			case 'site':
				return [{ label: m['connect.detail.labelApp'], value: fact.site }];
			case 'network':
				return [chainFact(fact.chain_id, m)];
			// A noun for a record of something done (083 F3 review), and who got
			// the money when somebody did — the core sends one or the other.
			case 'contract':
				return [partyFact(m['tokenDetail.labelContract'], fact.address, fact.name, ctx)];
			case 'recipient':
				return [partyFact(m['componentsTx.detail.to'], fact.address, fact.name, ctx)];
			case 'spender':
				return [partyFact(m['componentsUi.signing.labelSpender'], fact.address, fact.name, ctx)];
			case 'spending_cap': {
				const figure = allowanceFigure(fact.allowance, wm);
				return [
					{
						label: m['componentsUi.signingApprove.spendingCap'],
						// The row's own figure is this cap: the core says whether it
						// is money (`figure_maskable` — an unlimited one is not).
						value:
							hidden && item.figure_maskable
								? maskedAmount(figure.unit)
								: `${figure.amount} ${figure.unit}`.trim(),
						...(figure.danger ? { tone: 'danger' as const } : {})
					}
				];
			}
			case 'expires': {
				const at = fact.at === null ? null : fact.at * 1000;
				return [
					{
						label: m['componentsUi.signingApprove.expiresLabel'],
						value:
							at === null
								? m['componentsUi.signingApprove.noExpiry']
								: `${formatDate(at)} ${formatTime(new Date(at))}`
					}
				];
			}
			case 'balance_changes':
				// The sheet's own lines, one row each; the first carries the title.
				return (dapp.changes ?? []).map((change, i) => ({
					label: i === 0 ? m['componentsUi.signing.balanceChangesTitle'] : '',
					value: changeLine(change, ctx),
					...(change.direction === 'in' ? { tone: 'success' as const } : {})
				}));
			case 'date':
				return [dateFact(item, ctx)];
			default:
				// The technical lines belong to the collapsed section, not here.
				return [];
		}
	});
}

function operationText(operation: FeedDappOperation, m: WalletFlowMessages): string {
	switch (operation.type) {
		case 'contract_interaction':
			return m['componentsTx.detail.opContractInteraction'];
		case 'batch':
			return fill(m['componentsUi.signing.batchSubtitle'], { count: operation.calls });
		case 'signature':
			return m['componentsTx.detail.opSignature'];
		case 'typed_data_signature':
			return m['componentsTx.detail.opTypedDataSignature'];
	}
}

function contentLabel(content: FeedDappContent, m: WalletFlowMessages): string {
	switch (content) {
		case 'call_data':
			return m['connect.detail.contentCallData'];
		case 'typed_data':
			return m['connect.detail.contentTypedData'];
		case 'message':
			return m['connect.detail.contentMessage'];
	}
}

/**
 * The collapsed "Technical details" (spec 093): the core's lines, worded. The
 * stored request rides as a reader, not as text — the store is asked for it
 * when the section opens.
 */
function dappTechnical(item: FeedItem, dapp: FeedDapp, ctx: TxDetailContext): TxTechnicalRow[] {
	const { m } = ctx;
	return dapp.technical.flatMap((fact: FeedFact): TxTechnicalRow[] => {
		switch (fact.type) {
			case 'operation':
				return [
					{
						kind: 'fact',
						fact: {
							label: m['componentsTx.detail.labelOperation'],
							value: operationText(fact.operation, m)
						}
					}
				];
			case 'content':
				return [
					{
						kind: 'content',
						label: contentLabel(fact.content, m),
						missing: m['connect.detail.contentMissing'],
						// Read from the store when the section opens, and shown as
						// the core words it — never formatted here.
						read: () => dappRequestDisplay(fact.content, ctx.storedRequest?.(item.id) ?? '')
					}
				];
			case 'primary_type':
				return [
					{
						kind: 'fact',
						fact: { label: m['componentsUi.signing.typeLabel'], value: fact.name, mono: true }
					}
				];
			case 'hash':
				return [
					{ kind: 'fact', fact: hashFact(m['componentsTx.detail.labelHash'], fact.tx_hash, m) }
				];
			case 'user_op_hash':
				return [
					{ kind: 'fact', fact: hashFact(m['componentsTx.receipt.userOpHash'], fact.hash, m) }
				];
			default:
				return [];
		}
	});
}

/**
 * A dApp record as its detail (spec 093): the row's title and figure (or the
 * allowance it granted), its lifecycle — none for a signature, which says it
 * was off-chain instead — the core's facts and, collapsed, its technical
 * lines. Nothing here chooses which facts or in what order.
 */
function dappTxDetail(item: FeedItem, dapp: FeedDapp, ctx: TxDetailContext): TxDetailModel {
	const { m, wm, hidden } = ctx;
	const status = item.status;
	let amount = '';
	let fiat = '';
	let danger = false;
	let back = dapp.received ?? null;
	let positive = false;
	if (item.value !== null) {
		const about = dapp.estimated ? '≈ ' : '';
		const sign = item.direction === 'in' ? '+' : '−';
		// Hidden: the mask, and the coin kept (the core's `maskedAmount`).
		amount = hidden
			? maskedAmount(item.symbol)
			: `${about}${sign}${trimBalance(item.value)} ${item.symbol}`.trim();
		fiat = fiatText(item, ctx);
	} else if (dapp.allowance !== null) {
		const figure = allowanceFigure(dapp.allowance, wm);
		amount =
			hidden && item.figure_maskable
				? maskedAmount(figure.unit)
				: `${figure.amount} ${figure.unit}`.trim();
		danger = figure.danger;
	} else if (back !== null) {
		// Spec 097 N5: nothing left, something came back (a borrow) — what
		// came back IS the figure.
		amount = changeLine(back, ctx);
		positive = back.direction === 'in';
		back = null;
	}
	const failure = dapp.failure ?? null;
	return {
		title: dappTitle(dapp, wm),
		...(dapp.off_chain
			? { note: m['connect.detail.offChainNote'] }
			: { status: statusChip(status, m) }),
		// Spec 097 N4: a failed row says why, under its status.
		...(failure === null ? {} : { note: failureText(failure, m) }),
		closeLabel: m['componentsUi.identiconViewer.close'],
		amount,
		fiat,
		positive,
		...(danger ? { danger: true } : {}),
		...(back === null ? {} : { received: changeLine(back, ctx) }),
		facts: dappFacts(item, dapp, ctx),
		technical: {
			key: item.id,
			title: m['componentsUi.signing.advancedToggle'],
			rows: dappTechnical(item, dapp, ctx)
		},
		viewOnExplorer: m['history.viewOnExplorer'],
		explorerUrl: item.tx_hash === null ? undefined : explorerTxURL(item.chain_id, item.tx_hash),
		deleteLabel: m['history.deleteRecord'],
		deleteQuiet: status === 'pending' || status === 'unknown'
	};
}

/** One feed item as the A2 / DA2 detail. */
export function liveTxDetail(item: FeedItem, ctx: TxDetailContext): TxDetailModel {
	// Spec 093: a dApp's transaction or signature is described by the core.
	if (item.dapp) return dappTxDetail(item, item.dapp, ctx);
	const { m, hidden } = ctx;
	// Spec 082 RG1: the record's lifecycle and what it is are the core's
	// (`FeedItem.status`, `.kind`); a folded batch carries its first line's.
	const status = item.status;
	const received = item.kind === 'receive';
	const dappTx = item.kind === 'dapp_tx';
	const facts: FactRowModel[] = [];

	if (item.counterparty !== null) {
		facts.push({
			// Spec 082 RJ16: the core says who the counterparty IS — the one who
			// got the money, or the contract a dApp's call went to (a swap's
			// router is not its "recipient").
			label: received
				? m['componentsTx.detail.from']
				: item.counterparty_role === 'contract'
					? m['componentsUi.signing.interactingLabel']
					: m['componentsTx.detail.to'],
			value: item.alias ?? shortenAddress(item.counterparty),
			lead: {
				kind: 'identicon',
				svg: ctx.identicon(item.counterparty),
				address: item.counterparty
			},
			mono: item.alias === null,
			copy: m['componentsUi.identiconViewer.copyAddress'],
			copyValue: item.counterparty
		});
	}

	facts.push(chainFact(item.chain_id, m), dateFact(item, ctx));

	if (item.tx_hash !== null) {
		facts.push(hashFact(m['componentsTx.detail.labelHash'], item.tx_hash, m));
	}

	// Spec 038 #D2: a folded batch row opens to what it folded — the split's
	// recipients by name and avatar, the sweep's assets by their marks —
	// under the facts, where the single send's "To" would have been.
	const batch = item.batch;
	// Each part's figure is money on a masked surface (the core's
	// `MoneySurface::TransferDetail`): hidden, it is the mask and its coin.
	// These were drawn in full under a masked hero — who got how much of a
	// split, with the balance hidden — until the unit rule was looked at.
	const partValue = (transfer: { value: string; symbol: string }) =>
		hidden ? maskedAmount(transfer.symbol) : `${trimBalance(transfer.value)} ${transfer.symbol}`;
	const parts: BreakdownRowModel[] =
		batch === null
			? []
			: batch.transfers.map((transfer) =>
					batch.kind === 'split'
						? {
								identiconSvg: ctx.identicon(transfer.to),
								address: transfer.to,
								label: transfer.to_name ?? shortenAddress(transfer.to),
								value: partValue(transfer)
							}
						: {
								lead: sweptCoinMark(item.chain_id, transfer),
								label: transfer.symbol,
								value: partValue(transfer)
							}
				);
	const breakdownTitle =
		batch === null || parts.length === 0
			? undefined
			: batch.kind === 'split'
				? fill(m['send.recipientCount_other'], { count: parts.length })
				: fill(m['send.multiSendSummary'], { n: parts.length, chain: chainName(item.chain_id) });

	// A dApp record from a core that did not describe it has no figure of its
	// own to claim (083 H2). A sweep (several coins to one payee) has no one
	// figure and no one coin either — the core sends neither — so its hero
	// says how many coins left, as the sweep's own confirm did, and the
	// breakdown below names each one. Never a lone "−" over the list.
	const figure =
		item.value !== null
			? `${received ? '+' : '−'}${trimBalance(item.value)} ${item.symbol}`.trim()
			: !dappTx && batch !== null && parts.length > 0
				? fill(m['componentsTx.receipt.assetsCount'], { n: parts.length })
				: '';
	return {
		breakdownTitle,
		breakdown: parts.length > 0 ? parts : undefined,
		// "Sent ETH" names the coin; a record that names none (a sweep) is
		// "Sent", as its receipt was — not "Sent " with nobody after it.
		title: dappTx
			? m['history.txLabelDappTx']
			: item.symbol === '' && !received
				? m['componentsTx.detail.sent']
				: fill(received ? m['history.txLabelReceived'] : m['history.txLabelSent'], {
						symbol: item.symbol
					}),
		// The record's own lifecycle — never the confirmed chip by default
		// (issue 211). The desktop shell has read this since it was wired;
		// this one stamped "Confirmed" on a send that never left the wallet.
		status: statusChip(status, m),
		closeLabel: m['componentsUi.identiconViewer.close'],
		// Hidden: the mask and the coin ("•••• xDAI"). A sweep's hero names no
		// one coin — it counts them — so its mask stands alone.
		amount:
			figure === '' ? '' : hidden ? maskedAmount(item.value !== null ? item.symbol : '') : figure,
		fiat: dappTx && item.value === null ? '' : fiatText(item, ctx),
		positive: received,
		facts,
		viewOnExplorer: m['history.viewOnExplorer'],
		// The transaction hash is the only honest target (spec 082 RJ16: the
		// core never names an op hash as one); without it no control is drawn.
		explorerUrl: item.tx_hash === null ? undefined : explorerTxURL(item.chain_id, item.tx_hash),
		deleteLabel: m['history.deleteRecord'],
		// RJ18: on a pending record the delete is quiet — the record is the
		// trace that stops the same payment being sent twice. One nothing
		// settles (087 F04) may have been sent too.
		deleteQuiet: status === 'pending' || status === 'unknown'
	};
}

/**
 * A transaction screen is about ONE record in the open account's feed, and the
 * state it draws arrives PRERENDERED with a drawn transaction inside it — the
 * mocks' "+120 USDT received, from 0x9F3c…21aE". So the moment the selection
 * stops resolving, the screen must stop being that state (issue #213):
 * switching accounts re-points the feed under an open detail, and the drawn
 * receipt would then stand in for a record the new account never had.
 *
 * The answer is the list the detail was opened from — `a1` / `da1`, the state
 * directly beneath it on the flow stack. It is live-wired, so on an account
 * with no activity it says so instead of inventing a receipt.
 */
export function shownTxDetailStateMobile(
	state: FlowStateId | undefined,
	detail: TxDetailModel | undefined
): FlowStateId | undefined {
	return state === 'a2' && detail === undefined ? 'a1' : state;
}

/** The desktop third column's half of the same rule. */
export function shownTxDetailStateDesktop(
	state: DesktopFlowStateId | undefined,
	detail: TxDetailModel | undefined
): DesktopFlowStateId | undefined {
	return state === 'da2' && detail === undefined ? 'da1' : state;
}

/**
 * The desktop column showing a transaction gets the live one. Without a record
 * the column keeps the state it was handed, which is why the caller picks that
 * state through `shownTxDetailStateDesktop` first — a `tx-detail` body with no
 * record behind it is the drawn fixture, and this must never be the wallet's
 * answer for a real account.
 */
export function withLiveTxDetailDesktop(
	model: DesktopFlowModel,
	detail: TxDetailModel | undefined
): DesktopFlowModel {
	if (detail === undefined || model.body.kind !== 'tx-detail') return model;
	return { ...model, body: { kind: 'tx-detail', model: detail } };
}

/**
 * The phone's transaction sheet, likewise — except that a sheet CAN be taken
 * away, so this one refuses the fixture on its own: no record, no sheet, and
 * the live list it was raised over is what stays.
 */
export function withLiveTxDetailMobile(
	model: FlowScreenModel,
	detail: TxDetailModel | undefined
): FlowScreenModel {
	if (model.sheet?.kind !== 'tx-detail') return model;
	if (detail === undefined) return { ...model, sheet: undefined };
	return { ...model, sheet: { kind: 'tx-detail', model: detail } };
}
