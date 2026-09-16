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
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedTxStatus } from '$lib/core/generated/FeedTxStatus';
import type { FeedView } from '$lib/core/generated/FeedView';
import type { WalletFlowMessages } from '$lib/flows/messages';
import type {
	BreakdownRowModel,
	DesktopFlowModel,
	DesktopFlowStateId,
	FactRowModel,
	FlowScreenModel,
	FlowStateId,
	StatusChipModel,
	TxDetailModel
} from '$lib/flows/model';
import { chainMeta } from '$lib/services/chains';
import { formatTime } from '$lib/services/locale-format';
import { chainName, explorerTxURL } from '$lib/services/networks';
import { chainColor, MASK } from './fixtures';
import { shortenAddress } from './identity';
import { dayLabel, moneyText, trimBalance } from './live';
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
 * What actually happened to the record behind a feed row (issue 211).
 *
 * The feed is NOT settled history: a send is written the moment it is
 * submitted and stays `pending` until the tracker resolves it, and a definite
 * refusal or revert makes it `failed`. This screen used to stamp every row
 * `Confirmed`, so a send that never landed — one paying its gas in a coin the
 * account did not hold — was reported as money that had moved.
 *
 * A folded batch row carries the group's own status (its `id` is the shared
 * `user_op_hash`, which matches no record id); every other row is one record.
 * An id that resolves to nothing keeps the settled reading: rows that are not
 * local records at all (an incoming transfer the chain already carries) have
 * no lifecycle to report.
 */
export function feedItemStatus(
	feed: FeedView | null | undefined,
	item: FeedItem | undefined
): FeedTxStatus {
	if (item === undefined) return 'confirmed';
	if (item.batch !== null) return item.batch.status;
	const record = feed?.transactions.find((tx) => tx.id === item.id);
	return record?.status ?? 'confirmed';
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
	/**
	 * The record's lifecycle, from `feedItemStatus`. Required, not defaulted:
	 * the chip that lies is the one nobody had to think about (issue 211).
	 */
	status: FeedTxStatus;
	identicon: (seed: string) => string;
	now?: number;
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
	}
}

/** Feed timestamps are Unix seconds; a millisecond value is tolerated. */
function whenMs(item: FeedItem): number {
	return item.timestamp < 1e12 ? item.timestamp * 1000 : item.timestamp;
}

/** One feed item as the A2 / DA2 detail. */
export function liveTxDetail(item: FeedItem, ctx: TxDetailContext): TxDetailModel {
	const { m, wm, currency, hidden, status } = ctx;
	const received = item.direction === 'in';
	const chain = chainMeta(item.chain_id);
	const facts: FactRowModel[] = [];

	if (item.counterparty !== null) {
		facts.push({
			label: received ? m['componentsTx.detail.from'] : m['componentsTx.detail.to'],
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

	facts.push(
		{
			label: m['componentsTx.detail.labelChain'],
			value: chainName(item.chain_id),
			lead: {
				kind: 'token',
				mark: {
					ticker: chain?.iconLabel ?? chainName(item.chain_id).slice(0, 3).toUpperCase(),
					badgeColor: chainColor(item.chain_id)
				}
			}
		},
		{
			label: m['componentsTx.detail.labelDate'],
			value: `${dayLabel(item.day_start_ms, wm, ctx.now)} ${formatTime(new Date(whenMs(item)))}`
		}
	);

	if (item.tx_hash !== null) {
		facts.push({
			label: m['componentsTx.detail.labelHash'],
			value: shortenAddress(item.tx_hash),
			mono: true,
			copy: m['componentsUi.identiconViewer.copyAddress'],
			copyValue: item.tx_hash
		});
	}

	// Spec 038 #D2: a folded batch row opens to what it folded — the split's
	// recipients by name and avatar, the sweep's assets by their marks —
	// under the facts, where the single send's "To" would have been.
	const batch = item.batch;
	const parts: BreakdownRowModel[] =
		batch === null
			? []
			: batch.transfers.map((transfer) =>
					batch.kind === 'split'
						? {
								identiconSvg: ctx.identicon(transfer.to),
								address: transfer.to,
								label: transfer.to_name ?? shortenAddress(transfer.to),
								value: `${trimBalance(transfer.value)} ${transfer.symbol}`
							}
						: {
								lead: {
									ticker: transfer.symbol,
									badgeColor: chainColor(item.chain_id),
									logoUrls: transfer.logo_urls ?? undefined,
									badgeHidden: true
								},
								label: transfer.symbol,
								value: `${trimBalance(transfer.value)} ${transfer.symbol}`
							}
				);
	const breakdownTitle =
		batch === null || parts.length === 0
			? undefined
			: batch.kind === 'split'
				? fill(m['send.recipientCount_other'], { count: parts.length })
				: fill(m['send.multiSendSummary'], { n: parts.length, chain: chainName(item.chain_id) });

	const amount = item.value === null ? '' : `${trimBalance(item.value)} `;
	return {
		breakdownTitle,
		breakdown: parts.length > 0 ? parts : undefined,
		title: fill(received ? m['history.txLabelReceived'] : m['history.txLabelSent'], {
			symbol: item.symbol
		}),
		// The record's own lifecycle — never the confirmed chip by default
		// (issue 211). The desktop shell has read this since it was wired;
		// this one stamped "Confirmed" on a send that never left the wallet.
		status: statusChip(status, m),
		closeLabel: m['componentsUi.identiconViewer.close'],
		amount: hidden ? MASK : `${received ? '+' : '−'}${amount}${item.symbol}`,
		fiat: hidden ? MASK : `≈ ${moneyText(item.usd_value, currency)}`,
		positive: received,
		facts,
		viewOnExplorer: m['history.viewOnExplorer'],
		// The hash is the only honest target; a record without one (a pending
		// send the tracker has not yet resolved) draws the control inert.
		explorerUrl: item.tx_hash === null ? undefined : explorerTxURL(item.chain_id, item.tx_hash),
		deleteLabel: m['history.deleteRecord']
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
