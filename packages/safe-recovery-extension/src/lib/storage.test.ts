import { afterEach, describe, expect, it, vi } from 'vitest';
import { STORAGE_KEY } from './constants';
import {
  bindCredentialIdToSafe,
  clearCredentialIdForSafe,
  credentialBindingKey,
  credentialIdForSafe,
  getSettings,
  toPublicState,
} from './storage';
import type { RecoverySettings } from './types';

const SAFE_A = '0x000000000000000000000000000000000000000a';
const SAFE_B = '0x000000000000000000000000000000000000000b';

function settings(credentialIds: Record<string, string>): RecoverySettings {
  return {
    enabled: true,
    rpId: 'getvela.app',
    chainId: 100,
    rpcUrls: {},
    chainNames: {},
    credentialIds,
    relayerPrivateKey: `0x${'11'.repeat(32)}`,
    localConfirmations: {},
  };
}

function installStorage(stored: Partial<RecoverySettings>) {
  let saved: RecoverySettings | undefined;
  const set = vi.fn().mockImplementation(async (value: Record<string, RecoverySettings>) => {
    saved = value[STORAGE_KEY];
  });
  vi.stubGlobal('chrome', {
    storage: {
      local: {
        get: vi.fn().mockResolvedValue({ [STORAGE_KEY]: stored }),
        set,
      },
    },
  });
  return { set, saved: () => saved };
}

describe('Safe-address passkey bindings', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('normalizes Safe address casing', () => {
    expect(credentialBindingKey('0x00000000000000000000000000000000000000Ab'))
      .toBe('0x00000000000000000000000000000000000000ab');
  });

  it('ignores chain and RP changes for the same Safe address', () => {
    const recovery = settings({ [credentialBindingKey(SAFE_A)]: '0xcafe' });

    expect(credentialIdForSafe(recovery, SAFE_A)).toBe('0xcafe');
    expect(credentialIdForSafe({ ...recovery, chainId: 1, rpId: 'recovery.example' }, SAFE_A)).toBe('0xcafe');
  });

  it('never reuses one Safe binding for a different Safe', () => {
    const recovery = settings({ [credentialBindingKey(SAFE_A)]: '0xcafe' });

    expect(credentialIdForSafe(recovery, SAFE_B)).toBeUndefined();
  });

  it('updates and clears one Safe without changing another Safe binding', () => {
    const first = bindCredentialIdToSafe({}, SAFE_A, '0xcafe');
    const both = bindCredentialIdToSafe(first, SAFE_B, '0xbeef');
    const replaced = bindCredentialIdToSafe(both, SAFE_A.toUpperCase(), '0xdead');
    const cleared = clearCredentialIdForSafe(replaced, SAFE_A);

    expect(first).toEqual({ [credentialBindingKey(SAFE_A)]: '0xcafe' });
    expect(replaced).toEqual({
      [credentialBindingKey(SAFE_A)]: '0xdead',
      [credentialBindingKey(SAFE_B)]: '0xbeef',
    });
    expect(cleared).toEqual({ [credentialBindingKey(SAFE_B)]: '0xbeef' });
    expect(replaced[credentialBindingKey(SAFE_A)]).toBe('0xdead');
  });

  it('reports a pin only when the last Safe has a binding', () => {
    const recovery = settings({ [credentialBindingKey(SAFE_A)]: '0xcafe' });

    expect(toPublicState({ ...recovery, lastSafeAddress: SAFE_A }).credentialPinned).toBe(true);
    expect(toPublicState({ ...recovery, lastSafeAddress: SAFE_B }).credentialPinned).toBe(false);
  });

  it('migrates the old global pin only to its last successful Safe', async () => {
    const storage = installStorage({
      ...settings({}),
      credentialId: '0xbeef',
      lastSafeAddress: SAFE_A,
    });

    const migrated = await getSettings();

    expect(migrated.credentialId).toBeUndefined();
    expect(credentialIdForSafe(migrated, SAFE_A)).toBe('0xbeef');
    expect(credentialIdForSafe(migrated, SAFE_B)).toBeUndefined();
    expect(storage.saved()?.credentialId).toBeUndefined();
  });

  it('drops an old global pin when no Safe was successfully associated', async () => {
    const storage = installStorage({ ...settings({}), credentialId: '0xbeef' });

    const migrated = await getSettings();

    expect(migrated.credentialId).toBeUndefined();
    expect(migrated.credentialIds).toEqual({});
    expect(storage.saved()?.credentialId).toBeUndefined();
  });

  it('migrates RP/chain/Safe keys to a Safe-only binding', async () => {
    const oldKey = `getvela.app:100:${SAFE_A}`;
    const storage = installStorage({
      ...settings({ [oldKey]: '0xcafe' }),
      lastSafeAddress: SAFE_A,
    });

    const migrated = await getSettings();

    expect(credentialIdForSafe(migrated, SAFE_A)).toBe('0xcafe');
    expect(migrated.credentialIds[oldKey]).toBeUndefined();
    expect(storage.saved()?.credentialIds[credentialBindingKey(SAFE_A)]).toBe('0xcafe');
  });

  it('prefers the active old RP/chain binding when old keys conflict', async () => {
    const otherChainKey = `getvela.app:1:${SAFE_A}`;
    const activeKey = `getvela.app:100:${SAFE_A}`;
    installStorage({
      ...settings({ [otherChainKey]: '0xwrong', [activeKey]: '0xcorrect' }),
      lastSafeAddress: SAFE_A,
    });

    const migrated = await getSettings();

    expect(credentialIdForSafe(migrated, SAFE_A)).toBe('0xcorrect');
    expect(Object.keys(migrated.credentialIds)).toEqual([credentialBindingKey(SAFE_A)]);
  });

  it('never overwrites an existing Safe-only binding with a legacy key', async () => {
    const oldKey = `getvela.app:100:${SAFE_A}`;
    installStorage({
      ...settings({ [oldKey]: '0xlegacy', [credentialBindingKey(SAFE_A)]: '0xcurrent' }),
      lastSafeAddress: SAFE_A,
    });

    const migrated = await getSettings();

    expect(credentialIdForSafe(migrated, SAFE_A)).toBe('0xcurrent');
  });

  it('does not rewrite storage once bindings are already normalized', async () => {
    const storage = installStorage(settings({ [credentialBindingKey(SAFE_A)]: '0xcafe' }));

    const loaded = await getSettings();

    expect(credentialIdForSafe(loaded, SAFE_A)).toBe('0xcafe');
    expect(storage.set).not.toHaveBeenCalled();
  });

  it('removes the retired extension-created Safe cache during migration', async () => {
    const storage = installStorage({
      ...settings({ [credentialBindingKey(SAFE_A)]: '0xcafe' }),
      createdSafes: { [credentialBindingKey(SAFE_A)]: { legacy: true } },
    } as Partial<RecoverySettings>);

    const migrated = await getSettings();

    expect('createdSafes' in migrated).toBe(false);
    expect('createdSafes' in (storage.saved() ?? {})).toBe(false);
    expect(credentialIdForSafe(migrated, SAFE_A)).toBe('0xcafe');
  });
});
