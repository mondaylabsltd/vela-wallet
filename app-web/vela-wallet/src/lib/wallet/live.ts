/**
 * The live wallet-home builders (spec 025 T123, research D10): the core's
 * `BalanceView` + the committed display-currency pair → the display models
 * the drawn components already consume. Siblings of the fixture builders,
 * applied as overlays over an identity-filled base (the settings precedent).
 *
 * NO arithmetic decides anything here. The total is the core's
 * `display_total_usd` (already the aggregation, already withheld while
 * hidden — invariant ⑧); the per-row fiat is the same balance × price the
 * core sorts by. What this file adds is presentation: currency conversion at
 * the committed rate (or the honest USD figure when there is no rate — 024's
 * rule), grouping digits, trimming a balance's tail, choosing which drawn
 * state a view maps to.
 *
 * Activity rows are the core's (grouped, folded, tombstoned); the shell words
 * the day headers and formats the amounts.
 */

import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { formatDate, groupDigits, numberSeparators } from '$lib/services/locale-format';
import { chainName, explorerAddressURL, explorerBaseURL } from '$lib/services/networks';
import {
	balanceTokenBadgeChainId,
	balanceTokenLogoURLs,
	chainLogoURL
} from '$lib/services/tokens-model';
import { shortenAddress } from './identity';
import { fill } from './messages';
import { currencyGlyph, currencySymbol } from '$lib/settings/fixtures';
import { BALANCE_MASK, chainColor, MASK } from './fixtures';
import type { WalletMessages } from './messages';
import type {
	ActivityGroupModel,
	ActivityRowModel,
	AssetDetailPanelModel,
	AssetRowModel,
	BalanceModel,
	ChainRowModel,
	SectionModel,
	WalletDesktopModel,
	WalletHomeModel
} from './model';

export interface WalletLiveInputs {
	balance: BalanceView;
	currency: CurrencyView;
	m: WalletMessages;
	/** The feed, once Phase 4 boots it; `null` keeps the section a skeleton. */
	feed?: FeedView | null;
	/** The sidebar's network filter: one chain, or `null` for every network. */
	chainFilter?: number | null;
	/** Chains the person added (spec 038 #E9): always listed, even at zero. */
	customChainIds?: readonly number[];
	/**
	 * The held token whose detail the third column shows (spec 015's D3
	 * panel, live). Absent, the column is closed — or the flow host's.
	 */
	selectedToken?: BalanceToken;
}

/** A held token's key — chain, contract (or `native`), symbol — for a tap to name. */
export function balanceTokenId(token: BalanceToken): string {
	return `${token.chain_id}:${token.token_address ?? 'native'}:${token.symbol}`;
}

// ---------------------------------------------------------------------------
// The network filter
// ---------------------------------------------------------------------------

/**
 * The sidebar's network rows, from what is actually held: 全部 first, then one
 * row per chain with a holding, counted in tokens — the numbers the drawn
 * board shows for its fixture wallet, made true for this one. `selected`
 * follows the filter.
 */
export function liveChainRows(
	view: BalanceView,
	allNetworksLabel: string,
	filter: number | null,
	/**
	 * Networks the person added themselves (spec 038 #E9). A chain earns a
	 * row by holding something; one that was added on purpose is listed
	 * whether or not its balance has been read yet — otherwise adding Celo
	 * looks like nothing happened until the next fetch lands.
	 */
	customChainIds: readonly number[] = []
): ChainRowModel[] {
	const counts = new Map<number, number>();
	for (const token of view.tokens)
		counts.set(token.chain_id, (counts.get(token.chain_id) ?? 0) + 1);
	for (const id of customChainIds) if (!counts.has(id)) counts.set(id, 0);
	return [
		{
			name: allNetworksLabel,
			dot: 'all',
			count: view.tokens.length,
			selected: filter === null,
			chainId: null
		},
		...[...counts.entries()].map(([chainId, count]) => ({
			name: chainName(chainId),
			dot: chainColor(chainId),
			logoUrl: chainLogoURL(chainId),
			count,
			selected: filter === chainId,
			chainId
		}))
	];
}

/**
 * The feed narrowed to one chain — the phone app's `filterFeedRowsByChain`.
 * A day header whose items all fell away goes with them, or the list would
 * show dates with nothing under them.
 */
