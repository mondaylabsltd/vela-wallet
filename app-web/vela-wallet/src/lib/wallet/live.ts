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
import { currencyFractionDigits, fiatSymbol, maskedAmount } from '$lib/core/client';
import { checksumAddress, formatRelativeTime } from '$lib/core/kernels';
import {
	formatDate,
	groupDigits,
	numberSeparators,
	resolvedFormatKeys
} from '$lib/services/locale-format';
import { chainName, explorerAddressURL, explorerBaseURL, namedChain } from '$lib/services/networks';
import { balanceTokenMark, chainLogoURL } from '$lib/flows/marks';
import { provenanceNote, type ProvenanceNote } from '$lib/services/token-provenance';
import { shortenAddress } from './identity';
import { fill } from './messages';
import { pageLanguage } from '$lib/i18n/page-language.svelte';
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
	ActivityTitlePart,
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
	/**
	 * Spec 105: the testnet space is in front, and this is what it says in
	 * place of a total. Nothing there has real value: no total, no worth on
	 * a row. Absent, the mainnet space.
	 */
	noValue?: string;
	/**
	 * r13-1: the account shown lives in the other space (the core's
	 * `network_space::lives_elsewhere`): `heading` (that space's) stands in
	 * the figure's place, and the home claims nothing of the space in front
	 * — no status line ("Live · listening for payments"), no refresh, no
	 * empty states ("No activity yet", "Deposit your first asset").
	 */
	elsewhere?: { heading: string };
	/**
	 * r13-7: the shown multisig's history (`ProposalsView.history`): while
	 * its first read is out, Activity is being read, never "No activity yet";
	 * if it could not be read, Activity says that (`unreadText`).
	 */
	multisigHistory?: {
		stand: 'reading' | 'read' | 'unread';
		unreadText: string;
		/**
		 * §18 item 3: Try again under the unread line — the proposals machine's
		 * `retry` (`MULTISIG_RETRY`), as the natives draw it on the empty state.
		 */
		retry?: { id: string; label: string };
	};
}

/**
 * §18 item 3: Activity says the shown multisig's history could not be read —
 * its empty state, with Try again (the natives' place) — so the waiting
 * group does not say it again: one sentence and one control per failure.
 * Over rows (the chain filter's too), Activity shows the rows and the group's
 * note says it, with its Retry.
 */
