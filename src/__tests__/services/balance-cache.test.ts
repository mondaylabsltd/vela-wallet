/**
 * Balance cache unit tests.
 */

// Mock AsyncStorage
const mockStorage = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
  setItem: jest.fn(async (key: string, val: string) => { mockStorage.set(key, val); }),
}));

// Need to reset module state between tests since balance-cache has module-level state
let balanceCache: typeof import('@/services/balance-cache');

beforeEach(() => {
  mockStorage.clear();
  jest.resetModules();
  balanceCache = require('@/services/balance-cache');
});

describe('balance-cache', () => {
  it('stores and retrieves a balance', async () => {
    await balanceCache.setAccountBalance('0xAABB', 1234.56);
    const result = await balanceCache.getAccountBalance('0xAABB');
    expect(result).toBe(1234.56);
  });

  it('normalizes addresses to lowercase', async () => {
    await balanceCache.setAccountBalance('0xAABBCC', 100);
    const result = await balanceCache.getAccountBalance('0xaabbcc');
    expect(result).toBe(100);
  });

  it('returns null for unknown address', async () => {
    const result = await balanceCache.getAccountBalance('0x1234');
    expect(result).toBeNull();
  });

  it('returns null for expired entry', async () => {
    // Set a balance with a fake old timestamp
    await balanceCache.setAccountBalance('0xAA', 50);

    // Manipulate time — the TTL is 24 hours
    const origNow = Date.now;
    Date.now = () => origNow() + 25 * 60 * 60 * 1000; // 25 hours later

    const result = await balanceCache.getAccountBalance('0xAA');
    expect(result).toBeNull();

    Date.now = origNow;
  });

  it('getAccountBalances returns map of valid entries', async () => {
    await balanceCache.setAccountBalance('0xA1', 100);
    await balanceCache.setAccountBalance('0xA2', 200);

    const result = await balanceCache.getAccountBalances(['0xA1', '0xA2', '0xA3']);
    expect(result.size).toBe(2);
    expect(result.get('0xA1')).toBe(100);
    expect(result.get('0xA2')).toBe(200);
    expect(result.has('0xA3')).toBe(false);
  });

  it('getAccountBalances skips expired entries', async () => {
    await balanceCache.setAccountBalance('0xB1', 300);

    const origNow = Date.now;
    Date.now = () => origNow() + 25 * 60 * 60 * 1000;

    const result = await balanceCache.getAccountBalances(['0xB1']);
    expect(result.size).toBe(0);

    Date.now = origNow;
  });

  it('persists to AsyncStorage', async () => {
    await balanceCache.setAccountBalance('0xCC', 999);
    // Give fire-and-forget persist a moment
    await new Promise(r => setTimeout(r, 50));

    const stored = mockStorage.get('vela.balanceCache');
    expect(stored).toBeTruthy();
    const parsed = JSON.parse(stored!);
    expect(parsed['0xcc']).toBeTruthy();
    expect(parsed['0xcc'].usd).toBe(999);
  });

  it('hydrates from AsyncStorage on first access', async () => {
    // Pre-populate storage
    mockStorage.set('vela.balanceCache', JSON.stringify({
      '0xdd': { usd: 555, at: Date.now() },
    }));

    // Reset module to simulate fresh start
    jest.resetModules();
    balanceCache = require('@/services/balance-cache');

    const result = await balanceCache.getAccountBalance('0xDD');
    expect(result).toBe(555);
  });

  it('handles corrupt storage data gracefully', async () => {
    mockStorage.set('vela.balanceCache', 'not-json!!!');

    jest.resetModules();
    balanceCache = require('@/services/balance-cache');

    // Should not throw, just return null
    const result = await balanceCache.getAccountBalance('0x00');
    expect(result).toBeNull();
  });

  it('overwrites existing balance', async () => {
    await balanceCache.setAccountBalance('0xEE', 100);
    await balanceCache.setAccountBalance('0xEE', 200);
    const result = await balanceCache.getAccountBalance('0xEE');
    expect(result).toBe(200);
  });
});