export function narrowedFeed(feed: FeedView, filter: number | null): FeedView {
	if (filter === null) return feed;
	const kept = feed.rows.filter((row) => row.type === 'header' || row.item.chain_id === filter);
	return {
		...feed,
		rows: kept.filter((row, i) => {
			if (row.type !== 'header') return true;
			const next = kept[i + 1];
			return next !== undefined && next.type !== 'header';
		})
	};
}

// ---------------------------------------------------------------------------
// Money presentation
// ---------------------------------------------------------------------------

/**
 * A USD amount in the display currency: converted at the committed rate, or
 * the USD figure itself when the shell could not price the currency —
 * `rate: null` is NOT 1 (024's rule; a defaulted 1 under a ¥ is a lie).
 */
export function moneyParts(
	usd: number,
	currency: CurrencyView
): { code: string; glyph: string; integer: string; decimals: string } {
	const convertible = currency.rate !== null;
	const code = convertible ? currency.code : 'USD';
	const amount = convertible ? usd * (currency.rate as number) : usd;
	const fixed = Math.abs(amount).toFixed(2);
	const [whole, frac] = fixed.split('.');
	// The person's own number preset, not the platform's idea of one (spec 028
	// D47). Money is where this stops being cosmetic: a wallet that groups one
	// way here and another way on the next machine is a wallet whose totals a
	// person has to re-read before believing.
	const grouped = groupDigits(whole);
	const glyph = currencyGlyph(code);
	return { code, glyph, integer: `${glyph}${grouped}`, decimals: frac };
}

export function moneyText(usd: number, currency: CurrencyView): string {
	const parts = moneyParts(usd, currency);
	return `${parts.integer}${numberSeparators().decimal}${parts.decimals}`;
}

/**
 * The unit drawn beside a figure that is being TYPED (issue 231): the send
 * form's hero read "4.00" with nothing to say whether that was dollars or
 * coins — the unit was known and reached only the screen reader.
 *
 * `code` is the FIGURE's own currency (`SendView.amount_fiat_code`), never the
 * display currency: a figure typed in CNY is still CNY in the instant the
 * display currency changes under it, and drawing the new symbol over the old
 * digits is the relabel this project has paid for more than once. `null` means
 * the figure is in token units, and the unit is the token's symbol.
 *
 * A currency with a real symbol leads the figure ("$4.00"), exactly as
 * `moneyText` writes it on the line beneath; one the catalog has no symbol for
 * follows it as its code ("4.00 PLN"), and a token always follows
 * ("0.00075 BNB"). Nothing here defaults to "$": an unknown unit is no
 * adornment at all, which is a gap a person can see rather than a claim.
 */
export function unitAdornment(
	code: string | null,
	tokenSymbol: string
): { prefix?: string; suffix?: string } {
	if (code === null) return tokenSymbol === '' ? {} : { suffix: tokenSymbol };
	const symbol = currencySymbol(code);
	return symbol === null ? { suffix: code } : { prefix: symbol };
}

/**
 * A human decimal balance, tail trimmed — the number is the core's.
 *
 * The DECIMAL MARK follows the chosen preset; the grouping deliberately does
 * not. A decimal comma read as a thousands separator is a hundredfold mistake
 * about an amount, which is the whole reason presets exist — but the mocks draw
 * token amounts ungrouped, and this feature wires preferences rather than
 * redrawing screens. Money (`moneyParts`) gets both.
 *
 * String operations only: a uint256 balance must never pass through a JS
 * `number`, and a "tidy" `parseFloat` here would be a wrong figure on the
 * screen someone signs from.
 */
export function trimBalance(balance: string, maxDecimals = 6): string {
	if (!balance.includes('.')) return balance;
	const [whole, frac] = balance.split('.');
	const cut = frac.slice(0, maxDecimals).replace(/0+$/, '');
	return cut === '' ? whole : `${whole}${numberSeparators().decimal}${cut}`;
}

