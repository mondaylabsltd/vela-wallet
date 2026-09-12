/**
 * Token and chain marks for the flow screens (spec 028 Phase 9, T492) — the
 * asset rows' logo triple (`logoUrls` / `badgeLogoUrl` / `badgeHidden`),
 * built by one rule for every surface that draws a `TokenMarkModel`: the send
 * picker, the token card, the fee row and its options, the receive rows and
 * the QR centre, the token screen, the share card.
 *
 * Its own module because both `live.ts` and `live-send.ts` need it and one
 * imports the other; the rule about which logo a mark wears must not live
 * twice.
 */
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import { nativeSymbol } from '$lib/services/networks';
import {
	balanceTokenBadgeChainId,
	balanceTokenLogoURLs,
	chainLogoURL
} from '$lib/services/tokens-model';
import { chainColor } from '$lib/wallet/fixtures';
import type { TokenMarkModel } from './model';

/** A chain drawn as itself: its logo, no badge. */
export function chainMark(chainId: number): TokenMarkModel {
	const logo = chainLogoURL(chainId);
	return {
		ticker: nativeSymbol(chainId),
		badgeColor: chainColor(chainId),
		logoUrls: logo ? [logo] : undefined,
		badgeHidden: true
	};
}

/**
 * A token's mark from what every token shape carries: its chain, symbol and
 * contract (`null` = the chain's native coin). `logoUrls` the API already
 * named come first; the chain-data endpoint's candidates follow, so a logo
 * the index has not catalogued still has a second chance.
 */
export function tokenMarkFor(
	chainId: number,
	symbol: string,
	tokenAddress: string | null,
	logoUrls?: readonly string[]
): TokenMarkModel {
	const shape = { chain_id: chainId, symbol, token_address: tokenAddress };
	const badgeChain = balanceTokenBadgeChainId(shape);
	const badgeLogo = badgeChain === null ? undefined : chainLogoURL(badgeChain);
	const known = balanceTokenLogoURLs(shape);
	const named = logoUrls === undefined ? [] : logoUrls.filter((url) => url !== '');
	const candidates = [...named, ...known.filter((url) => !named.includes(url))];
	return {
		ticker: symbol,
		badgeColor: chainColor(chainId),
		logoUrls: candidates.length > 0 ? candidates : undefined,
		badgeLogoUrl: badgeLogo ? badgeLogo : undefined,
		badgeHidden: badgeChain === null
	};
}

/** A held token's mark — the asset rows' triple, on the flow screens too. */
export function balanceTokenMark(token: BalanceToken): TokenMarkModel {
	return tokenMarkFor(token.chain_id, token.symbol, token.token_address);
}
