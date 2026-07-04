/**
 * First-launch display-currency seeding (E06 FR-7).
 *
 * Invariants under test:
 *   - a stored preference always wins — never overwritten by the seed
 *   - the seed commits (persists) ONLY after a real rate resolves; the rate-1
 *     fallback must never style a seeded balance (₫78 vs ₫2,000,000 bug)
 *   - only the primary locale's currency counts, and only when priceable
 *   - USD regions and web (null currencyCode) leave the key absent
 *
 * currency.ts keeps module-level state (_code, _seedPromise), so every case
 * loads a fresh module registry.
 */
const CHAINLINK = ['EUR', 'GBP', 'JPY', 'CNY', 'AUD', 'CAD', 'CHF', 'KRW'];

const getItem = jest.fn();
const setItem = jest.fn();
const getLocales = jest.fn();
const getChainlinkRate = jest.fn();
const getFxRate = jest.fn();

jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: (...a: unknown[]) => getItem(...a), setItem: (...a: unknown[]) => setItem(...a) },
}));
jest.mock('expo-localization', () => ({ getLocales: (...a: unknown[]) => getLocales(...a) }));
jest.mock('@/services/fiat-rates', () => ({
  getChainlinkRate: (...a: unknown[]) => getChainlinkRate(...a),
  isChainlinkFiat: (c: string) => CHAINLINK.includes(c.toUpperCase()),
  FIAT_FEED_CODES: CHAINLINK,
}));
jest.mock('@/services/fiat-fx', () => ({
  getFxRate: (...a: unknown[]) => getFxRate(...a),
  getSupportedFxCodes: jest.fn(() => Promise.resolve(['USD'])),
}));

async function loadFresh(): Promise<typeof import('@/services/currency')> {
  let mod: typeof import('@/services/currency');
  jest.resetModules();
  jest.isolateModules(() => { mod = require('@/services/currency'); });
  return mod!;
}

beforeEach(() => {
  jest.clearAllMocks();
  getItem.mockResolvedValue(null);
  setItem.mockResolvedValue(undefined);
  getLocales.mockReturnValue([]);
  getChainlinkRate.mockResolvedValue(null);
  getFxRate.mockResolvedValue(null);
});

describe('first-launch currency seeding', () => {
  it('a stored preference wins — the seed never runs over it', async () => {
    getItem.mockResolvedValue('EUR');
    getLocales.mockReturnValue([{ currencyCode: 'JPY' }]);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('EUR');
    expect(setItem).not.toHaveBeenCalled();
  });

  it('seeds and persists the region currency once a real rate resolves', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'EUR' }]);
    getChainlinkRate.mockResolvedValue(0.92);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('EUR');
    expect(setItem).toHaveBeenCalledWith('vela.displayCurrency', 'EUR');
  });

  it('stays on USD (and does not persist) when no rate source can price the seed', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'EUR' }]);
    // Chainlink and the FX endpoint both unreachable → resolveRate null.
    getChainlinkRate.mockRejectedValue(new Error('offline'));
    getFxRate.mockRejectedValue(new Error('offline'));
    const { loadCurrency, getCurrencyCode } = await loadFresh();
    expect(await loadCurrency()).toBe('USD');
    expect(getCurrencyCode()).toBe('USD');
    expect(setItem).not.toHaveBeenCalled(); // key stays absent → retried next launch
  });

  it('USD region: no seed, no persist (absent key still means "never chose")', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'USD' }]);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('USD');
    expect(setItem).not.toHaveBeenCalled();
  });

  it('unpriceable region currency (no rate from any source) is not seeded', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'XXX' }]);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('USD');
    expect(setItem).not.toHaveBeenCalled();
  });

  it('seeds a currency outside the static base when the FX endpoint prices it (VND)', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'VND' }]);
    getFxRate.mockResolvedValue(25400);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('VND');
    expect(setItem).toHaveBeenCalledWith('vela.displayCurrency', 'VND');
  });

  it('an explicit choice made while the seed rate fetch is in flight wins', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'EUR' }]);
    let resolveChainlink!: (v: number) => void;
    getChainlinkRate.mockReturnValue(new Promise((res) => { resolveChainlink = res; }));
    const { loadCurrency, setCurrency } = await loadFresh();
    const pending = loadCurrency(); // seed starts, blocks on the rate
    await new Promise((r) => setTimeout(r, 0));
    await setCurrency('JPY'); // user picks in Settings mid-flight
    getItem.mockResolvedValue('JPY');
    resolveChainlink(0.92); // seed's rate finally lands
    expect(await pending).toBe('JPY'); // the seed must NOT overwrite the choice
    expect(setItem).toHaveBeenCalledTimes(1);
    expect(setItem).toHaveBeenCalledWith('vela.displayCurrency', 'JPY');
  });

  it('only the primary locale with a region signal counts — no scanning past it', async () => {
    // en-US phone with a secondary vi-VN keyboard must NOT seed the secondary.
    getLocales.mockReturnValue([{ currencyCode: 'USD' }, { currencyCode: 'EUR' }]);
    getChainlinkRate.mockResolvedValue(0.92);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('USD');
    expect(setItem).not.toHaveBeenCalled();
  });

  it('entries without a currencyCode (web) fall through harmlessly', async () => {
    getLocales.mockReturnValue([{ currencyCode: null }]);
    const { loadCurrency } = await loadFresh();
    expect(await loadCurrency()).toBe('USD');
    expect(setItem).not.toHaveBeenCalled();
  });

  it('seeding is single-flight across concurrent callers', async () => {
    getLocales.mockReturnValue([{ currencyCode: 'EUR' }]);
    getChainlinkRate.mockResolvedValue(0.92);
    const { loadCurrency } = await loadFresh();
    const [a, b] = await Promise.all([loadCurrency(), loadCurrency()]);
    expect(a).toBe('EUR');
    expect(b).toBe('EUR');
    expect(setItem).toHaveBeenCalledTimes(1);
  });
});
