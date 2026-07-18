/**
 * loadInBandFeeTokenOptions — the shared fee-asset options loader used by BOTH the Send confirm
 * slide and the dApp GasFeeCard.
 *
 * Regression pin: the selector must offer native + every whitelisted stable the Safe HOLDS,
 * read ON-CHAIN (not from the wallet's token list, whose async load once raced confirm-open and
 * silently dropped a held stable → no selector). Native-only chains / no-DEX / Tempo → null.
 *
 * Each option also carries a balance + decimals so the selector can render a per-token balance
 * row (native balance via eth_getBalance, stable decimals via resolveTokenMetadata). A held
 * stable whose decimals can't be resolved is excluded (fail closed).
 */

jest.mock('react-native', () => ({}));
jest.mock('@/models/network', () => ({ nativeSymbol: (id: number) => (id === 42161 ? 'ETH' : 'NATIVE') }));
jest.mock('@/models/types', () => ({
  nativeLogoURLs: () => ['native-logo'],
  tokenLogoURLsByAddress: (_c: number, addr: string) => [`token-logo:${addr.toLowerCase()}`],
}));

const isInBandChainMock = jest.fn();
jest.mock('@/services/bundler-service', () => ({ isInBandChain: (...a: any[]) => isInBandChainMock(...a) }));

const fetchChainTokensMock = jest.fn();
jest.mock('@/services/chain-tokens', () => ({ fetchChainTokens: (...a: any[]) => fetchChainTokensMock(...a) }));

const readErc20BalanceMock = jest.fn();
const readNativeBalanceMock = jest.fn();
jest.mock('@/services/token-reads', () => ({
  readErc20Balance: (...a: any[]) => readErc20BalanceMock(...a),
  readNativeBalance: (...a: any[]) => readNativeBalanceMock(...a),
}));

const resolveTokenMetadataMock = jest.fn();
jest.mock('@/services/token-metadata', () => ({ resolveTokenMetadata: (...a: any[]) => resolveTokenMetadataMock(...a) }));

jest.mock('@/services/tempo', () => ({ isTempoChain: (id: number) => id === 4217 || id === 42431 }));

import { loadInBandFeeTokenOptions } from '@/hooks/use-inband-fee-tokens';

const SAFE = '0x' + 'aa'.repeat(20);
const USDT = '0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9'; // Arbitrum USDT (the real one under test)
const USDC = '0xaf88d065e77c8cC2239327C5EDb3A432268e5831';
const ARB = 42161;
const NATIVE_BAL = 5n * 10n ** 18n; // 5 ETH

const arbData = {
  nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
  dex: { contracts: { quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e' } },
  wrappedNativeToken: '0x82aF49447D8a07e3bd95BD0d56f35241523fBab1',
  stables: [
    { symbol: 'USDC', contract: USDC },
    { symbol: 'USDT', contract: USDT },
  ],
};

/** Default metadata mock: resolve decimals=6 for every requested stable. */
function metaWithDecimals6() {
  resolveTokenMetadataMock.mockImplementation(async (_c: number, addrs: string[]) => {
    const m = new Map<string, { symbol: string; decimals: number }>();
    for (const a of addrs) m.set(a.toLowerCase(), { symbol: 'STABLE', decimals: 6 });
    return m;
  });
}

beforeEach(() => {
  isInBandChainMock.mockReset();
  fetchChainTokensMock.mockReset();
  readErc20BalanceMock.mockReset();
  readNativeBalanceMock.mockReset();
  resolveTokenMetadataMock.mockReset();
  readNativeBalanceMock.mockResolvedValue(NATIVE_BAL);
  metaWithDecimals6();
});

test('offers native + only the HELD stables (on-chain balance, not the wallet token list)', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  // Holds USDT, not USDC.
  readErc20BalanceMock.mockImplementation(async (_c: number, token: string) => (token === USDT ? 2_000_000n : 0n));

  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(opts).toEqual([
    { symbol: 'ETH', contract: null, balance: NATIVE_BAL, decimals: 18, logoUrls: ['native-logo'] },
    { symbol: 'USDT', contract: USDT, balance: 2_000_000n, decimals: 6, logoUrls: [`token-logo:${USDT.toLowerCase()}`] },
  ]);
});

test('the held USDT survives even when the wallet token list would not have it yet (the regression)', async () => {
  // The loader never touches a wallet token list — it reads balanceOf directly. So a held
  // stable is offered regardless of confirm-open timing.
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  readErc20BalanceMock.mockImplementation(async (_c: number, token: string) => (token === USDT ? 5n : 0n));

  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(opts!.some((o) => o.contract?.toLowerCase() === USDT.toLowerCase())).toBe(true);
});

test('no held stables → native only (still carries native balance + decimals)', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  readErc20BalanceMock.mockResolvedValue(0n);
  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(opts).toEqual([{ symbol: 'ETH', contract: null, balance: NATIVE_BAL, decimals: 18, logoUrls: ['native-logo'] }]);
});

test('a native balance read failure still offers native (balance 0), not a crash', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  readErc20BalanceMock.mockResolvedValue(0n);
  readNativeBalanceMock.mockResolvedValue(null);
  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(opts).toEqual([{ symbol: 'ETH', contract: null, balance: 0n, decimals: 18, logoUrls: ['native-logo'] }]);
});

test('a held stable whose decimals cannot be resolved is excluded (fail closed)', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  readErc20BalanceMock.mockImplementation(async (_c: number, token: string) => (token === USDT ? 7n : 0n));
  // Metadata comes back empty (RPC miss) → USDT can't be described → dropped.
  resolveTokenMetadataMock.mockResolvedValue(new Map());
  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(opts).toEqual([{ symbol: 'ETH', contract: null, balance: NATIVE_BAL, decimals: 18, logoUrls: ['native-logo'] }]);
});

test('not an in-band chain → null (no selector)', async () => {
  isInBandChainMock.mockResolvedValue(false);
  expect(await loadInBandFeeTokenOptions(ARB, SAFE)).toBeNull();
  expect(fetchChainTokensMock).not.toHaveBeenCalled();
});

test('chain has no uniswap-v3 QuoterV2 / wrapped native → null', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue({ nativeCurrency: { symbol: 'ETH', decimals: 18 }, dex: { contracts: {} }, wrappedNativeToken: null, stables: [{ symbol: 'X', contract: '0x1' }] });
  expect(await loadInBandFeeTokenOptions(ARB, SAFE)).toBeNull();
});

test('Tempo → null (fee is always pathUSD, no choice); no probe fired', async () => {
  expect(await loadInBandFeeTokenOptions(4217, SAFE)).toBeNull();
  expect(isInBandChainMock).not.toHaveBeenCalled();
});

test('a balance read failure excludes that stable (fail closed), native still offered', async () => {
  isInBandChainMock.mockResolvedValue(true);
  fetchChainTokensMock.mockResolvedValue(arbData);
  readErc20BalanceMock.mockImplementation(async (_c: number, token: string) => (token === USDT ? null : 100n));
  const opts = await loadInBandFeeTokenOptions(ARB, SAFE);
  // USDC (100) held, USDT read failed → excluded.
  expect(opts).toEqual([
    { symbol: 'ETH', contract: null, balance: NATIVE_BAL, decimals: 18, logoUrls: ['native-logo'] },
    { symbol: 'USDC', contract: USDC, balance: 100n, decimals: 6, logoUrls: [`token-logo:${USDC.toLowerCase()}`] },
  ]);
});
