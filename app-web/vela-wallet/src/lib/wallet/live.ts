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
import type { FeedAllowance } from '$lib/core/generated/FeedAllowance';
import type { FeedDapp } from '$lib/core/generated/FeedDapp';
import type { FeedDappChange } from '$lib/core/generated/FeedDappChange';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedLine } from '$lib/core/generated/FeedLine';
import type { FeedRow } from '$lib/core/generated/FeedRow';
import type { FeedView } from '$lib/core/generated/FeedView';
import { formatRelativeTime } from '$lib/core/kernels';
import {
	formatDate,
	groupDigits,
	numberSeparators,
	resolvedFormatKeys
} from '$lib/services/locale-format';
import { chainName, explorerAddressURL, explorerBaseURL } from '$lib/services/networks';
import { balanceTokenMark, chainLogoURL } from '$lib/flows/marks';
import { shortenAddress } from './identity';
import { fill } from './messages';
import { currencyGlyph, currencySymbol } from '$lib/settings/fixtures';
import { BALANCE_MASK, chainColor, MASK } from './fixtures';
import type { WalletMessages } from './messages';

/**
 * The words an Activity row is built from — the wallet bundle's `activity`
 * part, which a contact's page ships too (spec 093), so both draw the core's
 * rows through one builder.
 */
