/**
 * What a network fee READS, for every surface that prices one operation
 * (issue 201).
 *
 * The send screens and the dApp signing sheet quote the same fee session, and
 * the design sheet says so in as many words ("GasFeeCard — shared Send ↔
 * signing; the two surfaces must not drift"). Two formatters would be two
 * answers about what a transaction costs, so there is one, here — the same
 * reason `marks.ts` exists for logos.
 *
 * The amount is always the quote's own. Only the PRICE is looked up, and when
 * nothing can price the fee coin the line stays the coin amount alone: a fee
 * row that invents a figure is worse than one that shows only the coin.
 */
import type { CurrencyView } from '$lib/core/generated/CurrencyView';
import type { FeeEstimateView } from '$lib/core/generated/FeeEstimateView';
import type { FeeView } from '$lib/core/generated/FeeView';
import { nativeSymbol } from '$lib/services/networks';
import { feeFigure } from '$lib/core/kernels';
import { moneyText } from '$lib/wallet/live';
import { showsFiat } from '$lib/network-space/fiat';
import type { NetworkSpace } from '$lib/network-space/network-space.svelte';

/**
 * Below this the token amount is the honest primary and the fiat half is left
 * off (`03-domain-components.md` §3.1): a real fee rounded to "$0.00" reads as
 * free, which is a worse answer than no figure at all.
 */
export const FEE_FIAT_MIN_USD = 0.005;

/** A quote split into what a row needs: its words, its coin, what prices it. */
export interface FeeParts {
	/** "0.0021 ETH" — the coin amount, trimmed as that coin is trimmed. */
	coin: string;
	symbol: string;
	/** The whole-token figure a price multiplies; `null` when it will not parse. */
	units: number | null;
	/** The ERC-20 paying, or `null` for the chain's own coin. */
	contract: string | null;
}

/**
 * A fee amount as text — the ONE formatter for "how much coin is this fee",
 * wherever it is printed: the fee row, the fee-coin sheet, the signing sheet's
 * fee selector, a multisig execution's fee.
 *
 * r21-2 (§36): the figure is the CORE's (`fee_policy::fee_figure`, wasm
 * `feeFigure`) — `units` base units of a `decimals` coin at `shown` places,
 * rounded UP, so the figure on screen is never below what is charged; a fee
 * under the last place keeps two significant digits (issue 682: never a bare
 * "0" that reads as free). It used to be cut DOWN here through a float, so a
 * fee of 0.0000836 read "0.000083" — less than the charge. Exact, from the
 * base units as text: no float ever touches the figure. `—` for a count the
 * core cannot read.
 */
export function feeAmountText(units: string | bigint, decimals: number, shown: number): string {
	let figure: string | null;
	try {
		figure = feeFigure(units, decimals, shown);
	} catch {
		figure = null;
	}
	return figure ?? '—';
}

/** The fee places a row shows: 6 for the chain's own coin, 4 for a token (as before). */
export const NATIVE_FEE_PLACES = 6;
export const TOKEN_FEE_PLACES = 4;

/**
 * The ticker a quote is denominated in.
 *
 * NEVER the chain's native symbol for an ERC-20 fee: that is a different coin,
 * and naming it there is how a USDC fee came to read "0.000392 ETH" on the row
 * behind the sheet that had just ticked USDC. The quote carries its own symbol;
 * the relay's published row is the fallback when an older one does not.
 */
export function feeSymbol(fee: FeeEstimateView, options: FeeView['options']): string {
	if (fee.fee_asset.type !== 'erc20') return nativeSymbol(fee.chain_id);
	const contract = fee.fee_asset.token;
	return (
		fee.fee_asset.symbol ??
		options.find((option) => option.contract?.toLowerCase() === contract.toLowerCase())?.symbol ??
		''
	);
}

/**
 * The estimate as its parts.
 *
 * `total_wei` is the NATIVE figure even when an ERC-20 pays; reading it where
 * the token's own `amount` belongs prints a six-decimal stablecoin fee as an
 * eighteen-decimal number, under a ticker that is not the coin being spent.
 */
export function feeParts(fee: FeeEstimateView, options: FeeView['options']): FeeParts {
	const asset = fee.fee_asset;
	const symbol = feeSymbol(fee, options);
	if (asset.type === 'erc20') {
		const units = Number(asset.amount) / 10 ** asset.decimals;
		return {
			coin: `${feeAmountText(asset.amount, asset.decimals, TOKEN_FEE_PLACES)} ${symbol}`.trim(),
			symbol,
			units: Number.isFinite(units) ? units : null,
			contract: asset.token
		};
	}
	const units = Number(fee.total_wei) / 1e18;
	return {
		coin: `${feeAmountText(fee.total_wei, 18, NATIVE_FEE_PLACES)} ${symbol}`.trim(),
		symbol,
		units: Number.isFinite(units) ? units : null,
		contract: null
	};
}

/** The price of the coin ONE published quote row is charged in. */
export function feeOptionPriceUsd(
	contract: string | null,
	options: FeeView['options']
): number | null {
	const same = (other: string | null) =>
		contract === null ? other === null : other?.toLowerCase() === contract.toLowerCase();
	const published = options.find((option) => same(option.contract))?.usd_price;
	const price = published == null ? Number.NaN : Number(published);
	return Number.isFinite(price) && price > 0 ? price : null;
}

/**
 * The same two facts, kept apart: the coin, and what it costs in money — or
 * `null` when nothing can say, or the sum would round to a printed zero.
 *
 * The send form's fee row draws them as two pieces (issue 231) so a narrow row
 * drops the money onto its own line whole instead of breaking mid-label. The
 * fiat half is written "≈ $0.55", spaced like the amount's own "≈" line above
 * it on that form.
 */
export function feeLineParts(
	parts: FeeParts,
	priceUsd: number | null,
	currency: CurrencyView,
	space?: NetworkSpace
): { coin: string; fiat: string | null } {
	const money = feeMoney(parts, priceUsd, currency, space);
	return { coin: parts.coin, fiat: money === null ? null : `≈ ${money}` };
}

/** "0.0021 ETH · ≈$0.55" — the coin, and what it costs when anything can say. */
export function feeLine(
	parts: FeeParts,
	priceUsd: number | null,
	currency: CurrencyView,
	space?: NetworkSpace
): string {
	const money = feeMoney(parts, priceUsd, currency, space);
	return money === null ? parts.coin : `${parts.coin} · ≈${money}`;
}

/**
 * The ONE decision both shapes above share: is there a money figure to show.
 * r18-9: never in the testnet space (`showsFiat`) — a test coin's fee has no
 * real value, whatever price a relay published for it.
 */
function feeMoney(
	parts: FeeParts,
	priceUsd: number | null,
	currency: CurrencyView,
	space?: NetworkSpace
): string | null {
	if (!showsFiat(space)) return null;
	if (parts.units === null || priceUsd === null) return null;
	const usd = parts.units * priceUsd;
	if (usd < FEE_FIAT_MIN_USD) return null;
	return moneyText(usd, currency);
}
