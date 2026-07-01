/**
 * Parallel-space service — the dev test environment wiring.
 *
 * Verifies the ONE-difference contract: entering installs the fixed-key signer and
 * swaps in the fixture wallet (so the real signing path resolves fixture keys), and
 * exiting restores the real wallet cache and removes the override. AsyncStorage is
 * mocked (node has none); the passkey module is spied so we assert the override in/out.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Passkey from '@/modules/passkey';
import {
  enterParallelSpace, exitParallelSpace, applyParallelSpaceOnBoot,
  isParallelActiveSync, fixtureStoredAccounts, fixtureAccounts,
} from '@/services/dev/parallel-space';
import { FIXTURE_ACCOUNTS } from '@/services/dev/passkey-fixture';

// The real passkey module imports react-native (ESM, unbundlable in node) — mock the
// two things it reads so the module (and its dev override seam) loads for real.
jest.mock('react-native', () => ({ NativeModules: {}, Platform: { OS: 'web' } }));

// In-memory AsyncStorage
jest.mock('@react-native-async-storage/async-storage', () => {
  const store = new Map<string, string>();
  return {
    getItem: jest.fn(async (k: string) => (store.has(k) ? store.get(k)! : null)),
    setItem: jest.fn(async (k: string, v: string) => { store.set(k, v); }),
    removeItem: jest.fn(async (k: string) => { store.delete(k); }),
    multiSet: jest.fn(async (pairs: [string, string][]) => { for (const [k, v] of pairs) store.set(k, v); }),
    multiRemove: jest.fn(async (keys: string[]) => { for (const k of keys) store.delete(k); }),
    __store: store,
  };
});

const store = (AsyncStorage as any).__store as Map<string, string>;

beforeEach(async () => {
  store.clear();
  Passkey.__setPasskeyOverride(null);
  (globalThis as any).__VELA_PARALLEL__ = false;
});

describe('parallel-space service', () => {
  it('exposes the three fixtures as wallet records with public keys', () => {
    const stored = fixtureStoredAccounts();
    expect(stored).toHaveLength(FIXTURE_ACCOUNTS.length);
    for (const a of stored) {
      expect(a.publicKeyHex.startsWith('04')).toBe(true);
      expect(a.address).toMatch(/^0x[0-9a-fA-F]{40}$/);
    }
    // The in-memory Account view drops the public key (matches wallet-state).
    expect(fixtureAccounts()[0]).not.toHaveProperty('publicKeyHex');
  });

  it('enter installs the fixed-key signer, seeds the fixture wallet, sets the flag', async () => {
    const spy = jest.spyOn(Passkey, '__setPasskeyOverride');
    expect(Passkey.__hasPasskeyOverride()).toBe(false);

    await enterParallelSpace();

    expect(spy).toHaveBeenCalled();
    expect(Passkey.__hasPasskeyOverride()).toBe(true);
    expect(isParallelActiveSync()).toBe(true);
    expect(store.get('vela.parallelSpace')).toBe('1');
    expect(store.get('dev_unlocked')).toBe('1');
    // The fixture accounts must be the ones the signing path reads (same key store).
    const accounts = JSON.parse(store.get('vela.accounts')!);
    expect(accounts.map((a: any) => a.address)).toEqual(FIXTURE_ACCOUNTS.map((a) => a.address));
    expect(store.get('vela.activeAccountIndex')).toBe('0');
  });

  it('backs up a real wallet on enter and restores it on exit', async () => {
    // A pre-existing "real" wallet cache.
    store.set('vela.accounts', JSON.stringify([{ id: 'real', name: 'Real', address: '0xreal', publicKeyHex: '04aa', createdAt: 'x' }]));
    store.set('vela.activeAccountIndex', '0');

    await enterParallelSpace();
    // Swapped to fixtures while inside.
    expect(JSON.parse(store.get('vela.accounts')!)[0].id).not.toBe('real');

    await exitParallelSpace();
    // Real wallet restored, override removed, flag cleared.
    expect(JSON.parse(store.get('vela.accounts')!)[0].id).toBe('real');
    expect(Passkey.__hasPasskeyOverride()).toBe(false);
    expect(isParallelActiveSync()).toBe(false);
    expect(store.get('vela.parallelSpace')).toBeUndefined();
  });

  it('exit clears the fixture wallet when there was no real wallet to restore', async () => {
    await enterParallelSpace();
    await exitParallelSpace();
    expect(store.get('vela.accounts')).toBeUndefined();
    expect(store.get('vela.activeAccountIndex')).toBeUndefined();
  });

  it('does not re-back-up on a second enter (idempotent backup)', async () => {
    store.set('vela.accounts', JSON.stringify([{ id: 'real' }]));
    await enterParallelSpace();
    const backup1 = store.get('vela.parallelSpace.realWalletBackup');
    await enterParallelSpace(); // already in — must not overwrite the backup with fixtures
    expect(store.get('vela.parallelSpace.realWalletBackup')).toBe(backup1);
    await exitParallelSpace();
    expect(JSON.parse(store.get('vela.accounts')!)[0].id).toBe('real');
  });

  it('applyParallelSpaceOnBoot re-arms the signer only when the flag is set', async () => {
    await applyParallelSpaceOnBoot();
    expect(Passkey.__hasPasskeyOverride()).toBe(false); // flag not set → no-op

    store.set('vela.parallelSpace', '1');
    await applyParallelSpaceOnBoot();
    expect(Passkey.__hasPasskeyOverride()).toBe(true);
    expect(isParallelActiveSync()).toBe(true);
  });

  it('the installed signer produces a Safe-valid assertion for a fixture credential', async () => {
    await enterParallelSpace();
    const assertion = await Passkey.sign('0x' + 'ab'.repeat(32), FIXTURE_ACCOUNTS[1].id);
    expect(assertion.credentialId).toBe(FIXTURE_ACCOUNTS[1].id);
    const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
    expect(verifySafeWebAuthn(assertion).ok).toBe(true);
  });
});
