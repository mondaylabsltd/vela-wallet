/**
 * Per-request routing for concurrent dApp sessions (the two-slot raison d'être).
 *
 * The provider can hold TWO transports at once: a durable one (a live WalletPair or
 * remote-inject session) and a transient Safari-extension sign transport installed by
 * `beginExtensionSign`. When both are live, a response MUST go to the transport that
 * OWNS the originating request — bound PER-REQUEST on the request object, never a
 * single shared ref.
 *
 * Why (fund-safety, critic findings F2/F3/F4):
 *  - F2: with a shared ref, an extension signature would be delivered over the
 *    WalletPair socket to the WRONG origin (leak), or a WalletPair read resolving after
 *    an extension request would settle the extension's rid (swallowing the real sign).
 *  - F3: the sheet / history / SIWE guard must use the request's OWN dApp identity, not
 *    a concurrent session's global dappInfo.
 *  - F4: a cold-launch extension sign must sign against the origin's granted chain, not
 *    the volatile global chain a WalletPair session owns.
 *
 * These pure resolvers are the single seam the provider routes through, so the
 * concurrent-isolation invariant is unit-testable (see concurrent-session.test.ts).
 */
import type { DAppTransport, DAppInfo } from '@/services/dapp-transport';

/** The per-request fields an extension sign stamps onto its request; ordinary
 *  (WalletPair / remote-inject) requests carry none and fall back to the globals. */
export interface RoutedRequest {
  __transport?: DAppTransport;
  __chainId?: number;
  __dapp?: DAppInfo | null;
}

/**
 * The transport that OWNS a request. Its own `__transport` when stamped (an extension
 * sign), else the durable transport. NEVER a single shared ref — that is the leak/
 * mis-settle bug (F2). Returns null if neither is available (nothing to answer on).
 */
export function responseTransport(
  req: RoutedRequest | null | undefined,
  durable: DAppTransport | null,
): DAppTransport | null {
  return (req && req.__transport) || durable;
}

/** The chain to sign/display against for a request (F4): its own `__chainId` when
 *  stamped (the extension origin's granted chain), else the global chain. */
export function requestChainId(req: RoutedRequest | null | undefined, globalChainId: number): number {
  return req && req.__chainId != null ? req.__chainId : globalChainId;
}

/** The dApp identity for a request (F3): its own `__dapp` when stamped (the extension
 *  origin), else the global dappInfo. */
export function requestDApp(
  req: RoutedRequest | null | undefined,
  globalDApp: DAppInfo | null,
): DAppInfo | null {
  return (req && req.__dapp) || globalDApp;
}

/**
 * §12.1.6 — the account index to sign an extension request from: the one matching the
 * origin's GRANTED address, so the app never silently signs from whatever account the
 * user happens to have active. Falls back to the current active index when there is no
 * granted address, or the granted account isn't owned (the sheet then shows the real
 * signer — VISIBLE, never silent; the extension background drops grants for removed
 * accounts, so an unowned grant at sign time is rare). Pure + case-insensitive.
 */
export function signAccountIndex(
  accounts: { address: string }[],
  activeIndex: number,
  grantedAddress: string | undefined | null,
): number {
  if (!grantedAddress) return activeIndex;
  const want = grantedAddress.toLowerCase();
  const idx = accounts.findIndex((a) => (a.address || '').toLowerCase() === want);
  return idx >= 0 ? idx : activeIndex;
}
