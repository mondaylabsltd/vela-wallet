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
import { moneyText, trimBalance } from '$lib/wallet/live';

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
			coin: `${trimBalance(units.toString(), 4)} ${symbol}`.trim(),
			symbol,
			units: Number.isFinite(units) ? units : null,
			contract: asset.token
		};
	}
	const units = Number(fee.total_wei) / 1e18;
	return {
		coin: `${trimBalance(units.toString(), 6)} ${symbol}`.trim(),
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

/** "0.0021 ETH · ≈$0.55" — the coin, and what it costs when anything can say. */
export function feeLine(parts: FeeParts, priceUsd: number | null, currency: CurrencyView): string {
	if (parts.units === null || priceUsd === null) return parts.coin;
	const usd = parts.units * priceUsd;
	if (usd < FEE_FIAT_MIN_USD) return parts.coin;
	return `${parts.coin} · ≈${moneyText(usd, currency)}`;
}
