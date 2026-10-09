/**
 * Token model — WEB (spec 025).
 *
 * The `APIToken` shape and its computed helpers, ported (trimmed) from
 * src/models/types.ts @ c13e89d4. Everything else in that file belongs to
 * other domains and stays with them. Which logo a token wears is not here:
 * that rule is the core's (`$lib/flows/marks`).
 */

import { apiNetworkToChainId } from './chains';

export interface APIToken {
	network: string;
	chainName: string;
	symbol: string;
	balance: string;
	decimals: number;
	logo: string | null;
	name: string;
	tokenAddress: string | null;
	priceUsd: number | null;
	spam: boolean;
}

export function tokenId(t: APIToken): string {
	return `${t.network}_${t.tokenAddress ?? 'native'}_${t.symbol}`;
}

export function isNativeToken(t: APIToken): boolean {
	return t.tokenAddress == null;
}

export function tokenBalanceDouble(t: APIToken): number {
	return parseFloat(t.balance) || 0;
}

export function tokenUsdValue(t: APIToken): number {
	return tokenBalanceDouble(t) * (t.priceUsd ?? 0);
}

export function tokenChainId(t: APIToken): number {
	// Inverse of networkId(); both derive from CHAINS, one table entry each way.
	return apiNetworkToChainId(t.network);
}

/** A user-added ERC-20 (`vela.customTokens` record, Expo shape verbatim). */
export interface CustomToken {
	id: string; // "{chainId}_{contractAddress}"
	chainId: number;
	contractAddress: string;
	symbol: string;
	name: string;
	decimals: number;
	networkName: string;
}

/** The core's `is_address` shape — 0x + 40 hex. */
export function isAddress(value: string): boolean {
	return /^0x[0-9a-fA-F]{40}$/.test(value);
}
