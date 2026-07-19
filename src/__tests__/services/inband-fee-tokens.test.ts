/**
 * loadInBandFeeTokenOptions — the selector must use the complete address-only
 * vela_getInBandGasQuote response, not issue separate balance, metadata, chain
 * token, or price requests.
 */

jest.mock('react-native', () => ({}));
jest.mock('@/models/types', () => ({
  nativeLogoURLs: () => ['native-logo'],
  tokenLogoURLsByAddress: (_c: number, addr: string) => [`token-logo:${addr.toLowerCase()}`],
}));

const fetchInBandGasQuotesMock = jest.fn();
jest.mock('@/services/bundler-service', () => ({
  fetchInBandGasQuotes: (...a: any[]) => fetchInBandGasQuotesMock(...a),
}));
import { loadInBandFeeTokenOptions } from '@/hooks/use-inband-fee-tokens';

const SAFE = '0x' + 'aa'.repeat(20);
const USDT = '0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9';
const USDC = '0xaf88d065e77c8cC2239327C5EDb3A432268e5831';
const RECIPIENT = '0x' + '11'.repeat(20);
const ARB = 42161;

function quote(overrides: Record<string, unknown> = {}) {
  const asset = overrides.asset === 'erc20' ? 'erc20' : 'native';
  return {
    recipient: RECIPIENT,
    asset,
    feeToken: asset === 'erc20' ? USDC : null,
    balance: asset === 'erc20' ? 1_000_000n : 5n * 10n ** 18n,
    decimals: asset === 'erc20' ? 6 : 18,
    symbol: asset === 'erc20' ? 'USDC' : 'ETH',
    usdBalance: asset === 'erc20' ? '1' : '10000',
    usdPrice: asset === 'erc20' ? '1' : '2000',
    ...overrides,
  };
}

beforeEach(() => { fetchInBandGasQuotesMock.mockReset(); });

test('maps native + held stable rows from the one bundler response', async () => {
  fetchInBandGasQuotesMock.mockResolvedValue([
    quote(),
    quote({ asset: 'erc20', feeToken: USDT, symbol: 'USDT', balance: 2_000_000n, usdBalance: '2' }),
    quote({ asset: 'erc20', feeToken: USDC, symbol: 'USDC', balance: 0n, usdBalance: '0' }),
  ]);

  await expect(loadInBandFeeTokenOptions(ARB, SAFE)).resolves.toEqual([
    {
      asset: 'native', symbol: 'ETH', contract: null, balance: 5n * 10n ** 18n, decimals: 18,
      recipient: RECIPIENT, usdBalance: '10000', usdPrice: '2000',
      logoUrls: ['native-logo'],
    },
    {
      asset: 'erc20', symbol: 'USDT', contract: USDT, balance: 2_000_000n, decimals: 6,
      recipient: RECIPIENT, usdBalance: '2', usdPrice: '1',
      logoUrls: [`token-logo:${USDT.toLowerCase()}`],
    },
  ]);
  expect(fetchInBandGasQuotesMock).toHaveBeenCalledTimes(1);
  expect(fetchInBandGasQuotesMock).toHaveBeenCalledWith(ARB, SAFE);
});

test('keeps an empty native row for context but hides empty stablecoins', async () => {
  fetchInBandGasQuotesMock.mockResolvedValue([
    quote({ balance: 0n, usdBalance: '0' }),
    quote({ asset: 'erc20', balance: 0n, usdBalance: '0' }),
  ]);
  const options = await loadInBandFeeTokenOptions(ARB, SAFE);
  expect(options).toHaveLength(1);
  expect(options![0]).toMatchObject({ contract: null, balance: 0n, usdPrice: '2000' });
});

test('unavailable in-band quote means no selector', async () => {
  fetchInBandGasQuotesMock.mockResolvedValue(null);
  await expect(loadInBandFeeTokenOptions(ARB, SAFE)).resolves.toBeNull();
});

test('Tempo uses the same relay-published ERC-20 fee options as any other EVM network', async () => {
  fetchInBandGasQuotesMock.mockResolvedValue([
    quote({ asset: 'erc20', feeToken: USDC, symbol: 'pathUSD', balance: 2_000_000n, usdBalance: '2' }),
  ]);

  await expect(loadInBandFeeTokenOptions(4217, SAFE)).resolves.toEqual([
    expect.objectContaining({ asset: 'erc20', contract: USDC, symbol: 'pathUSD' }),
  ]);
  expect(fetchInBandGasQuotesMock).toHaveBeenCalledWith(4217, SAFE);
});
