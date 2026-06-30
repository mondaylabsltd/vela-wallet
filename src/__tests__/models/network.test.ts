/**
 * Tests for network module.
 */

// Mock react-native transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
import {
  chainName, nativeSymbol, networkId, DEFAULT_NETWORKS, getAllNetworksSync,
  explorerBaseURL, explorerTxURL, explorerAddressURL, explorerTokenURL,
} from '@/models/network';
import { CHAINS } from '@/models/chains';
import { tokenChainId, type APIToken } from '@/models/types';

describe('network', () => {
  describe('chainName', () => {
    test('returns correct names for all default networks', () => {
      expect(chainName(1)).toBe('Ethereum');
      expect(chainName(56)).toBe('BNB Chain');
      expect(chainName(137)).toBe('Polygon');
      expect(chainName(42161)).toBe('Arbitrum');
      expect(chainName(10)).toBe('Optimism');
      expect(chainName(8453)).toBe('Base');
      expect(chainName(43114)).toBe('Avalanche');
      expect(chainName(100)).toBe('Gnosis');
    });

    test('returns "Chain N" for unknown chain IDs', () => {
      expect(chainName(999)).toBe('Chain 999');
      expect(chainName(31337)).toBe('Chain 31337');
    });
  });

  describe('nativeSymbol', () => {
    test('returns correct symbols', () => {
      expect(nativeSymbol(1)).toBe('ETH');
      expect(nativeSymbol(56)).toBe('BNB');
      expect(nativeSymbol(137)).toBe('POL');
      expect(nativeSymbol(42161)).toBe('ETH');
      expect(nativeSymbol(10)).toBe('ETH');
      expect(nativeSymbol(8453)).toBe('ETH');
      expect(nativeSymbol(43114)).toBe('AVAX');
      expect(nativeSymbol(100)).toBe('xDAI');
    });

    test('defaults to ETH for unknown chains', () => {
      expect(nativeSymbol(999)).toBe('ETH');
    });
  });

  describe('networkId', () => {
    test('returns correct API identifiers', () => {
      expect(networkId(1)).toBe('eth-mainnet');
      expect(networkId(56)).toBe('bnb-mainnet');
      expect(networkId(137)).toBe('matic-mainnet');
      expect(networkId(42161)).toBe('arb-mainnet');
      expect(networkId(10)).toBe('opt-mainnet');
      expect(networkId(8453)).toBe('base-mainnet');
      expect(networkId(43114)).toBe('avax-mainnet');
      expect(networkId(100)).toBe('gnosis-mainnet');
    });

    test('returns chain-N for unknown', () => {
      expect(networkId(999)).toBe('chain-999');
    });
  });

  // Guards against the drift bug where adding a chain to one map but not the
  // other made tokens resolve to the wrong chain (e.g. WLD shown as Ethereum).
  describe('networkId <-> tokenChainId are inverse', () => {
    test('round-trips for every built-in chain', () => {
      for (const c of CHAINS) {
        expect(networkId(c.chainId)).toBe(c.apiNetworkId);
        expect(tokenChainId({ network: c.apiNetworkId } as APIToken)).toBe(c.chainId);
        expect(tokenChainId({ network: networkId(c.chainId) } as APIToken)).toBe(c.chainId);
      }
    });

    test('round-trips custom chain-{id} networks', () => {
      expect(networkId(31337)).toBe('chain-31337');
      expect(tokenChainId({ network: 'chain-31337' } as APIToken)).toBe(31337);
      expect(tokenChainId({ network: networkId(424242) } as APIToken)).toBe(424242);
    });

    test('unknown network id falls back to Ethereum', () => {
      expect(tokenChainId({ network: 'totally-unknown' } as APIToken)).toBe(1);
    });
  });

  describe('DEFAULT_NETWORKS derives from CHAINS', () => {
    test('one entry per CHAINS row, same order and ids', () => {
      expect(DEFAULT_NETWORKS.map(n => n.chainId)).toEqual(CHAINS.map(c => c.chainId));
      expect(DEFAULT_NETWORKS.map(n => n.id)).toEqual(CHAINS.map(c => c.id));
    });

    test('nativeSymbol matches the CHAINS table', () => {
      for (const c of CHAINS) {
        expect(nativeSymbol(c.chainId)).toBe(c.nativeSymbol);
      }
    });
  });

  describe('DEFAULT_NETWORKS', () => {
    test('has 12 default networks', () => {
      expect(DEFAULT_NETWORKS).toHaveLength(12);
    });

    test('all networks have required fields', () => {
      for (const net of DEFAULT_NETWORKS) {
        expect(net.id).toBeTruthy();
        expect(net.displayName).toBeTruthy();
        expect(net.chainId).toBeGreaterThan(0);
        expect(net.rpcURL).toMatch(/^https:\/\//);
        expect(net.explorerURL).toMatch(/^https:\/\//);
        expect(net.bundlerURL).toMatch(/^https:\/\//);
        expect(net.logoURL).toBeTruthy();
      }
    });

    test('all chain IDs are unique', () => {
      const ids = DEFAULT_NETWORKS.map(n => n.chainId);
      expect(new Set(ids).size).toBe(ids.length);
    });
  });

  describe('getAllNetworksSync', () => {
    test('returns at least default networks', () => {
      const all = getAllNetworksSync();
      expect(all.length).toBeGreaterThanOrEqual(DEFAULT_NETWORKS.length);
    });
  });

  describe('explorer links', () => {
    test('explorerBaseURL returns the chain explorer, null for unknown', () => {
      expect(explorerBaseURL(1)).toBe('https://etherscan.io');
      expect(explorerBaseURL(8453)).toBe('https://basescan.org');
      expect(explorerBaseURL(100)).toBe('https://gnosisscan.io');
      expect(explorerBaseURL(999999)).toBeNull();
    });

    test('explorerTxURL builds /tx/ links with the chain explorer', () => {
      expect(explorerTxURL(1, '0xabc')).toBe('https://etherscan.io/tx/0xabc');
      expect(explorerTxURL(8453, '0xdef')).toBe('https://basescan.org/tx/0xdef');
    });

    test('explorerAddressURL builds /address/ links', () => {
      expect(explorerAddressURL(42161, '0xaddr')).toBe('https://arbiscan.io/address/0xaddr');
    });

    test('explorerTokenURL builds /token/ links, with optional holder', () => {
      expect(explorerTokenURL(1, '0xtoken')).toBe('https://etherscan.io/token/0xtoken');
      expect(explorerTokenURL(1, '0xtoken', '0xholder')).toBe('https://etherscan.io/token/0xtoken?a=0xholder');
    });

    test('tx/address/token builders fall back to etherscan.io for unknown chains', () => {
      expect(explorerTxURL(999999, '0xabc')).toBe('https://etherscan.io/tx/0xabc');
      expect(explorerAddressURL(999999, '0xa')).toBe('https://etherscan.io/address/0xa');
      expect(explorerTokenURL(999999, '0xt')).toBe('https://etherscan.io/token/0xt');
    });
  });
});