export type RowMessages = Pick<WalletMessages, 'activity'>;
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
	/** Issue 462: the hero's refresh control — its clock and press hold. Absent, none. */
	refresh?: BalanceRefreshInput;
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
 *
 * Both lists are narrowed the same way: `rows` (History, every one) and
 * `home_rows` (the home's newest three, issue 469). Which rows are the
 * home's is the core's cut — this only drops what is not on the chain, it
 * never counts.
 */
export function narrowedFeed(feed: FeedView, filter: number | null): FeedView {
	if (filter === null) return feed;
	const narrowed = (rows: FeedRow[]): FeedRow[] => {
		const kept = rows.filter((row) => row.type === 'header' || row.item.chain_id === filter);
		return kept.filter((row, i) => {
			if (row.type !== 'header') return true;
			const next = kept[i + 1];
			return next !== undefined && next.type !== 'header';
		});
	};
	return { ...feed, rows: narrowed(feed.rows), home_rows: narrowed(feed.home_rows) };
}

// ---------------------------------------------------------------------------
// Money presentation
// ---------------------------------------------------------------------------

/**
 * `value` to two decimals as plain digits, never exponent notation (spec 082
 * G60): `toFixed` switches to "1e+24" from 10^21 up, and a dApp transfer of
 * 10^30 token units was shown as "≈ $1e+24". Past 10^15 a double has no cents
 * left to show, so the figure is its 15 significant digits and zeros.
 * `[whole, fraction]`; anything that is not a finite number reads 0.
 */
export function fixedTwo(value: number): [string, string] {
	if (!Number.isFinite(value)) return ['0', '00'];
	if (Math.abs(value) < 1e15) {
		const [whole, frac] = value.toFixed(2).split('.');
		return [whole, frac ?? '00'];
	}
	const [mantissa, exponent] = Math.abs(value).toExponential(14).split('e');
	const digits = mantissa.replace('.', '');
	const places = Number.parseInt(exponent, 10) + 1;
	const whole = digits.length >= places ? digits.slice(0, places) : digits.padEnd(places, '0');
	return [value < 0 ? `-${whole}` : whole, '00'];
}

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
	const [whole, frac] = fixedTwo(Math.abs(amount));
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

/**
 * Spec 092: the line over the networks the wallet cannot reach — the home's
 * status line and the title of the list it opens. The core chooses the
 * sentence (`unreachable_key`: one network named, several counted); this only
 * fills it. `undefined` when every network answered.
 *
 * PR 2 note 11 (issue 483): a read that failed inside Vela itself is said in
 * that line instead, by the core's key (`internal_key`) — an internal fault
 * never reads "Can't reach Ethereum", and the core already leaves such chains
 * out of `unreachable_networks`. A key this build has no words for falls
 * through to the network line (or none), never a dotted path.
 */
export function unreachableLine(
	view: Pick<BalanceView, 'unreachable_networks' | 'unreachable_key'> &
		Partial<Pick<BalanceView, 'internal_key'>>,
	words: {
		unreachableOne: string;
		unreachableMany: string;
		internal?: Readonly<Record<string, string>>;
	}
): string | undefined {
	const internal = view.internal_key ? words.internal?.[view.internal_key] : undefined;
	if (internal !== undefined) return internal;
	const first = view.unreachable_networks[0];
	if (view.unreachable_key === 'assets.unreachableOne' && first !== undefined) {
		return fill(words.unreachableOne, { name: chainName(first.chain_id) });
	}
	if (view.unreachable_key === 'assets.unreachableMany') {
		return fill(words.unreachableMany, { n: view.unreachable_networks.length });
	}
	return undefined;
}

/**
 * "now" / "2m" / "3h" / a weekday / the date — the core's compact relative
 * time (`formatRelativeTime`, the rule the phones and the desktop ask too),
 * on this device's clock and zone, in the person's date preset. `language`
 * is the page's locale, which names the weekday.
 */
export function agoText(
	atMs: number,
	nowMs: number,
	words: WalletMessages['balance']['ago'],
	language: string
): string {
	return formatRelativeTime(
		atMs,
		nowMs,
		-new Date(atMs).getTimezoneOffset(),
		resolvedFormatKeys().date,
		language,
		words
	);
}

/** What the page holds for the hero's refresh control (issue 462). */
export interface BalanceRefreshInput {
	/** The clock the "Updated <ago>" is worded against; the page ticks it. */
	now: number;
	/** A press's minimum spin (650 ms) is still running (`RefreshHold`). */
	held: boolean;
	/** The page's locale: it names the weekday once a read is a day old. */
	language: string;
}

/**
 * The hero's "↻ Updated 2m" (issue 462): when the last read settled — the
 * core's `last_refreshed_at_ms` — and whether the glyph turns: while the read
 * the person asked for is out (`view.refreshing`, which only a pull sets),
 * and in any case for the press's minimum hold, so a read answered in a
 * frame still says it happened.
 */
export function balanceRefresh(
	view: BalanceView,
	m: WalletMessages,
	input: BalanceRefreshInput
): NonNullable<BalanceModel['refresh']> {
	const at = view.last_refreshed_at_ms;
	return {
		updated:
			at === null
				? undefined
				: fill(m.balance.lastUpdated, {
						ago: agoText(at, input.now, m.balance.ago, input.language)
					}),
		updating: m.balance.updating,
		a11yIdle: m.balance.refreshBalance,
		spinning: view.refreshing || input.held
	};
}

export function liveBalance(
	view: BalanceView,
	currency: CurrencyView,
	m: WalletMessages,
	/** Issue 462: the refresh control's clock and hold. Absent, no control. */
	refreshInput?: BalanceRefreshInput
): BalanceModel {
	const base = {
		label: m.balance.totalLabel,
		a11yHide: m.balance.a11yHide,
		a11yShow: m.balance.a11yShow,
		// Under a skeleton and under a hidden figure too: hiding the figure is
		// not hiding that it is being read, and a first read that hangs is
		// exactly when a person reaches for it.
		...(refreshInput === undefined ? {} : { refresh: balanceRefresh(view, m, refreshInput) })
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
	// Nothing known (`unreachable`): the first read failed — or settled with
	// every chain it asked failed (PR 2 integration) — and nothing is cached.
	// The core gives no figure for it (`display_total_usd` is null, PR 2
	// polish); the skeleton and the reason, never a zero.
	if (total === null || view.unreachable) {
		return {
			...base,
			currency: currency.rate !== null ? currency.code : 'USD',
			state: 'loading',
			// Spec 038 finding 15: a first launch with no network is
			// "unreachable" over the skeleton, never a settled-looking $0 —
			// unless what failed was Vela itself (PR 2 note 11): that is said
			// as Vela's own fault, never as the network.
			...(view.unreachable || view.internal_key
				? {
						status: {
							kind: 'warning' as const,
							text:
								(view.internal_key ? m.assets.internal[view.internal_key] : undefined) ??
								m.balance.unreachable
						}
					}
				: {})
		};
	}

	const parts = moneyParts(total, currency);
	// A zero is "live" only once EVERY chain has answered: a partial zero (some
	// chain unreachable) is not a listening wallet, it is an unknown one.
	const zeroLive =
		total === 0 && !view.balance_unknown && !view.balance_partial && view.tokens.length === 0;
	const onCache = view.display_total_usd === null && view.cached_total_usd !== null;

	// One status line, most actionable first: the networks the wallet cannot
	// reach (spec 092 — every one, held or not; a rate limit heals on its own
	// and is never listed) — or, in their place, a read that failed inside
	// Vela (PR 2 note 11) — then the core's notice.
	//
	// NOT a read the person asked for (issue 462, `view.refreshing`): the
	// control they pressed says that itself, turning in place. As a line here
	// it landed ABOVE the control and pushed it a row down under the finger —
	// and a figure re-read on request is current, not "still updating".
	const unreachable = unreachableLine(view, m.assets);
	const status: BalanceModel['status'] =
		unreachable !== undefined
			? { kind: 'warning', text: unreachable }
			: onCache || view.notice === 'still_updating'
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
	const art = balanceTokenMark(token);
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
		badgeColor: art.badgeColor,
		logoUrls: art.logoUrls,
		badgeLogoUrl: art.badgeLogoUrl,
		badgeHidden: art.badgeHidden,
		balance: hidden ? MASK : tokenAmountText(token.balance),
		fiat,
		masked: hidden
	};
}

function assetsMode(view: BalanceView): SectionModel['mode'] {
	if (view.tokens.length > 0) return 'rows';
	// Nothing held yet — a skeleton while the first fetch is out, an empty
	// state once the core has actually looked. A look that reached nothing
	// (`unreachable`) is no look: never "Deposit your first asset" under a
	// line saying nothing could be read.
	return view.holdings_loading || view.balance_unknown || view.unreachable ? 'loading' : 'empty';
}

// ---------------------------------------------------------------------------
// Activity — the core's grouped rows, worded and formatted here
// ---------------------------------------------------------------------------

function localMidnight(ms: number): number {
	const d = new Date(ms);
	return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "Today" / "Yesterday" from the corpus; older days in the date preset. */
export function dayLabel(dayStartMs: number, m: RowMessages, now = Date.now()): string {
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
	m: RowMessages,
	hidden: boolean,
	/** The clock a `day` part is worded against (tests inject one). */
	now = Date.now()
): ActivityRowModel {
	// Spec 082 RG1, spec 093: what the row IS, and where it stands, are the
	// core's (`FeedItem.kind`, `.status`, `.dapp`) — never guessed from a
	// direction or looked up in the store here. A row the core describes as a
	// dApp's (its transactions and its signatures) is a dApp row.
	const kind: ActivityRowModel['kind'] =
		item.dapp != null || (item.kind !== 'receive' && item.kind !== 'send')
			? 'dapp'
			: item.kind === 'receive'
				? 'received'
				: 'sent';
	const received = kind === 'received';
	const figure = rowFigure(item, received, m);
	// Whether this row's figure is money is the core's (`FeedItem.figure_maskable`,
	// `app::privacy`): an amount, a batch count or a capped allowance masks; an
	// unlimited allowance or a figureless signature does not.
	const masked = hidden && item.figure_maskable;
	const back = item.dapp?.received ?? null;
	return {
		id: item.id,
		kind,
		title: item.dapp
			? dappTitle(item.dapp, m)
			: kind === 'received'
				? m.activity.received
				: kind === 'sent'
					? m.activity.sent
					: m.activity.dapp,
		// The core's second line (spec 093), worded: status first when the
		// tracker has not closed the row (RG2, 087 F04), then the site, the
		// network, or whom a transfer went to — in the core's order.
		subtitle: subtitleText(item.subtitle ?? [], m, now),
		amount: masked ? MASK : figure.amount,
		unit: figure.unit,
		positive: received,
		masked,
		...(figure.danger ? { danger: true } : {}),
		...(back === null
			? {}
			: { received: { amount: hidden ? MASK : changeFigure(back), unit: back.symbol } }),
		badgeColor: chainColor(item.chain_id),
		badgeLogoUrl: chainLogoURL(item.chain_id)
	};
}

/**
 * The right column of a row: the money it moved (as before; "≈" when the
 * figure is the simulation's expectation, 083 F1), else the allowance a grant
 * states (spec 093) — "Unlimited" in the danger tone, never masked: it is a
 * risk to see, not a balance — else a batch's count, else nothing. A dApp row
 * that moved and granted nothing has no figure, and nothing to mask: "••••"
 * would claim one (083 H2 review).
 */
function rowFigure(
	item: FeedItem,
	received: boolean,
	m: RowMessages
): { amount: string; unit: string; danger: boolean } {
	if (item.value !== null) {
		const about = item.dapp?.estimated ? '≈ ' : '';
		return {
			amount: `${about}${received ? '+' : '−'}${trimBalance(item.value)}`,
			unit: item.symbol,
			danger: false
		};
	}
	const allowance = item.dapp?.allowance ?? null;
	if (allowance !== null) return allowanceFigure(allowance, m);
	if (item.dapp != null) return { amount: '', unit: '', danger: false };
	return {
		amount: String(item.batch?.count ?? ''),
		unit: item.symbol,
		danger: false
	};
}

/**
 * An allowance as a row figure (spec 093): the core's cap and symbol, worded.
 * Whether it masks is the core's (`FeedItem.figure_maskable`), never this.
 */
export function allowanceFigure(
	allowance: FeedAllowance,
	m: RowMessages
): { amount: string; unit: string; danger: boolean } {
	if (allowance.unlimited) {
		return { amount: m.activity.unlimited, unit: allowance.symbol, danger: true };
	}
	return {
		amount: allowance.value === null ? '' : trimBalance(allowance.value),
		unit: allowance.symbol,
		danger: false
	};
}

/**
 * One line of what a dApp operation moved (083 F1): "−", "+" and the figure,
 * "≈" on every one the wallet cannot vouch for to the unit (`exact` is the
 * core's), and only a direction for a token nobody verified.
 */
export function changeFigure(change: FeedDappChange): string {
	const sign = change.direction === 'in' ? '+' : '−';
	if (!change.verified || change.value === null) return sign;
	return `${change.exact ? '' : '≈ '}${sign}${trimBalance(change.value)}`;
}

/**
 * A row's second line (spec 093): the core's parts, in its order, each worded
 * here and joined " · ". Which parts there are — a status the tracker has not
 * closed, the site when the title named a protocol, the network, whom a
 * transfer went to or came from, the day on a contact's page — is the core's
 * (`FeedItem.subtitle`). The day reads as the date headers do.
 */
export function subtitleText(lines: readonly FeedLine[], m: RowMessages, now = Date.now()): string {
	return lines
		.map((line) => {
			switch (line.type) {
				case 'status':
					return line.status === 'pending'
						? m.activity.pending
						: line.status === 'failed'
							? m.activity.failed
							: line.status === 'unknown'
								? m.activity.unknown
								: '';
				case 'to':
					return fill(m.activity.toName, { name: line.name ?? shortenAddress(line.address) });
				case 'from':
					return fill(m.activity.fromName, { name: line.name ?? shortenAddress(line.address) });
				case 'site':
					return line.site;
				case 'network':
					return chainName(line.chain_id);
				case 'day':
					return dayLabel(line.day_start_ms, m, now);
			}
		})
		.filter((part) => part !== '')
		.join(' · ');
}

/**
 * The headline verb of a dApp row (spec 093): the wallet's own word for the
 * core's `intent_term` in the reader's language, else the descriptor's text
 * when the wallet has no word for it.
 */
function dappVerb(dapp: FeedDapp, m: RowMessages): string {
	const word = dapp.intent_term ? m.activity.intents[dapp.intent_term] : undefined;
	return word || dapp.intent || m.activity.contractCall;
}

/**
 * What a dApp row is titled (spec 093): "<verb> on <place>" — the place being
 * a protocol the wallet knows by address, else the site's host, as the core
 * decided — or the verb alone when there is no place.
 */
export function dappTitle(dapp: FeedDapp, m: RowMessages): string {
	const intent = dappVerb(dapp, m);
	return dapp.place != null ? fill(m.activity.dappRowTitle, { intent, place: dapp.place }) : intent;
}

/**
 * The core emits headers and items already interleaved (invariant ⑥).
 *
 * `rows` is the list to draw, and WHICH list is the caller's to say: History
 * hands `FeedView.rows` (every one), the home hands `FeedView.home_rows` (the
 * core's newest three, issue 469). Nothing here caps — a cap in this helper
 * would silently cut History short too.
 */
export function liveActivityGroups(
	rows: FeedRow[],
	m: WalletMessages,
	hidden: boolean
): ActivityGroupModel[] {
	const groups: ActivityGroupModel[] = [];
	for (const row of rows) {
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
	const art = balanceTokenMark(token);
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
			badgeColor: art.badgeColor,
			balance: hidden ? MASK : `${tokenAmountText(token.balance)} ${token.symbol}`,
			fiatLine: [fiat, chainName(token.chain_id)].filter((part) => part !== undefined).join(' · '),
			logoUrls: art.logoUrls,
			badgeLogoUrl: art.badgeLogoUrl,
			badgeHidden: art.badgeHidden
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
		balance: liveBalance(view, currency, m, inputs.refresh),
		assetsMode:
			filter !== null && tokens.length === 0 && view.tokens.length > 0
				? ('empty' as const)
				: assetsMode(view),
		assetRows: tokens.map((t) => liveAssetRow(t, currency, m, view.hidden)),
		activityMode: activityMode(view, feed),
		// The home's Activity is the newest three (issue 469): the core's
		// `home_rows`, never `rows` — a long history pushed Assets off the
		// screen. "All" opens History, which draws every row.
		// The feed masks on its own flag (`FeedView.hidden`), never the
		// balance machine's threaded through (`app::privacy`).
		activityGroups: feed ? liveActivityGroups(feed.home_rows, m, feed.hidden) : [],
		// Spec 082 RG5: which empty line the home says is the core's
		// (`FeedView.home_empty_key`) — "no activity" or "none on this network".
		activityEmpty: {
			title:
				inputs.feed?.home_empty_key === 'home.emptyNoActivityNetwork'
					? m.activity.emptyTitleNetwork
					: m.activity.emptyTitle,
			caption: m.activity.emptyCaption
		}
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
		activitySection: {
			...model.activitySection,
			mode: live.activityMode,
			empty: live.activityEmpty
		},
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
		activitySection: {
			...model.activitySection,
			mode: live.activityMode,
			empty: live.activityEmpty
		},
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
