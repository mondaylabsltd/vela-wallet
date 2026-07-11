/**
 * Tests for autoAddReceivedTokens — silent, confirm-time token listing.
 * Focus: it adds ONLY net-received ERC-20s, skips held/known/already-listed, and
 * invalidates the token cache so the new token surfaces on the next sync.
 */
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true, default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn(), multiGet: jest.fn() },
}));

const deriveAssetDeltasMock = jest.fn();
jest.mock('@/services/sim-assets', () => ({ deriveAssetDeltas: (...a: any[]) => deriveAssetDeltasMock(...a) }));

const loadCustomTokensMock = jest.fn();
const saveCustomTokenMock = jest.fn();
jest.mock('@/services/storage', () => ({
  loadCustomTokens: (...a: any[]) => loadCustomTokensMock(...a),
  saveCustomToken: (...a: any[]) => saveCustomTokenMock(...a),
}));

const clearTokenCacheMock = jest.fn();
const getCachedHeldTokensMock = jest.fn();
jest.mock('@/services/wallet-api', () => ({
  clearTokenCache: (...a: any[]) => clearTokenCacheMock(...a),
  getCachedHeldTokens: (...a: any[]) => getCachedHeldTokensMock(...a),
}));

const resolveTokenMetadataMock = jest.fn();
jest.mock('@/services/token-metadata', () => ({ resolveTokenMetadata: (...a: any[]) => resolveTokenMetadataMock(...a) }));

const knownTokenSymbolMock = jest.fn();
jest.mock('@/services/tokens', () => ({ knownTokenSymbol: (...a: any[]) => knownTokenSymbolMock(...a) }));

jest.mock('@/models/network', () => ({ chainName: () => 'BNB Chain' }));

import { autoAddReceivedTokens } from '@/services/token-autoadd';

const FROM = '0xUser0000000000000000000000000000000000001';
const CAKE = '0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82';
const LOGS = [{ address: CAKE, topics: ['0xt'], data: '0x' }]; // shape only; deltas are mocked

beforeEach(() => {
  jest.clearAllMocks();
  loadCustomTokensMock.mockResolvedValue([]);
  getCachedHeldTokensMock.mockReturnValue([]);
  knownTokenSymbolMock.mockReturnValue(undefined);
  resolveTokenMetadataMock.mockResolvedValue(new Map([[CAKE, { symbol: 'CAKE', decimals: 18 }]]));
  saveCustomTokenMock.mockResolvedValue(undefined);
});

test('adds a net-received ERC-20 and invalidates the token cache', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: 813285000000000000n }]);
  const added = await autoAddReceivedTokens(FROM, 56, LOGS);
  expect(added).toBe(1);
  expect(saveCustomTokenMock).toHaveBeenCalledWith(expect.objectContaining({
    id: `56_${CAKE}`, chainId: 56, contractAddress: CAKE, symbol: 'CAKE', decimals: 18, networkName: 'BNB Chain',
  }));
  expect(clearTokenCacheMock).toHaveBeenCalledWith(FROM);
});

test('ignores a net-SENT token (negative delta) — a pure send adds nothing', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: -5n }]);
  const added = await autoAddReceivedTokens(FROM, 56, LOGS);
  expect(added).toBe(0);
  expect(saveCustomTokenMock).not.toHaveBeenCalled();
  expect(clearTokenCacheMock).not.toHaveBeenCalled();
});

test('skips a token already held', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: 1n }]);
  getCachedHeldTokensMock.mockReturnValue([CAKE.toUpperCase()]); // case-insensitive
  expect(await autoAddReceivedTokens(FROM, 56, LOGS)).toBe(0);
  expect(saveCustomTokenMock).not.toHaveBeenCalled();
});

test('skips a curated known token (stablecoin etc.)', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: 1n }]);
  knownTokenSymbolMock.mockReturnValue('USDC');
  expect(await autoAddReceivedTokens(FROM, 56, LOGS)).toBe(0);
});

test('skips a token already in the custom list', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: 1n }]);
  loadCustomTokensMock.mockResolvedValue([{ chainId: 56, contractAddress: CAKE.toUpperCase() }]);
  expect(await autoAddReceivedTokens(FROM, 56, LOGS)).toBe(0);
});

test('skips a received token whose symbol cannot be resolved (no "?" token)', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'erc20', token: CAKE, delta: 1n }]);
  resolveTokenMetadataMock.mockResolvedValue(new Map()); // unresolved
  expect(await autoAddReceivedTokens(FROM, 56, LOGS)).toBe(0);
  expect(saveCustomTokenMock).not.toHaveBeenCalled();
});

test('ignores native deltas (only ERC-20s are listed)', async () => {
  deriveAssetDeltasMock.mockReturnValue([{ kind: 'native', token: undefined, delta: 999n }]);
  expect(await autoAddReceivedTokens(FROM, 56, LOGS)).toBe(0);
});

test('no-ops on empty logs / missing from', async () => {
  expect(await autoAddReceivedTokens(FROM, 56, [])).toBe(0);
  expect(await autoAddReceivedTokens(undefined, 56, LOGS)).toBe(0);
  expect(deriveAssetDeltasMock).not.toHaveBeenCalled();
});
