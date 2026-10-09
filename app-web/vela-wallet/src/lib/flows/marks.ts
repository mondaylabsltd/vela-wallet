/**
 * Token and network marks — every circle the wallet draws for a coin or a
 * chain, on every screen: the asset rows, the send picker, the token card,
 * the fee row and its coins, the notices, the confirm and detail facts, the
 * receive rows and the QR centre, the token screen, the share card, the
 * signing sheet's fee coins.
 *
 * Which logo a mark wears is the CORE's rule now (`remote_mark::token_mark` /
 * `chain_mark` / `chain_logo_url`), the one the phones and the desktop ask
 * too; the copies each shell kept had already drifted apart. This module only
 * hands the core the person's chain-data endpoint, as stored, and turns its
 * answer into the drawing model. The core's vectors (`tests/vectors/
 * marks.json`) are replayed through these functions in `marks.test.ts`.
 *
 * The kind rule: anything that names a NETWORK (a network row or fact, a
 * notice that locks a chain, a receive row, the QR centre, a chip) wears
 * `chainMark`; anything that names a COIN wears `tokenMarkFor`. ETH sent on
 * Base has a Base network row, never Ethereum's logo beside "Base".
 */
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { MarkView } from '$lib/core/generated/MarkView';
import {
	chainLogoUrl as coreChainLogoUrl,
	chainMark as coreChainMark,
	tokenMark as coreTokenMark
} from '$lib/core/kernels';
import { loadServiceEndpoints } from '$lib/onboarding/core/storage';
import { nativeSymbol } from '$lib/services/networks';
import { chainColor } from '$lib/wallet/fixtures';
import type { TokenMarkModel } from './model';

/**
 * The chain-data endpoint exactly as the person stored it, read at call time
 * so a re-pointed 服务端点 moves every logo with it. Blank means the built-in
 * host and a trailing slash is trimmed — both decided by the core.
 */
function ethereumDataBase(): string {
	const stored = loadServiceEndpoints().ethereumDataURL;
	return typeof stored === 'string' ? stored : '';
}

let aboard = false;

/**
 * Has the core been loaded (by `loadCore()`, or a server or test `initSync`)?
 * A list can be drawn a frame before it is — the custom networks the filter
 * lists are in storage before the wasm has arrived — and until then there is
 * nobody to ask: the mark is its glyph alone, as in the fixtures, rather than
 * a URL invented here. Once true it stays true; a real error after that is
 * not swallowed.
 */
function coreAboard(): boolean {
	if (aboard) return true;
	try {
		coreChainLogoUrl('', 1);
		aboard = true;
	} catch {
		// Not loaded yet: ask again next time.
	}
	return aboard;
}

/** A COIN's mark as the core answers it; `null` before the core is aboard. */
export function tokenMarkView(
	chainId: number,
	symbol: string,
	tokenAddress: string | null,
	named: readonly string[] = []
): MarkView | null {
	if (!coreAboard()) return null;
	return coreTokenMark(ethereumDataBase(), chainId, symbol, tokenAddress, named);
}

/** A NETWORK's mark as the core answers it; `null` before the core is aboard. */
export function chainMarkView(chainId: number, nativeTicker: string): MarkView | null {
	if (!coreAboard()) return null;
	return coreChainMark(ethereumDataBase(), chainId, nativeTicker);
}

/**
 * A chain's own logo, for the surfaces that draw a network with something
 * other than a token circle (the filter rows, the activity badge, the
 * settings network marks, the signing chip, the receive rows). None for
 * chain 0, and none before the core is aboard.
 */
export function chainLogoURL(chainId: number): string | undefined {
	if (!coreAboard()) return undefined;
	return coreChainLogoUrl(ethereumDataBase(), chainId);
}

/**
 * A coin's logo candidates, best first — what a record captures while the
 * contract address is in hand (`SendToken.logo_urls`, a received transfer's
 * row), so it still has a logo after the token has left the balance.
 */
export function tokenLogoURLs(
	chainId: number,
	symbol: string,
	tokenAddress: string | null,
	named: readonly string[] = []
): string[] {
	return tokenMarkView(chainId, symbol, tokenAddress, named)?.logo_urls ?? [];
}

/**
 * The core's answer as the drawing model. The badge dot keeps the shell's
 * colour for the chain the core names; no badge at all when it names none.
 */
export function drawnMark(view: MarkView, ticker: string, chainId: number): TokenMarkModel {
	return {
		ticker,
		badgeColor: chainColor(view.badge_chain_id ?? chainId),
		logoUrls: view.logo_urls.length > 0 ? view.logo_urls : undefined,
		badgeLogoUrl: view.badge_logo_url ?? undefined,
		badgeHidden: view.badge_chain_id === null
	};
}

/** Before the core is aboard: the glyph on the sunken disc, nothing claimed. */
function glyphOnly(ticker: string, chainId: number): TokenMarkModel {
	return { ticker, badgeColor: chainColor(chainId), badgeHidden: true };
}

/** A chain drawn as itself: its logo, no badge (`chain_mark`). */
export function chainMark(chainId: number): TokenMarkModel {
	const ticker = nativeSymbol(chainId);
	const view = chainMarkView(chainId, ticker);
	return view === null ? glyphOnly(ticker, chainId) : drawnMark(view, ticker, chainId);
}

/**
 * A coin's mark from what every token shape carries: its chain, symbol and
 * contract (`null` = the chain's native coin). `logoUrls` the API already
 * named come first; the chain-data endpoint's candidates follow, so a logo
 * the index has not catalogued still has a second chance (`token_mark`).
 */
export function tokenMarkFor(
	chainId: number,
	symbol: string,
	tokenAddress: string | null,
	logoUrls?: readonly string[]
): TokenMarkModel {
	const view = tokenMarkView(chainId, symbol, tokenAddress, logoUrls ?? []);
	return view === null ? glyphOnly(symbol, chainId) : drawnMark(view, symbol, chainId);
}

/** A held token's mark — the asset rows', on the flow screens too. */
export function balanceTokenMark(token: BalanceToken): TokenMarkModel {
	return tokenMarkFor(token.chain_id, token.symbol, token.token_address);
}
