/**
 * App Group account sync (app → Safari extension).
 *
 * Phase A of the Safari extension needs to answer connect / read / state
 * requests entirely in Safari (zero app hop). To do that the extension reads a
 * PUBLIC account snapshot the app writes to the shared App Group container:
 *
 *   vela.ext.account.json = {
 *     address, name,                         // active Safe address + display name
 *     accounts: [{ name, address }],          // all accounts (for the connect sheet)
 *     chainId,                                // default/current chain (>= 1)
 *     chains: { [chainId]: { name, rpcUrl, bundlerUrl } },  // all 12 networks
 *     updatedAt,
 *   }
 *
 * The extension's native handler (SafariWebExtensionHandler `getAccount`) reads
 * this file; background.js proxies read-only RPC to `chains[chainId].rpcUrl` /
 * `.bundlerUrl`. NOTHING sensitive crosses — no credentialId, no key material;
 * the shared container is readable on jailbroken devices, so it carries only the
 * same public data a dApp would learn anyway (address, chain, public RPC URLs).
 *
 * Written reactively by <AccountFileWriter/> on account/chain change AND on every
 * foreground (§12.1.6 — a user who installed while already logged in must not
 * have an empty extension cache). No-op off iOS (mirrors app-group-echo).
 */

import AsyncStorage from '@react-native-async-storage/async-storage';
import * as AppGroup from '@/modules/app-group';
import { getAllNetworksSync } from '@/models/network';

export const ACCOUNT_FILE = 'vela.ext.account.json';

// Universal-Link attestation. The Safari extension picks its sign hand-off launch
// (UL vs the velawallet:// scheme) from THIS flag, not a compile-time constant: a
// failed UL is destructive (it navigates the dApp tab away and loses the pending
// sign), so it may only be used once we KNOW the applinks association resolves on
// this device. The app proves that by setting the flag whenever it is opened via a
// https://getvela.app/sign Universal Link (see <AccountFileWriter/>).
//
// PERISHABLE (fund-safety): the association can silently break AFTER attestation —
// e.g. the user picks "Open in Safari" for getvela.app, a persistent per-domain iOS
// preference the app can't observe — after which a UL launch would hijack the dApp
// tab. So attestation is NOT write-once-true: it stores a timestamp and EXPIRES
// after UL_TTL_MS. Every successful getvela.app/sign UL open (real sign or the popup
// probe) refreshes it, so active users never lapse; if UL stops resolving, no more
// opens land → it ages out → the extension reverts to the always-safe scheme. (The
// extension ALSO self-heals faster: a failed UL lands on getvela.app where content.js
// vetoes it after a single failure — this TTL is the backstop.) Device-level.
const UL_VERIFIED_KEY = 'vela.ext.ulVerifiedAt';
const UL_TTL_MS = 14 * 24 * 60 * 60 * 1000; // 14 days

/** The raw attestation timestamp (ms), or 0 if never/unreadable. The extension
 *  compares this against its self-heal veto timestamp (UL_BROKEN) so a re-attestation
 *  NEWER than a break auto-clears the veto — no optimistic clearing, no race. */
export async function getUniversalLinkVerifiedAt(): Promise<number> {
  try {
    const ts = parseInt((await AsyncStorage.getItem(UL_VERIFIED_KEY)) || '', 10);
    return Number.isFinite(ts) && ts > 0 ? ts : 0;
  } catch {
    return 0;
  }
}

export async function getUniversalLinkVerified(): Promise<boolean> {
  const ts = await getUniversalLinkVerifiedAt();
  return ts > 0 && Date.now() - ts < UL_TTL_MS && Date.now() - ts >= 0;
}

/** Record (fresh timestamp) that a getvela.app UL resolved to the app on this device. */
export async function markUniversalLinkVerified(): Promise<void> {
  try {
    await AsyncStorage.setItem(UL_VERIFIED_KEY, String(Date.now()));
  } catch {
    /* storage unavailable — stay unverified (extension keeps using the scheme) */
  }
}

// The wallet has no global "current network" (it is intentionally multi-chain),
// so the extension cache advertises a STABLE default chain. Each connected dApp
// then picks/switches its own chain via the connect sheet + wallet_switchEthereumChain
// (stored per-origin in the extension). Do NOT source this from the volatile
// dApp-bridge connection chainId — that would make eth_chainId reflect whatever
// the last WalletPair dApp switched to.
export const DEFAULT_EXT_CHAIN_ID = 1; // Ethereum