/**
 * A token amount as every money surface reads it — the asset list's rows, the
 * Send picker and token card, the confirm and the receipt — ONE call, so the
 * balance on the home row and the balance beside the token on Send are the
 * same digits, and the figure `Max` writes (the core's `send::max_figure`) is
 * the figure the confirm repeats.
 *
 * The core's ladder, digit for digit: 6 places under 1, 4 under 1000, 2 from
 * 1000 (`l10n::number::format_token_amount`, which the desktop's rows use),
 * rounded HALF UP, trailing zeros dropped; an amount too small to survive six
 * places keeps two significant digits instead of reading `0`. String
 * arithmetic only — a uint256 must never pass through a JS `number` — and the
 * decimal mark is the person's preset. Ungrouped, like the field `Max` fills:
 * a figure and the balance it came from must read the same.
 *
 * It used to be `trimBalance`'s six places, TRUNCATED, at every magnitude:
 * `1.22456789` read "1.224567" on the row while Max wrote "1.2246", and a
 * balance less a fee could read one digit lower on the confirm than on the
 * form it came from.
 */
export function tokenAmountText(amount: string, rounding: 'half-up' | 'down' = 'half-up'): string {
	const exact = amount.trim();
	const dot = exact.indexOf('.');
	const intRaw = dot === -1 ? exact : exact.slice(0, dot);
	const fracRaw = dot === -1 ? '' : exact.slice(dot + 1);
	if ((intRaw === '' && fracRaw === '') || !/^\d*$/.test(intRaw) || !/^\d*$/.test(fracRaw)) {
		return exact;
	}
	const intDigits = intRaw.replace(/^0+/, '');
	const places = intDigits.length === 0 ? 6 : intDigits.length <= 3 ? 4 : 2;
	const digits = (intDigits + fracRaw.padEnd(places, '0').slice(0, places))
		.split('')
		.map((d) => d.charCodeAt(0) - 48);
	const next = fracRaw[places];
	// `down` for a CEILING the person may type back ("you can send up to"):
	// rounded up, it would be a figure the balance cannot cover.
	if (rounding === 'half-up' && next !== undefined && next >= '5') {
		let i = digits.length;
		for (;;) {
			if (i === 0) {
				digits.unshift(1);
				break;
			}
			i -= 1;
			if (digits[i] === 9) digits[i] = 0;
			else {
				digits[i] += 1;
				break;
			}
		}
	}
	const decimal = numberSeparators().decimal;
	const split = digits.length - places;
	const intPart = digits.slice(0, split).join('') || '0';
	const fracPart = digits.slice(split).join('').replace(/0+$/, '');
	if (intPart === '0' && fracPart === '') {
		// Below the ladder's last place: two significant digits, cut.
		const lead = /^0*/.exec(fracRaw)?.[0].length ?? 0;
		if (lead === fracRaw.length) return '0';
		const keep = Math.min(lead + 2, fracRaw.length);
		return `0${decimal}${fracRaw.slice(0, keep).replace(/0+$/, '')}`;
	}
	return fracPart === '' ? intPart : `${intPart}${decimal}${fracPart}`;
}

/**
 * A token figure that is about to be SENT: the person's decimal mark, and
 * every digit the core gave it. `trimBalance`'s six places are for a balance
 * being glanced at — a share of 0.00022989 printed as 0.000229 is not what
 * leaves the account, and a total must be the sum of the rows above it.
 */
