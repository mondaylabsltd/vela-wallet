/**
 * Chain token discovery unit tests.
 */

jest.mock('@/services/storage', () => ({
  getEthereumDataURL: () => 'https://ethereum-data.awesometools.dev',
}));

const mockResponses = new Map<string, any>();
const originalFetch = global.fetch;

beforeAll(() => {
  global.fetch = jest.fn(async (url: string) => {
    const resp = mockResponses.get(url);
    if (resp) return { ok: true, json: async () => resp } as Response;
    return { ok: false } as Response;
  }) as any;
});

afterAll(() => { global.fetch = originalFetch; });

// Reset module-level cache between tests
let chainTokens: typeof import('@/services/chain-tokens');

beforeEach(() => {
  mockResponses.clear();
  jest.resetModules();
  chainTokens = require('@/services/chain-tokens');
});

describe('pickQuoteToken', () => {
  it('prefers native USDC', () => {
    const stables = [
      { symbol: 'USDT', type: 'native', contract: '0x1' },
      { symbol: 'USDC', type: 'bridge', contract: '0x2' },
      { symbol: 'USDC', type: 'native', contract: '0x3' },
    ];
    expect(chainTokens.pickQuoteToken(stables)?.contract).toBe('0x3');
  });

  it('falls back to any USDC if no native', () => {
    const stables = [
      { symbol: 'USDT', type: 'native', contract: '0x1' },
      { symbol: 'USDC', type: 'bridge', contract: '0x2' },
    ];
    expect(chainTokens.pickQuoteToken(stables)?.contract).toBe('0x2');
  });

  it('falls back to USDT if no USDC', () => {
    const stables = [
      { symbol: 'DAI', type: 'native', contract: '0x1' },
      { symbol: 'USDT', type: 'native', contract: '0x2' },
    ];
    expect(chainTokens.pickQuoteToken(stables)?.contract).toBe('0x2');
  });

  it('falls back to first stable if no USDC/USDT', () => {
    const stables = [
      { symbol: 'DAI', type: 'native', contract: '0x1' },
    ];
    expect(chainTokens.pickQuoteToken(stables)?.contract).toBe('0x1');
  });

  it('returns null for empty list', () => {
    expect(chainTokens.pickQuoteToken([])).toBeNull();
  });
});

describe('fetchChainTokens', () => {
  it('fetches and parses chain data', async () => {
    mockResponses.set('https://ethereum-data.awesometools.dev/chains/eip155-1.json', {
      nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
      stables: [{ symbol: 'USDC', type: 'native', contract: '0xA0b8' }],
      wrappedNativeToken: '0xC02a',
    });

    const result = await chainTokens.fetchChainTokens(1);
    expect(result).not.toBeNull();
    expect(result!.chainId).toBe(1);
    expect(result!.nativeCurrency.symbol).toBe('ETH');
    expect(result!.stables).toHaveLength(1);
    expect(result!.wrappedNativeToken).toBe('0xC02a');
    // Ethereum has built-in DEX override
    expect(result!.dex).not.toBeNull();
    expect(result!.dex!.dex).toBe('Uniswap');
  });

  it('returns null for unknown chain', async () => {
    const result = await chainTokens.fetchChainTokens(99999);
    expect(result).toBeNull();
  });

  it('uses built-in DEX override over API data', async () => {
    mockResponses.set('https://ethereum-data.awesometools.dev/chains/eip155-8453.json', {
      nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
      stables: [],
      dex: { dex: 'SomeDex', protocol: 'unknown', contracts: {} },
    });

    const result = await chainTokens.fetchChainTokens(8453);
    expect(result).not.toBeNull();
    // Base has Aerodrome built-in, should override API's "SomeDex"
    expect(result!.dex!.dex).toBe('Aerodrome');
  });

  it('caches results', async () => {
    mockResponses.set('https://ethereum-data.awesometools.dev/chains/eip155-137.json', {
      nativeCurrency: { name: 'POL', symbol: 'POL', decimals: 18 },
      stables: [],
    });

    const r1 = await chainTokens.fetchChainTokens(137);
    const r2 = await chainTokens.fetchChainTokens(137);
    expect(r1).toBe(r2); // same object reference = cached
    expect(global.fetch).toHaveBeenCalledTimes(1); // only 1 fetch
  });

  it('handles missing nativeCurrency gracefully', async () => {
    mockResponses.set('https://ethereum-data.awesometools.dev/chains/eip155-42161.json', {
      stables: [],
    });

    const result = await chainTokens.fetchChainTokens(42161);
    expect(result).not.toBeNull();
    expect(result!.nativeCurrency.name).toBe('Ether');
    expect(result!.nativeCurrency.symbol).toBe('ETH');
    expect(result!.nativeCurrency.decimals).toBe(18);
  });

  it('validates decimals range', async () => {
    mockResponses.set('https://ethereum-data.awesometools.dev/chains/eip155-10.json', {
      nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 999 }, // invalid
      stables: [],
    });

    const result = await chainTokens.fetchChainTokens(10);
    expect(result).not.toBeNull();
    expect(result!.nativeCurrency.decimals).toBe(18); // falls back to 18
  });
});
