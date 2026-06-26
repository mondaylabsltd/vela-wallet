/**
 * Single source of truth for well-known ERC-20 static metadata (symbol +
 * decimals), keyed by lowercased address.
 *
 * Previously this data was duplicated across three places — clear-signing's
 * KNOWN_DECIMALS + KNOWN_SYMBOLS and SigningRequestModal's own guessTokenSymbol —
 * which drift independently. Consolidated here so a token is described once.
 * On-chain resolution for everything else lives in token-metadata.ts (Multicall3);
 * it consults this table first to skip the RPC for the common tokens.
 */

export interface KnownToken {
  symbol: string;
  decimals: number;
}

/** Well-known tokens (lowercased address → symbol + decimals). */
export const KNOWN_TOKENS: Record<string, KnownToken> = {
  // ---- Ethereum mainnet ----
  '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': { symbol: 'USDC', decimals: 6 },
  '0xdac17f958d2ee523a2206206994597c13d831ec7': { symbol: 'USDT', decimals: 6 },
  '0x6b175474e89094c44da98b954eedeac495271d0f': { symbol: 'DAI', decimals: 18 },
  '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2': { symbol: 'WETH', decimals: 18 },
  '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599': { symbol: 'WBTC', decimals: 8 },
  '0x514910771af9ca656af840dff83e8264ecf986ca': { symbol: 'LINK', decimals: 18 },
  '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984': { symbol: 'UNI', decimals: 18 },
  '0xae7ab96520de3a18e5e111b5eaab095312d7fe84': { symbol: 'stETH', decimals: 18 },
  '0xbe9895146f7af43049ca1c1ae358b0541ea49704': { symbol: 'cbETH', decimals: 18 },
  '0xae78736cd615f374d3085123a210448e74fc6393': { symbol: 'rETH', decimals: 18 },
  '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0': { symbol: 'wstETH', decimals: 18 },
  '0x5a98fcbea516cf06857215779fd812ca3bef1b32': { symbol: 'LDO', decimals: 18 },
  '0xd533a949740bb3306d119cc777fa900ba034cd52': { symbol: 'CRV', decimals: 18 },
  '0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9': { symbol: 'AAVE', decimals: 18 },
  '0xc00e94cb662c3520282e6f5717214004a7f26888': { symbol: 'COMP', decimals: 18 },
  '0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2': { symbol: 'MKR', decimals: 18 },
  // ---- Polygon ----
  '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359': { symbol: 'USDC', decimals: 6 },
  '0x2791bca1f2de4661ed88a30c99a7a9449aa84174': { symbol: 'USDC.e', decimals: 6 },
  // ---- Arbitrum ----
  '0xaf88d065e77c8cc2239327c5edb3a432268e5831': { symbol: 'USDC', decimals: 6 },
  '0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9': { symbol: 'USDT', decimals: 6 },
};

export function knownToken(addr: string | undefined): KnownToken | undefined {
  return addr ? KNOWN_TOKENS[addr.toLowerCase()] : undefined;
}

export function knownTokenSymbol(addr: string | undefined): string | undefined {
  return knownToken(addr)?.symbol;
}

export function knownTokenDecimals(addr: string | undefined): number | undefined {
  return knownToken(addr)?.decimals;
}
