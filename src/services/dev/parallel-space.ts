/**
 * Parallel space — a dev-only test environment.
 *
 * The parallel space is the real Vela app with exactly ONE difference: passkey
 * signing is served by a fixed keyset (see `passkey-fixture.ts`) instead of a real
 * device authenticator. Chains, bundler, backend, storage, transports and UI are all
 * the real thing. This lets every feature — onboarding, home, send, receive, connect
 * dApp, settings — be driven end-to-end (and on a real network like Gnosis) without a
 * biometric prompt, deterministically.
 *
 * Boundary (real space ⇄ parallel space):
 *   - Passkey / WebAuthn:  real device credential  →  fixed fixture keyset   (the ONLY change)
 *   - Wallet accounts:     user's real accounts     →  fixture Safe accounts (swapped, real cache backed up & restored on exit)
 *   - Everything else:     unchanged (real RPC / bundler / relay / storage / UI)
 *
 * All parallel-space pages live under the `/parallel/*` route prefix and the app shows
 * a persistent "PARALLEL SPACE" badge whenever the mode is active, so the two spaces
 * can never be confused.
 *
 * Everything here is gated on `__DEV__` (the passkey override in `modules/passkey`
 * is a compile-time no-op in release), so none of it ships enabled in production.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Passkey from '@/modules/passkey';
import type { Account, StoredAccount } from '@/models/types';
import {
  FIXTURE_ACCOUNTS,
  FIXTURE_ACCOUNT,
  FIXTURE_ADDRESSES,
  buildMockAssertion,
  buildMockRegistration,
} from './passkey-fixture';

// Storage keys. The account keys intentionally match storage.ts so the real signing
// path (findAccountByCredentialId → publicKeyHex) resolves fixture keys unchanged.
const K_ACCOUNTS = 'vela.accounts';
const K_ACTIVE_INDEX = 'vela.activeAccountIndex';
const K_FLAG = 'vela.parallelSpace';
const K_BACKUP = 'vela.parallelSpace.realWalletBackup';
// Connection sessions created while inside the parallel space — cleared on exit so
// they don't linger in the real space.
const K_REMOTE_SESSION = 'vela.remoteInjectSession';
const K_WALLETPAIR_SESSION = 'vela.walletpairSession';

const FIXTURE_CREATED_AT = '2025-01-01T00:00:00.000Z';

// ---------------------------------------------------------------------------
// Live state (drives the UI badge; survives navigation, reset on exit/boot)
//
// Kept on `globalThis`, not module scope: Metro can bundle this module more than
// once (app code + the expo-router route tree resolve it separately), and a module-
// local flag would let the installer and the badge see different values. A single
// global source of truth makes the badge reliable regardless.
// ---------------------------------------------------------------------------

const G = globalThis as any;

function listeners(): Set<() => void> {
  return (G.__VELA_PARALLEL_LISTENERS__ ??= new Set<() => void>());
}

function setActive(v: boolean): void {
  G.__VELA_PARALLEL__ = v;
  listeners().forEach((l) => l());
}

/** Synchronous check for whether the parallel space is currently active. */
export function isParallelActiveSync(): boolean {
  return !!G.__VELA_PARALLEL__;
}

/** Subscribe to parallel-space activation changes (for the badge). Returns unsubscribe. */
export function subscribeParallel(listener: () => void): () => void {
  listeners().add(listener);
  return () => { listeners().delete(listener); };
}

// ---------------------------------------------------------------------------
// Fixture accounts as wallet records
// ---------------------------------------------------------------------------

/** The fixture accounts as `StoredAccount[]` (what the wallet persists / signs from). */
export function fixtureStoredAccounts(): StoredAccount[] {
  return FIXTURE_ACCOUNTS.map((a) => ({
    id: a.id,
    name: a.name,
    address: a.address,
    publicKeyHex: a.publicKeyHex,
    createdAt: FIXTURE_CREATED_AT,
  }));
}

/** The fixture accounts as `Account[]` (what the wallet context holds in memory). */
export function fixtureAccounts(): Account[] {
  return FIXTURE_ACCOUNTS.map((a) => ({
    id: a.id,
    name: a.name,
    address: a.address,
    createdAt: FIXTURE_CREATED_AT,
  }));
}

// ---------------------------------------------------------------------------
// Passkey override install/uninstall
// ---------------------------------------------------------------------------

/** Wire the fixed-key signer into the passkey module. Idempotent; flips the badge on. */
export function installMockPasskey(): void {
  Passkey.__setPasskeyOverride({
    sign: async (challengeHex, credentialId) =>
      buildMockAssertion(challengeHex, { credentialId }),
    authenticate: async () =>
      buildMockAssertion('0x' + '00'.repeat(32), { credentialId: FIXTURE_ACCOUNT.id }),
    register: async () => buildMockRegistration({ credentialId: FIXTURE_ACCOUNT.id }),
  });
  setActive(true);
}