export interface ExtAccountCache {
  address: string;
  name: string;
  accounts: { name: string; address: string }[];
  chainId: number;
  chains: Record<string, { name: string; rpcUrl: string; bundlerUrl: string }>;
  updatedAt: number;
  /** True once a getvela.app /sign Universal Link has resolved to the app on this
   *  device AND the attestation is within its TTL; the extension launches the sign
   *  hand-off via the UL only when this is true (and not self-heal-vetoed). */
  ulVerified?: boolean;
  /** Raw attestation timestamp (ms; 0 if none). The extension compares it against its
   *  self-heal veto so a re-attestation newer than a break re-enables the UL. */
  ulVerifiedAt?: number;
  /** The app's color-scheme PREFERENCE ('auto'|'light'|'dark') so the extension UI
   *  matches the app exactly (a forced dark app → dark sheet even on a light system;
   *  'auto' → the extension follows the OS `prefers-color-scheme`, like the app). */
  theme?: 'auto' | 'light' | 'dark';
  /** The app's RESOLVED display language (e.g. 'en', 'zh', 'ja') so the extension can
   *  render its UI strings in the same language the user reads in the app. */
  locale?: string;
}

/** All known networks as the extension's chainId → endpoints map. */
export function buildChainsMap(): ExtAccountCache['chains'] {
  const chains: ExtAccountCache['chains'] = {};
  for (const n of getAllNetworksSync()) {
    chains[String(n.chainId)] = {
      name: n.displayName,
      rpcUrl: n.rpcURL,
      bundlerUrl: n.bundlerURL,
    };
  }
  return chains;
}

export function buildAccountCache(input: {
  address: string;
  name: string;
  accounts: { name: string; address: string }[];
  chainId?: number;
  ulVerified?: boolean;
  ulVerifiedAt?: number;
  theme?: 'auto' | 'light' | 'dark';
  locale?: string;
}): ExtAccountCache {
  return {
    address: input.address,
    name: input.name || '',
    // Re-project to EXACTLY { name, address } so no extra field a caller might
    // pass (e.g. a richer Account with publicKeyHex) can ever reach the shared,
    // world-readable file. Belt-and-braces on top of the caller's own mapping.
    accounts: (input.accounts || []).map((a) => ({ name: a.name, address: a.address })),
    chainId: input.chainId && input.chainId > 0 ? input.chainId : DEFAULT_EXT_CHAIN_ID,
    chains: buildChainsMap(),
    updatedAt: Date.now(),
    ulVerified: !!input.ulVerified,
    ulVerifiedAt: input.ulVerifiedAt && input.ulVerifiedAt > 0 ? input.ulVerifiedAt : 0,
    theme: input.theme === 'light' || input.theme === 'dark' ? input.theme : 'auto',
    locale: input.locale || '',
  };
}

/** Write the public account snapshot for the extension. No-op off iOS. */
export async function writeAccountCache(input: {
  address: string;
  name: string;
  accounts: { name: string; address: string }[];
  chainId?: number;
  theme?: 'auto' | 'light' | 'dark';
  locale?: string;
}): Promise<void> {
  if (!AppGroup.isSupportedSync()) return;
  if (!input.address) return void (await clearAccountCache());
  try {
    // Attestation is device-level + persisted, not passed by the caller — read it
    // here so every write reflects current state. Emit BOTH the TTL-checked boolean
    // (for the popup) and the raw timestamp (for the extension's veto comparison).
    const ulVerifiedAt = await getUniversalLinkVerifiedAt();
    const ulVerified = ulVerifiedAt > 0 && Date.now() - ulVerifiedAt < UL_TTL_MS && Date.now() - ulVerifiedAt >= 0;
    await AppGroup.writeFile(ACCOUNT_FILE, JSON.stringify(buildAccountCache({ ...input, ulVerified, ulVerifiedAt })));
  } catch (e) {
    console.log('[account-sync] write failed', e);
  }
}

/** Remove the snapshot (logout / no active account) → extension shows empty-state. */
export async function clearAccountCache(): Promise<void> {
  if (!AppGroup.isSupportedSync()) return;
  try {
    await AppGroup.remove(ACCOUNT_FILE);
  } catch (e) {
    console.log('[account-sync] clear failed', e);
  }
}