export function activitySaysHistory(
	inputs: Pick<WalletLiveInputs, 'feed' | 'chainFilter' | 'multisigHistory'> & {
		balance?: Pick<BalanceView, 'address'>;
	}
): boolean {
	const filter = inputs.chainFilter ?? null;
	const mine = inputs.balance ? feedFor(inputs.feed, inputs.balance.address) : inputs.feed;
	const feed = mine ? narrowedFeed(mine, filter) : mine;
	return !!feed && feed.rows.length === 0 && inputs.multisigHistory?.stand === 'unread';
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
 * r15-1 (§23): the feed, only when its rows are the shown account's
 * (`FeedView.account`, the core's word) — else `null`, drawn as loading. A
 * switch empties the core's rows at once; until the feed names the account
 * shown, nothing it holds is drawn under it.
 */
export function feedFor(
	feed: FeedView | null | undefined,
	shown: string | null | undefined
): FeedView | null | undefined {
	if (!feed) return feed;
	return !!shown && !!feed.account && feed.account.toLowerCase() === shown.toLowerCase()
		? feed
		: null;
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
	return fixedPlaces(value, 2);
}

/**
 * `fixedTwo` at `places` fraction digits — the currency's own count (a yen
 * figure has none, so its fraction is `''`).
 */
export function fixedPlaces(value: number, places: number): [string, string] {
	const zeros = '0'.repeat(places);
	if (!Number.isFinite(value)) return ['0', zeros];
	if (Math.abs(value) < 1e15) {
		const [whole, frac] = value.toFixed(places).split('.');
		return [whole, frac ?? zeros];
	}
	const [mantissa, exponent] = Math.abs(value).toExponential(14).split('e');
	const digits = mantissa.replace('.', '');
	const count = Number.parseInt(exponent, 10) + 1;
	const whole = digits.length >= count ? digits.slice(0, count) : digits.padEnd(count, '0');
	return [value < 0 ? `-${whole}` : whole, zeros];
}

/**
 * What stands where a money figure WILL be, while the display currency is not
 * the person's yet.
 *
 * The core's rule (`CurrencyView.committed`, and the module doc of
 * `display_currency.rs`): while it is false the pair on the wire is the USD/1
 * placeholder, and no fiat figure is drawn in it — on ANY surface. A home that
 * showed "$1,234" for a few seconds and then jumped to "¥8,876" is what the
 * rule ends (the 102 device run). The hero keeps its skeleton (`liveBalance`);
 * every other figure is this mark, so the line it stands on is already there
 * and already the height the figure will be. The figure appears once, in the
 * right money, where its room was kept.
 */
export const MONEY_PENDING = '…';

/**
 * The currency a figure is — or will be — counted in, for a surface that
 * names it apart from the figure (the hero's "Total · CNY").
 *
 * Committed: the code the figure is really in — the display currency, or USD
 * when it could not be priced (`moneyParts` degrades the same way). Not yet:
 * the person's stored choice on its way (`CurrencyView.pending`), and nothing
 * at all while even that is unknown — never the placeholder's "USD".
 */
export function figureCurrency(currency: CurrencyView): string | undefined {
	if (!currency.committed) return currency.pending ?? undefined;
	return currency.rate !== null ? currency.code : 'USD';
}

/** The pattern space between a symbol that ends in a letter and its digits. */
const NBSP = '\u00A0';

/**
 * The symbol a figure in `code` LEADS with, as the page's language writes it
 * — or `null` when the currency has no glyph of its own and is named by its
 * code instead.
 *
 * The symbol is the core's (`fiat_symbol`, device walk of 2026-10-11, item 8):
 * one rule for all four apps, so a figure reads the same string on the home
 * total, a fee's worth, the send form and Settings' currency row. The web's
 * own table wrote the Hong Kong dollar with the US dollar's "$" and the yen
 * and the yuan with one "¥"; under the core's rule no two currencies are
 * written alike within a language ("¥" is the reader's own of the two, the
 * other is "JP¥" / "CN¥").
 *
 * What is the web's is only the spacing, and it is the core formatter's too:
 * a symbol that ends in a letter ("zł", "Rp") is held off the digits by a
 * no-break space, and a glyph sits against them.
 */
function leadingSymbol(code: string): string | null {
	const symbol = fiatSymbol(code, pageLanguage());
	if (symbol === code.trim().toUpperCase()) return null;
	return /\p{L}$/u.test(symbol) ? symbol + NBSP : symbol;
}

/** A fiat figure, in the pieces a surface draws it from. */
export interface MoneyParts {
	code: string;
	glyph: string;
	integer: string;
	decimals: string;
}

/**
 * THE ONE PLACE A FIAT FIGURE IS MADE — a USD amount in the display currency,
 * or `null`: WITHHELD, because the display currency has not committed.
 *
 * Every figure the web draws in the display currency comes through here (by
 * way of `moneyText`, or directly for the hero's two-size figure), so no
 * surface has to remember the withhold rule and none can forget it: there is
 * no other formatter to reach for, and this one answers "withheld" before it
 * answers anything else. `fiat-withheld.test.ts` holds every surface the core
 * names (`FIAT_SURFACES`) to it.
 *
 * Committed: converted at the committed rate, or the USD figure itself when
 * the shell could not price the currency — `rate: null` is NOT 1 (024's rule;
 * a defaulted 1 under a ¥ is a lie).
 */
export function moneyParts(usd: number, currency: CurrencyView): MoneyParts | null {
	// Not the person's currency yet: no figure, in any money.
	if (!currency.committed) return null;
	const convertible = currency.rate !== null;
	const code = convertible ? currency.code : 'USD';
	const amount = convertible ? usd * (currency.rate as number) : usd;
	// The currency's own minor units (the core's table): "¥194,912", never
	// "¥194,911.60" — the yen, the won and the rupiah have no cents to show.
	const [whole, frac] = fixedPlaces(Math.abs(amount), currencyFractionDigits(code));
	// The person's own number preset, not the platform's idea of one (spec 028
	// D47). Money is where this stops being cosmetic: a wallet that groups one
	// way here and another way on the next machine is a wallet whose totals a
	// person has to re-read before believing.
	const grouped = groupDigits(whole);
	// A currency with no glyph leads with its code, held off the digits the
	// same way ("CHF 4.00").
	const glyph = leadingSymbol(code) ?? code + NBSP;
	return { code, glyph, integer: `${glyph}${grouped}`, decimals: frac };
}

/** A fiat figure as one string — or the pending mark while it is withheld. */
export function moneyText(usd: number, currency: CurrencyView): string {
	const parts = moneyParts(usd, currency);
	if (parts === null) return MONEY_PENDING;
	// A zero-decimal currency has no fraction, and so no decimal mark.
	if (parts.decimals === '') return parts.integer;
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
 * `moneyText` writes it on the line beneath; one the core has no symbol for
 * follows it as its code ("4.00 CHF"), and a token always follows
 * ("0.00075 BNB"). Nothing here defaults to "$": an unknown unit is no
 * adornment at all, which is a gap a person can see rather than a claim.
 */
export function unitAdornment(
	code: string | null,
	tokenSymbol: string
): { prefix?: string; suffix?: string } {
	if (code === null) return tokenSymbol === '' ? {} : { suffix: tokenSymbol };
	const symbol = leadingSymbol(code);
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
 * A figure the core has ALREADY cut for reading (`cut_token_amount` — the
 * remainder of a split, `SendView.split_remaining_display`), with the
 * person's decimal mark and nothing else done to it.
 *
 * Not `tokenAmountText`: that rounds half up, which is right for a balance
 * and wrong for what is LEFT — "1.199999999999999999" left read "1.2 left",
 * more than there is. The core cuts, never rounds; this only writes its
 * figure the way the balance beside it is written. Every digit the core gave
 * is kept: dust is cut at its first significant digit, not to "0".
 */
export function cutFigureText(figure: string): string {
	return trimBalance(figure, figure.length);
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
 * decimal mark and the grouping are the person's preset (r4a-21: one number
 * format everywhere — the multisig's sheet read "1,000" where the confirm read
 * "1000"). Only an editable field is never grouped (`inputSeparators`), so a
 * separator never jumps about under the person's typing.
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
	const grouped = groupDigits(intPart);
	return fracPart === '' ? grouped : `${grouped}${decimal}${fracPart}`;
}

/**
 * An amount that moved, or is about to: ONE rule for the confirm's headline,
 * the receipt, the activity row and its detail (send-12) — the ladder of
 * {@link tokenAmountText}, cut, never rounded up. The receipt rounded half
 * up while the activity row truncated at six places, so one transfer of
 * 1.23456789 read "1.2346" on one screen and "1.234567" on the next; and a
 * figure rounded up says more left the account than did.
 */
export function sentAmountText(amount: string): string {
	return tokenAmountText(amount, 'down');
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
 * send-19: the balance as the send screens read it. The home lists every
 * network it cannot reach (spec 092); while money is being sent only the
 * networks that matter to it warn — the ones the person holds something on
 * (the core's `last_known`), and the one the send moves on. "Can't reach
 * Monad" over a Gnosis-only account sending on Gnosis is noise. The
 * sentence keeps the core's choice for what is left: the one network named,
 * several counted, or nothing.
 */
export function sendScopedOutage(view: BalanceView, sendChainId: number | null): BalanceView {
	const kept = view.unreachable_networks.filter(
		(network) => network.last_known === 'held' || network.chain_id === sendChainId
	);
	if (kept.length === view.unreachable_networks.length) return view;
	const listKey = view.unreachable_key === 'assets.tokenListUnreachable';
	const key =
		kept.length === 0
			? null
			: kept.length === 1
				? listKey
					? view.unreachable_key
					: 'assets.unreachableOne'
				: 'assets.unreachableMany';
	return { ...view, unreachable_networks: kept, unreachable_key: key };
}

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
 *
 * PR 3 note 4: the one network may be one whose RPC is fine and whose token
 * list could not be loaded (Tempo, its registry document away). The core names
 * that sentence too (`assets.tokenListUnreachable`, `{{name}}` as for the one
 * network) — "Can't reach Tempo" there named the wrong thing.
 */
export function unreachableLine(
	view: Pick<BalanceView, 'unreachable_networks' | 'unreachable_key'> &
		Partial<Pick<BalanceView, 'internal_key'>>,
	words: {
		unreachableOne: string;
		unreachableMany: string;
		tokenListUnreachable: string;
		/** Owner decision 2026-10-28: the one network's own node (`{{name}}`). */
		ownNodeUnreachable?: string;
		internal?: Readonly<Record<string, string>>;
	}
): string | undefined {
	const internal = view.internal_key ? words.internal?.[view.internal_key] : undefined;
	if (internal !== undefined) return internal;
	const first = view.unreachable_networks[0];
	if (view.unreachable_key === 'assets.unreachableOne' && first !== undefined) {
		return fill(words.unreachableOne, { name: chainName(first.chain_id) });
	}
	if (view.unreachable_key === 'assets.tokenListUnreachable' && first !== undefined) {
		return fill(words.tokenListUnreachable, { name: chainName(first.chain_id) });
	}
	if (
		view.unreachable_key === 'assets.ownNodeUnreachable' &&
		first !== undefined &&
		words.ownNodeUnreachable !== undefined
	) {
		return fill(words.ownNodeUnreachable, { name: chainName(first.chain_id) });
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
	// Round 16 leftover 6: "Updated" is the core's one answer (`updated_at_ms`):
	// never a freshness claim while a network said for the account is unread.
	const at = view.updated_at_ms;
	return {
		updated:
			at === null || at === undefined
				? undefined
				: fill(m.balance.lastUpdated, {
						ago: agoText(at, input.now, m.balance.ago, input.language)
					}),
		updating: m.balance.updating,
		a11yIdle: m.balance.refreshBalance,
		spinning: view.refreshing || input.held
	};
}

/**
 * What the hero's line says about the networks, whatever the figure above it
 * shows — a figure, a mask, or the testnet space's words: the networks the
 * wallet cannot reach (spec 092 — every one, held or not; a rate limit heals
 * on its own and is never listed), or in their place a read that failed
 * inside Vela (PR 2 note 11); else that what is shown is still the last
 * read (`onCache`, or the core's "still updating" notice). `undefined` when
 * the networks have nothing to say.
 */
function networkStatus(
	view: BalanceView,
	m: WalletMessages,
	onCache = false
): BalanceModel['status'] {
	const unreachable = unreachableLine(view, m.assets);
	if (unreachable !== undefined) return { kind: 'warning', text: unreachable, unread: true };
	if (view.unreachable) return { kind: 'warning', text: m.balance.unreachable, unread: true };
	if (onCache || view.notice === 'still_updating') {
		return { kind: 'refreshing', text: m.balance.stale };
	}
	return undefined;
}

/**
 * home-13: the person's currency could not be priced right now, so every
 * figure is in US dollars (`moneyParts`' degrade — `rate: null` is not 1).
 * That is said, never a silent swap: the hero's label already names "USD",
 * and this line says why it is not the currency they chose.
 */
function currencyStatus(currency: CurrencyView, m: WalletMessages): BalanceModel['status'] {
	if (!currency.committed || currency.rate !== null || currency.code === 'USD') return undefined;
	return { kind: 'warning', text: fill(m.balance.currencyUnavailable, { code: currency.code }) };
}

export function liveBalance(
	view: BalanceView,
	currency: CurrencyView,
	m: WalletMessages,
	/** Issue 462: the refresh control's clock and hold. Absent, no control. */
	refreshInput?: BalanceRefreshInput,
	/** Spec 105: the testnet space's words in place of a total. */
	noValue?: string
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

	// PR 3 final note F19: the first read of this account is still out
	// (`checking_key`, the core's): the status line says "Checking…" from the
	// first frame — under the skeleton and under a cached figure alike — and
	// in place of anything else that line could say. Until a round has ended
	// no chain has said the wallet is live and none has failed to answer; a
	// cached zero drew "Live · listening for payments" over a wallet nothing
	// had read, and then swapped it for "Can't reach 24 networks".
	const checkingText = view.checking_key ? m.balance.said[view.checking_key] : undefined;
	const checking = checkingText === undefined ? {} : { checkingText };

	// Hidden, the figure is masked — never what the wallet has to say about
	// it (home-8): a network it cannot reach is no secret, and the masked
	// hero under "Updated now" read as a wallet read a moment ago.
	if (view.hidden) {
		const status = checkingText === undefined ? networkStatus(view, m) : undefined;
		return {
			...base,
			...checking,
			currency: figureCurrency(currency),
			state: 'hidden',
			integer: BALANCE_MASK,
			...(status === undefined ? {} : { status })
		};
	}

	// The testnet space (spec 105): no figure at all — a "$12" of test coins
	// would read as money. Said in the figure's place; the line under it
	// still says what the wallet has to say (home-1): the network it cannot
	// reach, or that what is shown is the last read — never a bare "Updated
	// now" over a network that did not answer.
	if (noValue !== undefined) {
		const status = checkingText === undefined ? networkStatus(view, m) : undefined;
		return {
			...base,
			...checking,
			state: 'no-value',
			noValueText: noValue,
			...(status === undefined ? {} : { status })
		};
	}

	// The display currency is not the person's yet (`CurrencyView.committed`):
	// the skeleton, whatever the balance already knows. The cached total is
	// ready in milliseconds and the rate takes a round trip, so this is the
	// frame that drew "$1,234" and then jumped to "¥8,876". The label names
	// the stored choice on its way (`pending`) or no currency at all.
	if (!currency.committed) {
		return { ...base, ...checking, currency: figureCurrency(currency), state: 'loading' };
	}

	// home-11: held, and none of it can be priced — the core gives no figure
	// (`unpriced_only`): the sum is unknown, not "$0.00". Said in the figure's
	// place, and the line under it says why — once, not twice.
	if (view.unpriced_only) {
		const status =
			checkingText === undefined
				? (networkStatus(view, m) ?? { kind: 'warning' as const, text: m.balance.allUnpriced })
				: undefined;
		return {
			...base,
			...checking,
			currency: figureCurrency(currency),
			state: 'no-value',
			noValueText: m.balance.unpricedTotal,
			...(status === undefined ? {} : { status })
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
			...checking,
			currency: figureCurrency(currency),
			state: 'loading',
			// Spec 038 finding 15: a first launch with no network is
			// "unreachable" over the skeleton, never a settled-looking $0 —
			// unless what failed was Vela itself (PR 2 note 11): that is said
			// as Vela's own fault, never as the network.
			...(view.unreachable || view.internal_key
				? {
						status: {
							kind: 'warning' as const,
							unread: true as const,
							text:
								(view.internal_key ? m.assets.internal[view.internal_key] : undefined) ??
								m.balance.unreachable
						}
					}
				: {})
		};
	}

	const parts = moneyParts(total, currency);
	// Withheld: the skeleton (the early return above is this same answer,
	// said first so the label can name the currency on its way).
	if (parts === null) {
		return { ...base, ...checking, currency: figureCurrency(currency), state: 'loading' };
	}
	// "Zero, live" is the core's to say (`live_key`, F19) and nothing else
	// here decides it: the last round settled, every chain it asked answered,
	// and the wallet holds nothing. This shell used to derive it from the
	// total and the partial flag, which a cached zero satisfies before
	// anything has been read.
	const zeroLive = (view.live_key ?? null) !== null;
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
	const status: BalanceModel['status'] =
		// "Checking…" stands alone on the line (F19): a cached figure under a
		// first read is not "still updating" yet — nothing has been read at all.
		checkingText !== undefined
			? undefined
			: (networkStatus(view, m, onCache) ??
				(view.notice === 'unpriced' ? { kind: 'warning', text: m.balance.unpriced } : undefined) ??
				currencyStatus(currency, m));

	return {
		...base,
		...checking,
		currency: parts.code,
		state: zeroLive ? 'zero-live' : 'normal',
		integer: parts.integer,
		decimals: parts.decimals,
		// The mark between them is the preset's too (T480): `moneyParts`
		// grouped the integer by it, and a `.` drawn after `1.575` read as a
		// second thousands separator.
		decimalMark: numberSeparators().decimal,
		liveText: view.live_key ? m.balance.said[view.live_key] : undefined,
		status
	};
}

export function liveAssetRow(
	token: BalanceToken,
	currency: CurrencyView,
	m: WalletMessages,
	hidden: boolean,
	/** Spec 105: the testnet space — a row has no worth to show. */
	noValue = false,
	/**
	 * The row's network did not answer the last read (`unreachable_networks`):
	 * what it shows is what was last read there, and it says so — "Last seen
	 * $2.00" — rather than passing for a balance read just now.
	 */
	stale = false
): AssetRowModel {
	const art = balanceTokenMark(token);
	const worth = (): string => {
		const text = moneyText((parseFloat(token.balance) || 0) * (token.price_usd ?? 0), currency);
		return stale ? fill(m.assets.lastSeen, { amount: text }) : text;
	};
	const fiat: AssetRowModel['fiat'] = hidden
		? { kind: 'masked' }
		: noValue
			? { kind: 'none' }
			: token.price_usd === null
				? { kind: 'no-price', text: m.balance.noPrice }
				: { kind: 'value', text: worth() };
	return {
		id: balanceTokenId(token),
		ticker: token.symbol,
		chain: chainName(token.chain_id),
		badgeColor: art.badgeColor,
		logoUrls: art.logoUrls,
		badgeLogoUrl: art.badgeLogoUrl,
		badgeHidden: art.badgeHidden,
		// Hidden, the coin stays (the core's `masked_amount`, device walk of
		// 2026-10-11, item 3): "•••• BNB" says what kind of money without
		// saying how much, on this row as on a feed row and the token's own
		// page. The worth beside it is the bare mask — its unit is the
		// display currency, which every figure on the screen shares.
		balance: hidden ? maskedAmount(token.symbol) : tokenAmountText(token.balance),
		fiat,
		masked: hidden,
		...(stale ? { stale: true } : {})
	};
}

function assetsMode(view: BalanceView): SectionModel['mode'] {
	if (view.tokens.length > 0) return 'rows';
	// "Deposit your first asset" is the core's to say (`empty_key`, PR 3
	// device round) and nothing here decides it: a read has ended and found
	// nothing held. This shell used to take "no tokens, a known total" for an
	// empty wallet — which a cached total of 0 satisfies before anything has
	// been read, so it invited a first deposit under "Checking…". With no
	// tokens and no key the list is still being read: the skeleton — unless
	// the read has ended and could not reach what it asked (`unreadAssets`).
	return (view.empty_key ?? null) !== null || unreadAssets(view) ? 'empty' : 'loading';
}

/**
 * home-3 / vis-3: the list has no rows, the core gave no empty state, and the
 * read has ENDED (no "Checking…") saying why it could not list them — a
 * network it cannot reach, a fault inside Vela, or nothing read at all. A
 * skeleton there stood for ever; the list says what the hero's line says.
 */
function unreadAssets(view: BalanceView): boolean {
	return (
		view.tokens.length === 0 &&
		(view.empty_key ?? null) === null &&
		(view.checking_key ?? null) === null &&
		(view.unreachable ||
			view.unreachable_networks.length > 0 ||
			(view.internal_key ?? null) !== null)
	);
}

/** The words of {@link unreadAssets}' list: the hero line's sentence, and that nothing is lost. */
function unreadAssetsEmpty(
	view: BalanceView,
	m: WalletMessages
): { title: string; caption: string } {
	return {
		title: unreachableLine(view, m.assets) ?? m.balance.unreachable,
		caption: m.assets.unreachableBody
	};
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
	// send-6: a send that failed moved nothing. Its title says so and its
	// figure carries no "−" — never a row that reads as money that left.
	const notSent = kind === 'sent' && item.status === 'failed';
	const figure = rowFigure(item, received, m, notSent);
	// Whether this row's figure is money is the core's (`FeedItem.figure_maskable`,
	// `app::privacy`): an amount, a batch count or a capped allowance masks; an
	// unlimited allowance or a figureless signature does not.
	const masked = hidden && item.figure_maskable;
	const back = item.dapp?.received ?? null;
	const titleParts = item.dapp ? dappTitleParts(item.dapp, m) : undefined;
	const note = tokenNote(item, m);
	return {
		id: item.id,
		kind,
		title: item.dapp
			? dappTitle(item.dapp, m)
			: kind === 'received'
				? m.activity.received
				: notSent
					? m.activity.notSent
					: kind === 'sent'
						? m.activity.sent
						: m.activity.dapp,
		// Where the title may be broken (item 1 of the 2026-10-11 walk).
		...(titleParts === undefined ? {} : { titleParts }),
		// The core's second line (spec 093), worded: status first when the
		// tracker has not closed the row (RG2, 087 F04), then the site, the
		// network, or whom a transfer went to — in the core's order. A send
		// titled "Not sent" does not say "Failed" again.
		subtitle: subtitleText(
			(item.subtitle ?? []).filter((line) => !(notSent && line.type === 'status')),
			m,
			now
		),
		amount: masked ? MASK : figure.amount,
		unit: figure.unit,
		positive: received,
		masked,
		...(figure.danger ? { danger: true } : {}),
		// Round 5: whose token it was, where a symbol alone could lie.
		...(note ? { caution: `${note.text} · ${note.short}` } : {}),
		...(back === null
			? {}
			: { received: { amount: hidden ? MASK : changeFigure(back), unit: back.symbol } }),
		badgeColor: chainColor(item.chain_id),
		badgeLogoUrl: chainLogoURL(item.chain_id)
	};
}

/**
 * Round 5 (r4a-2 in Activity): the note a transfer of an unvouched token
 * wears — its record's contract (`FeedItem.token`, a folded batch's lines'
 * own), judged by the core (`token_provenance`): "Unverified token", or a
 * lookalike's "Not the real USDC". The first such line of a batch speaks for
 * it. `null` for the chain's coin, a vouched token, an older record that kept
 * no contract, and a dApp row (its lines are the simulation's).
 */
export function tokenNote(item: FeedItem, m: RowMessages): ProvenanceNote | null {
	const unverified = m.activity.tokenUnverified;
	const lookalike = m.activity.tokenLookalike;
	if (!unverified || !lookalike || item.dapp) return null;
	const words = { unverified, lookalike };
	const lines: { token?: string | null; symbol: string }[] = item.batch
		? item.batch.transfers
		: [{ token: item.token, symbol: item.symbol }];
	let note: ProvenanceNote | null = null;
	for (const line of lines) {
		const found = line.token ? provenanceNote(item.chain_id, line.token, line.symbol, words) : null;
		// A lookalike is said over a plain unverified one.
		if (found && (note === null || (found.tone === 'danger' && note.tone !== 'danger'))) {
			note = found;
		}
	}
	return note;
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
	m: RowMessages,
	/** send-6: a failed send — the figure with no sign: nothing left. */
	notSent = false
): { amount: string; unit: string; danger: boolean } {
	if (item.value !== null) {
		const about = item.dapp?.estimated ? '≈ ' : '';
		const sign = notSent ? '' : received ? '+' : '−';
		return {
			amount: `${about}${sign}${sentAmountText(item.value)}`,
			unit: item.symbol,
			danger: false
		};
	}
	const allowance = item.dapp?.allowance ?? null;
	if (allowance !== null) return allowanceFigure(allowance, m);
	if (item.dapp != null) return { amount: '', unit: '', danger: false };
	// A sweep names no one coin (send-20): its count reads "2 assets", never a
	// bare "2".
	if (item.batch?.kind === 'multi_select') {
		return {
			amount: fill(m.activity.assetsCount, { n: item.batch.count }),
			unit: '',
			danger: false
		};
	}
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
 * Whom a transfer went to or came from, short and in its EIP-55 case
 * (r4D-19): a transfer read from the chain's logs names it lowercased, and
 * one home read "To 0xd9c79C…" beside "From 0xd9c79c…" for one address.
 * As given when the core cannot say (not an address, or not loaded).
 */
export function counterpartyText(address: string): string {
	try {
		return shortenAddress(checksumAddress(address));
	} catch {
		return shortenAddress(address);
	}
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
					return fill(m.activity.toName, { name: line.name ?? counterpartyText(line.address) });
				case 'from':
					return fill(m.activity.fromName, { name: line.name ?? counterpartyText(line.address) });
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
 * The same title, in the two parts a row may break it between (device walk of
 * 2026-10-11, item 1).
 *
 * The owner's row read 「在 arcorbs.game3.app 合 / 约交互」 on the phone: the
 * title was one wrapping text, and a wrapping text breaks wherever the width
 * runs out — inside the action, in a language written without spaces. The
 * rule: a title wraps only BETWEEN what was done and where; the action is
 * never broken; the site is cut in its middle if it must be.
 *
 * The parts are read off the corpus's own template (`history.dappRowTitle`,
 * "{{intent}} on {{place}}" / 「在 {{place}} {{intent}}」 / 「{{place}}で
 * {{intent}}」), so the order and the joining words are each language's. What
 * joins them stays with the place — 「在 arcorbs.game3.app」 over 「合约交互」,
 * "Contract interaction" over "on arcorbs.game3.app" — and the white space
 * between the two parts ends the first, so the parts joined are the title
 * exactly. `undefined` when there is no place, or the template is not the two
 * placeholders this reads: the title is then drawn as one text.
 */
export function dappTitleParts(dapp: FeedDapp, m: RowMessages): ActivityTitlePart[] | undefined {
	if (dapp.place == null) return undefined;
	const template = m.activity.dappRowTitle;
	const INTENT = '{{intent}}';
	const PLACE = '{{place}}';
	const i = template.indexOf(INTENT);
	const p = template.indexOf(PLACE);
	if (i < 0 || p < 0) return undefined;
	const intent = dappVerb(dapp, m);
	const { head, tail } = middleCut(dapp.place);
	if (i < p) {
		// "<a>{{intent}}<b>{{place}}<c>": the space that opens <b> ends the action.
		const between = template.slice(i + INTENT.length, p);
		const space = /^\s*/.exec(between)?.[0] ?? '';
		return [
			{ kind: 'action', text: template.slice(0, i) + intent + space },
			{
				kind: 'place',
				before: between.slice(space.length),
				head,
				tail,
				after: template.slice(p + PLACE.length)
			}
		];
	}
	// "<a>{{place}}<b>{{intent}}<c>": <b> stays with the place, its space included.
	return [
		{
			kind: 'place',
			before: template.slice(0, p),
			head,
			tail,
			after: template.slice(p + PLACE.length, i)
		},
		{ kind: 'action', text: intent + template.slice(i + INTENT.length) }
	];
}

/**
 * Where a place gives way when its line is too short for it: `head` is cut
 * (and ends in "…"), `tail` is kept. A site's host keeps its end — the
 * registrable domain, the part that says whose site it is
 * (`app.uniswap.org.evil.xyz` must never read as `app.uniswap.org…`): its
 * last two labels, when they are short enough to leave the head something.
 * Anything else — a protocol's name — keeps its last few characters. A place
 * too short to be worth cutting in two is all head.
 */
export function middleCut(place: string): { head: string; tail: string } {
	const KEPT = 6;
	const LONGEST_KEPT = 18;
	if (place.length <= KEPT * 2) return { head: place, tail: '' };
	const labels = place.split('.');
	const host = labels.length >= 3 && !/\s/.test(place);
	const domain = host ? '.' + labels.slice(-2).join('.') : '';
	const at =
		host && domain.length <= LONGEST_KEPT ? place.length - domain.length : place.length - KEPT;
	return { head: place.slice(0, at), tail: place.slice(at) };
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
function activityMode(
	view: BalanceView,
	feed: FeedView | null | undefined,
	unread: string | undefined
): SectionModel['mode'] {
	if (!feed) return 'loading';
	if (feed.rows.length > 0) return 'rows';
	// Round 16 leftover 5: a network the scan could not read is said in the
	// empty state's place — never "No activity yet", never a skeleton for ever.
	if (unread !== undefined) return 'empty';
	// r15-1 (§23): "No activity yet" only once the core says it has looked
	// for this account (`settled`: its store read and its first scan back).
	if (!feed.settled) return 'loading';
	// vis-4: "No activity yet" is said once the wallet has finished its first
	// look (`checking_key`, the core's "Checking…"), not over a cached total
	// while the reads — a multisig's waiting proposals among them — are out.
	return view.balance_unknown || (view.checking_key ?? null) !== null ? 'loading' : 'empty';
}

/**
 * Round 16 leftover 5: the core's line for the networks the last scan could
 * not read (`FeedView.unread_key`): "Activity can't be read on {{network}}
 * right now." (the one network named) or "… on {{n}} networks …". `undefined`
 * when every network was read — or a fixture has no words for it.
 */
export function activityUnreadLine(
	feed: FeedView | null | undefined,
	m: WalletMessages
): string | undefined {
	const chains = feed?.unread_chains ?? [];
	const first = chains[0];
	// r17-4: the network by its name — the core's, else this shell's — and
	// never its raw chain id: with no name, the many-networks words.
	const name =
		feed?.unread_network ?? (first !== undefined ? namedChain(first) : undefined) ?? undefined;
	if (feed?.unread_key === 'home.activityUnreadOne' && name && m.activity.unreadOne) {
		return fill(m.activity.unreadOne, { network: name });
	}
	if (
		(feed?.unread_key === 'home.activityUnreadMany' ||
			feed?.unread_key === 'home.activityUnreadOne') &&
		m.activity.unreadMany
	) {
		return fill(m.activity.unreadMany, { n: String(chains.length) });
	}
	return undefined;
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
	// A worth that exists is masked in place when hidden, never dropped: the
	// line read "$496.46 · BNB Chain" shown and "BNB Chain" hidden, as if the
	// token had no price. One with no price has no worth to hide.
	const fiat =
		token.price_usd === null
			? undefined
			: hidden
				? MASK
				: moneyText(held * token.price_usd, currency);
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
			balance: hidden
				? maskedAmount(token.symbol)
				: `${tokenAmountText(token.balance)} ${token.symbol}`,
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
	const mine = feedFor(inputs.feed, view.address);
	const feed = mine ? narrowedFeed(mine, filter) : mine;
	// The networks the last read could not reach: their rows are last-known.
	const quiet = new Set(view.unreachable_networks.map((row) => row.chain_id));
	if (inputs.elsewhere) {
		// r13-1: nothing here is this account's — no figure, no status, no
		// empty state that would say "nothing" of a space it does not live in.
		const hero: BalanceModel = {
			label: m.balance.totalLabel,
			a11yHide: m.balance.a11yHide,
			a11yShow: m.balance.a11yShow,
			state: 'no-value',
			noValueText: inputs.elsewhere.heading
		};
		return {
			balance: hero,
			assetsMode: 'rows' as const,
			assetsEmpty: undefined,
			assetRows: tokens.map((t) => liveAssetRow(t, currency, m, view.hidden, true, false)),
			activityMode: 'rows' as const,
			activityNote: undefined as string | undefined,
			activityGroups: feed ? liveActivityGroups(feed.home_rows, m, feed.hidden) : [],
			activityEmpty: { title: m.activity.emptyTitle, caption: m.activity.emptyCaption }
		};
	}
	const history = inputs.multisigHistory;
	const historyMode =
		feed && feed.rows.length === 0 && history?.stand === 'reading' ? ('loading' as const) : null;
	const historyUnread = activitySaysHistory(inputs);
	const unread = activityUnreadLine(feed, m);
	return {
		balance: liveBalance(view, currency, m, inputs.refresh, inputs.noValue),
		// The filter's own case, not the core's empty wallet (`empty_key` is
		// about the whole account, and this one HOLDS tokens): none of them is
		// on the network picked.
		assetsMode:
			filter !== null && tokens.length === 0 && view.tokens.length > 0
				? ('empty' as const)
				: assetsMode(view),
		/** The list's words when the read could not reach what it asked (home-3). */
		assetsEmpty: unreadAssets(view) ? unreadAssetsEmpty(view, m) : undefined,
		assetRows: tokens.map((t) =>
			liveAssetRow(t, currency, m, view.hidden, inputs.noValue !== undefined, quiet.has(t.chain_id))
		),
		activityMode: historyMode ?? activityMode(view, feed, unread),
		// Round 16 leftover 5: over rows, the networks the scan could not read.
		activityNote: feed && feed.rows.length > 0 ? unread : undefined,
		// The home's Activity is the newest three (issue 469): the core's
		// `home_rows`, never `rows` — a long history pushed Assets off the
		// screen. "All" opens History, which draws every row.
		// The feed masks on its own flag (`FeedView.hidden`), never the
		// balance machine's threaded through (`app::privacy`).
		activityGroups: feed ? liveActivityGroups(feed.home_rows, m, feed.hidden) : [],
		// Spec 082 RG5: which empty line the home says is the core's
		// (`FeedView.home_empty_key`) — "no activity" or "none on this network".
		// r13-7: a multisig's history that could not be read is said as that,
		// never "No activity yet".
		activityEmpty:
			historyUnread && history
				? {
						title: history.unreadText,
						caption: '',
						...(history.retry ? { action: history.retry } : {})
					}
				: unread !== undefined
					? { title: unread, caption: '' }
					: {
							title:
								feed?.home_empty_key === 'home.emptyNoActivityNetwork'
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
		assetsSection: {
			...model.assetsSection,
			mode: live.assetsMode,
			...(live.assetsEmpty === undefined ? {} : { empty: live.assetsEmpty })
		},
		assetRows: live.assetRows,
		activitySection: {
			...model.activitySection,
			mode: live.activityMode,
			empty: live.activityEmpty,
			...(live.activityNote === undefined ? {} : { note: live.activityNote })
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
		assetsSection: {
			...model.assetsSection,
			mode: live.assetsMode,
			...(live.assetsEmpty === undefined ? {} : { empty: live.assetsEmpty })
		},
		assetRows: live.assetRows,
		activitySection: {
			...model.activitySection,
			mode: live.activityMode,
			empty: live.activityEmpty,
			...(live.activityNote === undefined ? {} : { note: live.activityNote })
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
