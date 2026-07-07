// Pure-logic tests for the dApp-browser grant resolution — especially the
// "never log out on a cold/empty read" rule. AsyncStorage is not exercised here.

import { resolveGranted, shouldDropGrant, type DAppGrant } from '@/services/dapp-permissions';

const grant: DAppGrant = { origin: 'https://app.uniswap.org', address: '0xAbC', chainId: 1, grantedAt: 1 };

describe('resolveGranted', () => {
  test('no grant → []', () => {
    expect(resolveGranted(null, ['0xabc'])).toEqual([]);
  });

  test('granted address still present → [address]', () => {
    expect(resolveGranted(grant, ['0xother', '0xabc'])).toEqual(['0xAbC']);
  });

  test('granted address gone from a known list → []', () => {
    expect(resolveGranted(grant, ['0xother'])).toEqual([]);
  });

  test('cold/unknown addresses → keep the grant (no logout)', () => {
    expect(resolveGranted(grant, null)).toEqual(['0xAbC']);
    expect(resolveGranted(grant, [])).toEqual(['0xAbC']);
  });

  test('address comparison is case-insensitive', () => {
    expect(resolveGranted(grant, ['0xABC'])).toEqual(['0xAbC']);
  });
});

describe('shouldDropGrant', () => {
  test('drops only when a known list no longer contains the address', () => {
    expect(shouldDropGrant(grant, ['0xother'])).toBe(true);
  });

  test('never drops on a cold/empty read', () => {
    expect(shouldDropGrant(grant, null)).toBe(false);
    expect(shouldDropGrant(grant, [])).toBe(false);
  });

  test('does not drop when the address is present', () => {
    expect(shouldDropGrant(grant, ['0xabc'])).toBe(false);
  });

  test('no grant → nothing to drop', () => {
    expect(shouldDropGrant(null, ['0xabc'])).toBe(false);
  });
});
