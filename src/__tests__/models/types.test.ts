/**
 * Tests for data model utility functions.
 */
import {
  formatBalance,
  shortAddr,
  isAddress,
  extractAddress,
  ADDRESS_RE,
  tokenId,
  isNativeToken,
  tokenBalanceDouble,
  tokenUsdValue,
  tokenChainId,
  tokenLogoURL,
  nftDisplayName,
  nftImageURL,
  type APIToken,
  type APINFT,
} from '@/models/types';

describe('formatBalance', () => {
  test('zero → "0"', () => {
    expect(formatBalance(0)).toBe('0');
  });

  test('large values show 2 decimal places', () => {
    const result = formatBalance(1234.5678);
    expect(result).toContain('1');
    expect(result).toContain('234');
  });

  test('values >= 1 show 4 decimal places', () => {
    const result = formatBalance(1.23456);
    expect(result.includes('1.2345') || result.includes('1.2346')).toBe(true);
  });

  test('small values use significant digits', () => {
    const result = formatBalance(0.001234);
    expect(result.includes('1234') || result.includes('0.001234')).toBe(true);
  });
});

describe('shortAddr', () => {
  test('shortens long address', () => {
    const addr = '0x1234567890abcdef1234567890abcdef12345678';
    const short = shortAddr(addr);
    expect(short.length).toBeLessThan(addr.length);
    expect(short).toContain('...');
    expect(short.startsWith('0x123456')).toBe(true);
  });

  test('returns short address unchanged', () => {
    expect(shortAddr('0x1234')).toBe('0x1234');
  });
});

describe('isAddress', () => {
  const VALID = '0x1234567890abcDEF1234567890aBcdef12345678';
  test('accepts a well-formed 20-byte address (any case)', () => {
    expect(isAddress(VALID)).toBe(true);
    expect(isAddress(VALID.toLowerCase())).toBe(true);
  });
  test('rejects wrong length, missing prefix, non-hex, and nullish', () => {
    expect(isAddress('0x1234')).toBe(false); // too short
    expect(isAddress(VALID + 'ab')).toBe(false); // too long
    expect(isAddress('1234567890abcdef1234567890abcdef12345678')).toBe(false); // no 0x
    expect(isAddress('0xZZZ4567890abcdef1234567890abcdef12345678')).toBe(false); // non-hex
    expect(isAddress('')).toBe(false);
    expect(isAddress(null)).toBe(false);
    expect(isAddress(undefined)).toBe(false);
  });
  test('ADDRESS_RE is anchored (rejects embedded address)', () => {
    expect(ADDRESS_RE.test(`ethereum:${VALID}`)).toBe(false);
  });
});

describe('extractAddress', () => {
  const VALID = '0x1234567890abcdef1234567890abcdef12345678';
  test('pulls the first address out of wrapping text', () => {
    expect(extractAddress(`ethereum:${VALID}@1`)).toBe(VALID);
    expect(extractAddress(`send to ${VALID} now`)).toBe(VALID);
    expect(extractAddress(VALID)).toBe(VALID);
  });
  test('returns null when no address is present', () => {
    expect(extractAddress('no address here')).toBeNull();
    expect(extractAddress('0x1234')).toBeNull();
  });
});

describe('APIToken helpers', () => {
  const mockToken: APIToken = {
    network: 'eth-mainnet',
    chainName: 'Ethereum',
    symbol: 'ETH',
    balance: '1.5',
    decimals: 18,
    logo: null,
    name: 'Ethereum',
    tokenAddress: null,
    priceUsd: 3000,
    spam: false,
  };

  test('tokenId generates unique ID', () => {
    expect(tokenId(mockToken)).toBe('eth-mainnet_native_ETH');
  });

  test('isNativeToken detects native tokens', () => {
    expect(isNativeToken(mockToken)).toBe(true);
    expect(isNativeToken({ ...mockToken, tokenAddress: '0x123' })).toBe(false);
  });

  test('tokenBalanceDouble parses balance', () => {
    expect(tokenBalanceDouble(mockToken)).toBe(1.5);
    expect(tokenBalanceDouble({ ...mockToken, balance: 'invalid' })).toBe(0);
  });

  test('tokenUsdValue computes correctly', () => {
    expect(tokenUsdValue(mockToken)).toBe(4500); // 1.5 * 3000
    expect(tokenUsdValue({ ...mockToken, priceUsd: null })).toBe(0);
  });

  test('tokenChainId maps network identifiers', () => {
    expect(tokenChainId(mockToken)).toBe(1);
    expect(tokenChainId({ ...mockToken, network: 'arb-mainnet' })).toBe(42161);
    expect(tokenChainId({ ...mockToken, network: 'base-mainnet' })).toBe(8453);
    expect(tokenChainId({ ...mockToken, network: 'bnb-mainnet' })).toBe(56);
  });

  test('tokenLogoURL generates correct URLs', () => {
    // Native token without logo → chain logo URL
    const nativeUrl = tokenLogoURL(mockToken);
    expect(nativeUrl).toContain('eip155-1');

    // ERC-20 token → token-specific URL
    const erc20 = { ...mockToken, tokenAddress: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48' };
    const erc20Url = tokenLogoURL(erc20);
    expect(erc20Url).toContain('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48');

    // Token with existing logo
    const withLogo = { ...mockToken, logo: 'https://example.com/logo.png' };
    expect(tokenLogoURL(withLogo)).toBe('https://example.com/logo.png');
  });
});

describe('APINFT helpers', () => {
  const mockNft: APINFT = {
    network: 'eth-mainnet',
    chainName: 'Ethereum',
    contractAddress: '0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D',
    tokenId: '1234',
    name: 'Bored Ape #1234',
    description: 'A bored ape.',
    image: 'ipfs://QmRRPWG96cmgTn2qSzjwr2qvfNEuhunv6FNeMFGa9bx6mQ',
    tokenType: 'ERC721',
    collectionName: 'Bored Ape Yacht Club',
    collectionImage: null,
  };

  test('nftDisplayName uses name when available', () => {
    expect(nftDisplayName(mockNft)).toBe('Bored Ape #1234');
  });

  test('nftDisplayName falls back to collection + tokenId', () => {
    expect(nftDisplayName({ ...mockNft, name: null })).toBe('Bored Ape Yacht Club #1234');
  });

  test('nftDisplayName falls back to NFT + tokenId', () => {
    expect(nftDisplayName({ ...mockNft, name: null, collectionName: null })).toBe('NFT #1234');
  });

  test('nftImageURL converts IPFS URLs', () => {
    const url = nftImageURL(mockNft);
    expect(url).toContain('https://ipfs.io/ipfs/');
    expect(url).not.toContain('ipfs://');
  });

  test('nftImageURL returns regular URLs unchanged', () => {
    const nft = { ...mockNft, image: 'https://example.com/image.png' };
    expect(nftImageURL(nft)).toBe('https://example.com/image.png');
  });

  test('nftImageURL returns null for missing image', () => {
    expect(nftImageURL({ ...mockNft, image: null })).toBeNull();
  });
});
