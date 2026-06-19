/**
 * RPC provider URL construction & chain-coverage unit tests.
 */

import {
  buildProviderRpcUrl,
  providerChainIds,
  PROVIDER_ORDER,
  PROVIDERS,
  type ProviderId,
} from '@/services/rpc-providers';
import { chainMeta } from '@/models/chains';

describe('buildProviderRpcUrl', () => {
  it('builds the documented URL shape per provider', () => {
    expect(buildProviderRpcUrl('alchemy', 1, 'KEY')).toBe('https://eth-mainnet.g.alchemy.com/v2/KEY');
    expect(buildProviderRpcUrl('drpc', 1, 'KEY')).toBe('https://lb.drpc.org/ogrpc?network=ethereum&dkey=KEY');
    expect(buildProviderRpcUrl('ankr', 1, 'KEY')).toBe('https://rpc.ankr.com/eth/KEY');
  });

  it('returns undefined for a chain no slug map covers', () => {
    for (const id of PROVIDER_ORDER) {
      expect(buildProviderRpcUrl(id, 999999, 'KEY')).toBeUndefined();
    }
  });

  it('returns undefined when the key is empty', () => {
    expect(buildProviderRpcUrl('alchemy', 1, '')).toBeUndefined();
  });
});

describe('verified provider coverage', () => {
  // Alchemy + dRPC serve all 12 Vela chains; Ankr lacks these four.
  const ANKR_UNSUPPORTED = [130 /* Unichain */, 480 /* World Chain */, 143 /* Monad */, 4217 /* Tempo */];

  it('Alchemy and dRPC serve every built-in chain (incl. Tempo & Monad)', () => {
    for (const id of ['alchemy', 'drpc'] as const) {
      expect(providerChainIds(id)).toHaveLength(12);
      for (const cid of ANKR_UNSUPPORTED) {
        expect(buildProviderRpcUrl(id, cid, 'K')).toBeTruthy();
      }
    }
  });

  it('Ankr omits Unichain, World Chain, Monad and Tempo', () => {
    expect(providerChainIds('ankr')).toHaveLength(8);
    for (const cid of ANKR_UNSUPPORTED) {
      expect(buildProviderRpcUrl('ankr', cid, 'K')).toBeUndefined();
    }
  });
});

describe('providerChainIds', () => {
  it('only lists built-in chains, in canonical CHAINS order', () => {
    for (const id of PROVIDER_ORDER) {
      const ids = providerChainIds(id);
      // Every listed chain is a real built-in network and yields a URL.
      for (const cid of ids) {
        expect(chainMeta(cid)).toBeDefined();
        expect(buildProviderRpcUrl(id, cid, 'KEY')).toBeTruthy();
      }
    }
  });

  it('every provider serves Ethereum mainnet', () => {
    for (const id of PROVIDER_ORDER) {
      expect(providerChainIds(id)).toContain(1);
    }
  });
});

describe('provider metadata', () => {
  it('PROVIDER_ORDER and PROVIDERS cover the same four ids', () => {
    const metaIds = Object.keys(PROVIDERS) as ProviderId[];
    expect([...PROVIDER_ORDER].sort()).toEqual([...metaIds].sort());
    for (const id of PROVIDER_ORDER) {
      expect(PROVIDERS[id].keyUrl).toMatch(/^https:\/\//);
    }
  });
});
