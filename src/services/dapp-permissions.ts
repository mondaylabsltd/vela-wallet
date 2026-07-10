// Per-origin connection grants for the in-app dApp browser.
//
// The signing brain (use-dapp-signing.ts) answers eth_requestAccounts INSTANTLY
// with the active address and no prompt — because for the relay transports the
// pairing act (scanning the QR / confirming the WalletPair fingerprint) WAS the
// consent. An in-app browser has no prior pairing, so it must own its own
// per-origin consent + grant store. This mirrors the Safari extension's
// storage.local `vela.perm.<origin>` model (background.js), including the
// load-bearing rule: NEVER drop a grant on a cold/empty account read, or a
// transient empty state logs the user out of every open dApp.
//
// See docs/dapp-browser/ARCHITECTURE.md §5.3.
import AsyncStorage from '@react-native-async-storage/async-storage';

const PREFIX = 'vela.perm.';

export interface DAppGrant {
  origin: string;
  address: string;
  chainId: number;
  grantedAt: number;
}

export function grantKey(origin: string): string {
  return PREFIX + origin;
}

export async function getGrant(origin: string): Promise<DAppGrant | null> {
  try {
    const raw = await AsyncStorage.getItem(grantKey(origin));
    return raw ? (JSON.parse(raw) as DAppGrant) : null;
  } catch {
    return null;
  }
}

export async function setGrant(grant: DAppGrant): Promise<void> {
  try {
    await AsyncStorage.setItem(grantKey(grant.origin), JSON.stringify(grant));
  } catch {
    /* best-effort persist */
  }
}

export async function revokeGrant(origin: string): Promise<void> {
  try {
    await AsyncStorage.removeItem(grantKey(origin));
  } catch {
    /* best-effort */
  }
}

/**
 * The accounts to expose to `origin` given the wallet's current addresses.
 *
 * - No grant → `[]` (dApp sees a disconnected wallet; `eth_accounts` never prompts).
 * - Grant + address still present → `[address]`.
 * - Grant + address gone → `[]`.
 * - Grant + UNKNOWN current addresses (cold load, `null`/empty) → `[address]`:
 *   trust the grant, do NOT log the user out on a transient empty read.
 */
export function resolveGranted(grant: DAppGrant | null, currentAddresses: string[] | null): string[] {
  if (!grant) return [];
  if (currentAddresses == null || currentAddresses.length === 0) return [grant.address];
  const present = currentAddresses.some((a) => a.toLowerCase() === grant.address.toLowerCase());
  return present ? [grant.address] : [];
}

/**
 * Whether a grant should be dropped: ONLY when the account list is known
 * (present + non-empty) and no longer contains the granted address. Never on a
 * cold/empty read (that would revoke every dApp on app launch).
 */
export function shouldDropGrant(grant: DAppGrant | null, currentAddresses: string[] | null): boolean {
  if (!grant) return false;
  if (currentAddresses == null || currentAddresses.length === 0) return false;
  return !currentAddresses.some((a) => a.toLowerCase() === grant.address.toLowerCase());
}