export function exactAmount(amount: string): string {
	return trimBalance(amount, Number.MAX_SAFE_INTEGER);
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

export function liveBalance(
	view: BalanceView,
	currency: CurrencyView,
	m: WalletMessages
): BalanceModel {
	const base = {
		label: m.balance.totalLabel,
		a11yHide: m.balance.a11yHide,
		a11yShow: m.balance.a11yShow
	};

	if (view.hidden) {
		return {
			...base,
			currency: currency.rate !== null ? currency.code : 'USD',
			state: 'hidden',
			integer: BALANCE_MASK
		};
	}

	// The core withholds the display total while the skeleton shows; the
	// last-known cached total paints first, live replaces it (max(live,cached)
	// is the core's rule — this only chooses what to show meanwhile).
	const total = view.display_total_usd ?? view.cached_total_usd;
	if (total === null) {
		return {
			...base,
			currency: currency.rate !== null ? currency.code : 'USD',
			state: 'loading',
			// Spec 038 finding 15: a first launch with no network is
			// "unreachable" over the skeleton, never a settled-looking $0.
			...(view.unreachable
				? { status: { kind: 'warning' as const, text: m.balance.unreachable } }
				: {})
		};
	}

	const parts = moneyParts(total, currency);
	// A zero is "live" only once EVERY chain has answered: a partial zero (some
	// chain unreachable) is not a listening wallet, it is an unknown one.
	const zeroLive =
		total === 0 && !view.balance_unknown && !view.balance_partial && view.tokens.length === 0;
	const onCache = view.display_total_usd === null && view.cached_total_usd !== null;

	// One status line, most actionable first. `banner_chain_ids` is already
	// failed MINUS rate-limited — the core's exclusion (a rate limit heals on
	// its own; the balance quietly stays on cache, no nag) — so a chain here
	// really is unreachable and the person can fix its RPC (the Expo
	// `RpcTroubleBanner`, worded with its corpus). Then the core's notice.
	const banner = view.banner_chain_ids;
	const status: BalanceModel['status'] =
		banner.length === 1
			? {
					kind: 'warning',
					text: fill(m.assets.rpcUnavailableSingle, { name: chainName(banner[0]) })
				}
			: banner.length > 1
				? { kind: 'warning', text: fill(m.assets.rpcUnavailableMultiple, { count: banner.length }) }
				: view.refreshing || onCache || view.notice === 'still_updating'
					? { kind: 'refreshing', text: m.balance.stale }
					: view.notice === 'unpriced'
						? { kind: 'warning', text: m.balance.unpriced }
						: undefined;

	return {
		...base,
		currency: parts.code,
		state: zeroLive ? 'zero-live' : 'normal',
		integer: parts.integer,
		decimals: parts.decimals,
		// The mark between them is the preset's too (T480): `moneyParts`
		// grouped the integer by it, and a `.` drawn after `1.575` read as a
		// second thousands separator.
		decimalMark: numberSeparators().decimal,
		liveText: zeroLive ? m.balance.liveIndicator : undefined,
		status
	};
}

export function liveAssetRow(
	token: BalanceToken,
	currency: CurrencyView,
	m: WalletMessages,
	hidden: boolean
): AssetRowModel {
	const badgeChain = balanceTokenBadgeChainId(token);
	const fiat: AssetRowModel['fiat'] = hidden
		? { kind: 'masked' }
		: token.price_usd === null
			? { kind: 'no-price', text: m.balance.noPrice }
			: {
					kind: 'value',
					text: moneyText((parseFloat(token.balance) || 0) * token.price_usd, currency)
				};
	return {
		id: balanceTokenId(token),
		ticker: token.symbol,
		chain: chainName(token.chain_id),
		badgeColor: chainColor(token.chain_id),
		logoUrls: balanceTokenLogoURLs(token),
		badgeLogoUrl: badgeChain === null ? undefined : chainLogoURL(badgeChain),
		badgeHidden: badgeChain === null,
		balance: hidden ? MASK : tokenAmountText(token.balance),
		fiat,
		masked: hidden
	};
}

function assetsMode(view: BalanceView): SectionModel['mode'] {
	if (view.tokens.length > 0) return 'rows';
	// Nothing held yet — a skeleton while the first fetch is out, an empty
	// state once the core has actually looked.
	return view.holdings_loading || view.balance_unknown ? 'loading' : 'empty';
}

// ---------------------------------------------------------------------------
// Activity — the core's grouped rows, worded and formatted here
// ---------------------------------------------------------------------------

function localMidnight(ms: number): number {
	const d = new Date(ms);
	return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "Today" / "Yesterday" from the corpus; older days in the date preset. */
export function dayLabel(dayStartMs: number, m: WalletMessages, now = Date.now()): string {
	const today = localMidnight(now);
	if (dayStartMs === today) return m.activity.today;
	if (dayStartMs === today - 86_400_000) return m.activity.yesterday;
	// Older days read in the person's own date preset — which is what the phone
	// does (`activity.ts::dayGroupLabel` calls the same `formatDate`), so the
	// two clients group a history the same way.
	return formatDate(dayStartMs);
}

export function liveActivityRow(
	item: FeedItem,
	m: WalletMessages,
	hidden: boolean
): ActivityRowModel {
	const received = item.direction === 'in';
	const kind: ActivityRowModel['kind'] = received
		? 'received'
		: item.direction === 'out'
			? 'sent'
			: 'dapp';
	const who = item.alias ?? (item.counterparty !== null ? shortenAddress(item.counterparty) : null);
	const subtitle =
		who === null
			? chainName(item.chain_id)
			: fill(received ? m.activity.fromName : m.activity.toName, { name: who });
	const amount =
		item.value === null
			? String(item.batch?.count ?? '')
			: `${received ? '+' : '-'}${trimBalance(item.value)}`;
	return {
		id: item.id,
		kind,
		title:
			kind === 'received'
				? m.activity.received
				: kind === 'sent'
					? m.activity.sent
					: m.activity.dapp,
		subtitle,
		amount: hidden ? MASK : amount,
		unit: item.symbol,
		positive: received,
		masked: hidden,
		badgeColor: chainColor(item.chain_id),
		badgeLogoUrl: chainLogoURL(item.chain_id)
	};
}

/** The core emits headers and items already interleaved (invariant ⑥). */
export function liveActivityGroups(
	view: FeedView,
	m: WalletMessages,
	hidden: boolean
): ActivityGroupModel[] {
	const groups: ActivityGroupModel[] = [];
	for (const row of view.rows) {
		if (row.type === 'header') {
			groups.push({ label: dayLabel(row.day_start_ms, m), rows: [] });
			continue;
		}
		const last = groups.at(-1);
		const model = liveActivityRow(row.item, m, hidden);
		if (last === undefined) groups.push({ label: '', rows: [model] });
		else last.rows.push(model);
	}
	return groups;
}

/**
 * The feed has no "loaded" flag (an empty store and a pristine view are the
 * same rows). The store read lands in milliseconds while the balance fetch
 * takes seconds, so "the balance has looked" is a safe proxy for "the feed
 * has looked" — a presentation choice, recorded as such.
 */
function activityMode(view: BalanceView, feed: FeedView | null | undefined): SectionModel['mode'] {
	if (!feed) return 'loading';
	if (feed.rows.length > 0) return 'rows';
	return view.balance_unknown ? 'loading' : 'empty';
}

// ---------------------------------------------------------------------------
// The asset-detail column (spec 015 D3, live)
// ---------------------------------------------------------------------------

/**
 * One held token in the third column: what the drawn D3 panel shows for its
 * fixture BNB, for whichever row was tapped. The transactions under it are
 * the feed's rows for this token on this chain — the same rows the home
 * lists, narrowed the way the phone's token screen narrows them.
 */
export function liveAssetDetail(
	token: BalanceToken,
	inputs: WalletLiveInputs,
	drawn: AssetDetailPanelModel
): AssetDetailPanelModel {
	const { balance: view, currency, m } = inputs;
	const hidden = view.hidden;
	const badgeChain = balanceTokenBadgeChainId(token);
	const held = parseFloat(token.balance) || 0;
	const fiat =
		hidden || token.price_usd === null ? undefined : moneyText(held * token.price_usd, currency);
	const rows = (inputs.feed?.rows ?? [])
		.flatMap((row) => (row.type === 'item' ? [row.item] : []))
		.filter((item) => item.chain_id === token.chain_id && item.symbol === token.symbol)
		.map((item) => liveActivityRow(item, m, hidden));
	return {
		...drawn,
		id: balanceTokenId(token),
		title: token.symbol,
		token: {
			ticker: token.symbol,
			badgeColor: chainColor(token.chain_id),
			balance: hidden ? MASK : `${tokenAmountText(token.balance)} ${token.symbol}`,
			fiatLine: [fiat, chainName(token.chain_id)].filter((part) => part !== undefined).join(' · '),
			logoUrls: balanceTokenLogoURLs(token),
			badgeLogoUrl: badgeChain === null ? undefined : chainLogoURL(badgeChain),
			badgeHidden: badgeChain === null
		},
		facts: [
			{ label: m.assetDetail.labelName, value: token.name },
			{
				label: m.assetDetail.labelPrice,
				value:
					token.price_usd === null
						? m.balance.noPrice
						: fill(m.assetDetail.priceValue, {
								symbol: token.symbol,
								value: moneyText(token.price_usd, currency)
							})
			},
			{
				label: m.assetDetail.labelContract,
				value:
					token.token_address === null
						? m.assetDetail.nativeToken
						: shortenAddress(token.token_address),
				// The shortened form is for reading; the copy writes the whole one.
				copy: token.token_address === null ? undefined : m.identiconViewer.copyAddress,
				copyValue: token.token_address ?? undefined
			},
			{ label: m.assetDetail.labelDecimals, value: String(token.decimals) }
		],
		rows,
		explorerUrl: tokenExplorerURL(token, view.address)
	};
}

/**
 * Where "view on explorer" leads for a held token: the token page, scoped to
 * this account, for an ERC-20; the account page for the chain's native coin.
 * `null` for a chain with no explorer — no link rather than a wrong one.
 */
export function tokenExplorerURL(token: BalanceToken, account: string | null): string | undefined {
	const base = explorerBaseURL(token.chain_id);
	if (base === null) return undefined;
	if (token.token_address === null) {
		return account === null ? undefined : explorerAddressURL(token.chain_id, account);
	}
	const suffix = account === null ? '' : `?a=${account}`;
	return `${base}/token/${token.token_address}${suffix}`;
}

// ---------------------------------------------------------------------------
// Overlays
// ---------------------------------------------------------------------------

/**
 * What both shapes share: the hero, and the two sections under the filter.
 *
 * The filter narrows the holdings and the feed the way the phone app's
 * `selectedChainId` does (`HoldingsList.tsx`, `useHomeController.ts`); the
 * hero total stays the whole wallet's, as it does there. A chain filtered
 * down to nothing reads as the empty state, not as a blank list.
 */
function liveSections(inputs: WalletLiveInputs) {
	const { balance: view, currency, m } = inputs;
	const filter = inputs.chainFilter ?? null;
	const tokens = filter === null ? view.tokens : view.tokens.filter((t) => t.chain_id === filter);
	const feed = inputs.feed ? narrowedFeed(inputs.feed, filter) : inputs.feed;
	return {
		balance: liveBalance(view, currency, m),
		assetsMode:
			filter !== null && tokens.length === 0 && view.tokens.length > 0
				? ('empty' as const)
				: assetsMode(view),
		assetRows: tokens.map((t) => liveAssetRow(t, currency, m, view.hidden)),
		activityMode: activityMode(view, feed),
		activityGroups: feed ? liveActivityGroups(feed, m, view.hidden) : []
	};
}

/** Live balance + holdings over an identity-filled home model. */
export function withLiveWallet(model: WalletHomeModel, inputs: WalletLiveInputs): WalletHomeModel {
	const live = liveSections(inputs);
	return {
		...model,
		balance: live.balance,
		assetsSection: { ...model.assetsSection, mode: live.assetsMode },
		assetRows: live.assetRows,
		activitySection: { ...model.activitySection, mode: live.activityMode },
		activityGroups: live.activityGroups
	};
}

/** The desktop shape of the same overlay — plus the sidebar's network list. */
export function withLiveWalletDesktop(
	model: WalletDesktopModel,
	inputs: WalletLiveInputs
): WalletDesktopModel {
	const live = liveSections(inputs);
	return {
		...model,
		sidebar: {
			...model.sidebar,
			networks: liveChainRows(
				inputs.balance,
				inputs.m.networkFilter.allNetworks,
				inputs.chainFilter ?? null,
				inputs.customChainIds ?? []
			)
		},
		balance: live.balance,
		assetsSection: { ...model.assetsSection, mode: live.assetsMode },
		assetRows: live.assetRows,
		activitySection: { ...model.activitySection, mode: live.activityMode },
		activityGroups: live.activityGroups,
		// The third column is the model's to open on a live page: a tapped row
		// puts its token here, and closing the column takes it away again.
		panels:
			inputs.selectedToken === undefined
				? model.panels
				: {
						...model.panels,
						assetDetail: liveAssetDetail(inputs.selectedToken, inputs, model.panels.assetDetail)
					},
		initialPanel: inputs.selectedToken === undefined ? 'none' : 'asset-detail'
	};
}