/** Remove the fixed-key signer; flips the badge off. */
export function uninstallMockPasskey(): void {
  Passkey.__setPasskeyOverride(null);
  setActive(false);
}

// ---------------------------------------------------------------------------
// Enter / exit
// ---------------------------------------------------------------------------

/**
 * Enter the parallel space: install the fixed-key signer and swap the fixture wallet
 * into local storage. The real wallet cache is backed up on first entry and restored
 * on exit; the true keys live in the device passkey, never here, so this swap is safe
 * and fully reversible.
 *
 * The live wallet context won't reflect the swap until it reloads — the `/parallel`
 * layout dispatches `SET_WALLET` with {@link fixtureAccounts}; a bare reload also works.
 */
export async function enterParallelSpace(): Promise<void> {
  installMockPasskey();

  const alreadyIn = (await AsyncStorage.getItem(K_FLAG)) === '1';
  if (!alreadyIn) {
    const [accounts, idx] = await Promise.all([
      AsyncStorage.getItem(K_ACCOUNTS),
      AsyncStorage.getItem(K_ACTIVE_INDEX),
    ]);
    await AsyncStorage.setItem(K_BACKUP, JSON.stringify({ accounts, idx }));
  }

  await AsyncStorage.multiSet([
    [K_ACCOUNTS, JSON.stringify(fixtureStoredAccounts())],
    [K_ACTIVE_INDEX, '0'],
    [K_FLAG, '1'],
    ['dev_unlocked', '1'],
  ]);
}

/**
 * Exit the parallel space: remove the fixed-key signer, restore the real wallet cache
 * captured on entry, and clear any parallel-created connection sessions.
 */
export async function exitParallelSpace(): Promise<void> {
  uninstallMockPasskey();

  const rawBackup = await AsyncStorage.getItem(K_BACKUP);
  if (rawBackup) {
    try {
      const { accounts, idx } = JSON.parse(rawBackup) as { accounts: string | null; idx: string | null };
      const restore: [string, string][] = [];
      const remove: string[] = [];
      accounts != null ? restore.push([K_ACCOUNTS, accounts]) : remove.push(K_ACCOUNTS);
      idx != null ? restore.push([K_ACTIVE_INDEX, idx]) : remove.push(K_ACTIVE_INDEX);
      if (restore.length) await AsyncStorage.multiSet(restore);
      if (remove.length) await AsyncStorage.multiRemove(remove);
    } catch {
      await AsyncStorage.multiRemove([K_ACCOUNTS, K_ACTIVE_INDEX]);
    }
  } else {
    await AsyncStorage.multiRemove([K_ACCOUNTS, K_ACTIVE_INDEX]);
  }

  await AsyncStorage.multiRemove([K_BACKUP, K_FLAG, K_REMOTE_SESSION, K_WALLETPAIR_SESSION]);
}

/**
 * On app boot (dev only): if the parallel flag is set, re-install the fixed-key signer
 * so a reload inside the parallel space stays in it (fixtures are already in storage).
 */
export async function applyParallelSpaceOnBoot(): Promise<void> {
  try {
    if ((await AsyncStorage.getItem(K_FLAG)) === '1') installMockPasskey();
  } catch {
    /* storage unavailable — stay in real space */
  }
}

// ---------------------------------------------------------------------------
// Console (vela.parallel.*)
// ---------------------------------------------------------------------------

/** Install the `vela.parallel.*` dev console commands. Call once, in `__DEV__`. */
export function installParallelConsole(): void {
  const g = globalThis as any;
  const summary = () => ({
    active: isParallelActiveSync(),
    accounts: FIXTURE_ACCOUNTS.map((a) => ({ name: a.name, id: a.id, address: a.address })),
  });
  const api = {
    async enter() {
      await enterParallelSpace();
      console.log('[vela] parallel space ON — reload (or open /parallel) to load the fixture wallet');
      return summary();
    },
    async exit() {
      await exitParallelSpace();
      console.log('[vela] parallel space OFF — reload to restore the real wallet');
      return summary();
    },
    status: () => summary(),
    addresses: () => FIXTURE_ADDRESSES,
    help() {
      console.log(
        '[vela.parallel] test environment (fixed passkey, everything else real)\n' +
        '  vela.parallel.enter()     seed fixture wallet + mock passkey\n' +
        '  vela.parallel.exit()      restore real wallet + remove mock passkey\n' +
        '  vela.parallel.status()    show active state + fixture accounts\n' +
        '  vela.parallel.addresses() fixture Safe addresses (fund these on-chain)',
      );
      return undefined;
    },
  };
  g.vela = Object.assign(g.vela ?? {}, { parallel: api });
}
