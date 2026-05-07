/**
 * API client for getvela.app endpoints.
 * Matches iOS WalletAPIService.swift.
 */
import type { APIToken, APINFT } from '@/models/types';

const BASE_URL = 'https://getvela.app/api';
const TOKEN_CACHE_TTL_MS = 5 * 60 * 1000;

type TokenCacheEntry = {
  fetchedAt: number;
  tokens: APIToken[];
  inFlight?: Promise<APIToken[]>;
};

const tokenCache = new Map<string, TokenCacheEntry>();

export type FetchTokensOptions = {
  /** Bypass the in-memory cache for explicit user refreshes. */
  forceRefresh?: boolean;
  /** Override the default cache TTL for a single call. */
  maxAgeMs?: number;
};

export class APIError extends Error {
  constructor(message = 'Failed to fetch data from server.') {
    super(message);
    this.name = 'APIError';
  }
}

function cloneTokens(tokens: APIToken[]): APIToken[] {
  return tokens.map((token) => ({ ...token }));
}

function normalizedAddress(address: string): string {
  return address.trim().toLowerCase();
}

/** Fetch token balances across all supported networks. */
export async function fetchTokens(
  address: string,
  options: FetchTokensOptions = {},
): Promise<APIToken[]> {
  const cacheKey = normalizedAddress(address);
  const maxAgeMs = options.maxAgeMs ?? TOKEN_CACHE_TTL_MS;
  const cached = tokenCache.get(cacheKey);
  const now = Date.now();

  if (!options.forceRefresh && cached) {
    if (cached.inFlight) {
      return cloneTokens(await cached.inFlight);
    }

    if (now - cached.fetchedAt < maxAgeMs) {
      return cloneTokens(cached.tokens);
    }
  }

  const request = fetchTokensFromAPI(address);
  tokenCache.set(cacheKey, {
    fetchedAt: cached?.fetchedAt ?? 0,
    tokens: cached?.tokens ?? [],
    inFlight: request,
  });

  try {
    const tokens = await request;
    tokenCache.set(cacheKey, { fetchedAt: Date.now(), tokens });
    return cloneTokens(tokens);
  } catch (error) {
    if (cached?.tokens.length) {
      tokenCache.set(cacheKey, cached);
    } else {
      tokenCache.delete(cacheKey);
    }
    throw error;
  }
}

async function fetchTokensFromAPI(address: string): Promise<APIToken[]> {
  const url = `${BASE_URL}/wallet?address=${encodeURIComponent(address)}`;
  const response = await fetch(url, { cache: 'no-store' });

  if (!response.ok) {
    throw new APIError(`/wallet failed: HTTP ${response.status}`);
  }

  const data: { tokens: APIToken[] } = await response.json();
  return data.tokens.filter(t => !t.spam);
}

export function clearTokenCache(address?: string): void {
  if (address) tokenCache.delete(normalizedAddress(address));
  else tokenCache.clear();
}

/** Fetch USD to target currency exchange rate. */
export async function fetchExchangeRate(currency = 'CNY'): Promise<number> {
  const url = `${BASE_URL}/exchange-rate?currency=${encodeURIComponent(currency)}`;
  const response = await fetch(url);

  if (!response.ok) {
    throw new APIError(`/exchange-rate failed: HTTP ${response.status}`);
  }

  const data: { currency: string; rate: number } = await response.json();
  return data.rate;
}
