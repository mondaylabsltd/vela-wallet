/**
 * Integration tests for recipient identity resolution.
 *
 * These tests query real RPCs and name service registries to verify
 * that reverse resolution works correctly for known addresses.
 *
 * Run with: npx jest -- src/__tests__/services/recipient-identity.test.ts
 */

// Mock react-native transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(() => Promise.resolve(null)),
    setItem: jest.fn(() => Promise.resolve()),
  },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(), save: jest.fn(), remove: jest.fn(), syncNow: jest.fn(),
}));

// Mock the passkey index to avoid hitting that API
jest.mock('@/services/public-key-index', () => ({
  queryByWalletRef: jest.fn(() => Promise.resolve(null)),
}));

import { resolveRecipientIdentity } from '@/services/recipient-identity';

// These tests hit real RPCs — increase timeout
jest.setTimeout(30_000);

describe('namehash', () => {
  // We test namehash indirectly through the resolver calls.
  // But let's also import and test it directly.
  // namehash is not exported, so we verify via known reverse resolutions.
});

describe('resolveRecipientIdentity', () => {
  it('returns null for invalid address', async () => {
    expect(await resolveRecipientIdentity('not-an-address')).toBeNull();
    expect(await resolveRecipientIdentity('0x123')).toBeNull();
  });

  // .bnb: spaceid.bnb → 0xb5932a6B7d50A966AEC6C74C97385412Fb497540
  it('resolves spaceid.bnb via .bnb registry', async () => {
    const result = await resolveRecipientIdentity('0xb5932a6B7d50A966AEC6C74C97385412Fb497540');
    expect(result).not.toBeNull();
    expect(result!.name).toBe('spaceid.bnb');
    expect(result!.source).toBe('.bnb');
  });

  // .arb: ape.arb → 0x5929B404b43e49a2EBBD6afDe45294598d4fdD29
  it('resolves ape.arb via .arb registry (Arbitrum)', async () => {
    const result = await resolveRecipientIdentity('0x5929B404b43e49a2EBBD6afDe45294598d4fdD29');
    expect(result).not.toBeNull();
    expect(result!.name).toBe('ape.arb');
    expect(result!.source).toBe('.arb');
  });

  // .g: second.g → 0x1C4e5b02e73b12f374744f6dc1c8469ec9EcD62E
  it('resolves second.g via .g registry (Gravity)', async () => {
    const result = await resolveRecipientIdentity('0x1C4e5b02e73b12f374744f6dc1c8469ec9EcD62E');
    expect(result).not.toBeNull();
    expect(result!.name).toBe('second.g');
    expect(result!.source).toBe('.g');
  });

  // Basename (ENSIP-19): sendora.base.eth → 0x3Fb9266232E90A8Ca088Fd292c7435b2ed62b831
  it('resolves sendora.base.eth via Basename (ENSIP-19)', async () => {
    const result = await resolveRecipientIdentity('0x3Fb9266232E90A8Ca088Fd292c7435b2ed62b831');
    expect(result).not.toBeNull();
    expect(result!.name).toBe('sendora.base.eth');
    expect(result!.source).toBe('Basename');
  });

  // ENS: vitalik.eth → 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
  it('resolves vitalik.eth via ENS', async () => {
    const result = await resolveRecipientIdentity('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045');
    expect(result).not.toBeNull();
    expect(result!.name).toBe('vitalik.eth');
    expect(result!.source).toBe('ENS');
  });

  // Address with no name set should return null
  it('returns null for address with no reverse record', async () => {
    const result = await resolveRecipientIdentity('0x0000000000000000000000000000000000000001');
    expect(result).toBeNull();
  });
});
