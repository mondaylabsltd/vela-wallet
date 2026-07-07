// Request router for the in-app dApp browser.
//
// The browser intercepts CONNECT/state methods locally (per-origin consent +
// grants — the signing brain has no consent gate), and FORWARDS everything else
// (read-only RPC, chain switch, and all signing) to the WebViewTransport, which
// feeds the existing DAppConnectionProvider → SigningRequestModal pipeline.
//
// This module is pure so it can be unit-tested off-device; the BrowserScreen
// performs the side effects (respond via transport, show the connect sheet).
// See docs/dapp-browser/ARCHITECTURE.md §5-7.

export type BrowserAction =
  /** Answer locally, right now (accounts/permissions for a known grant state). */
  | { kind: 'respond'; result: unknown }
  /** No grant yet — the browser must show the connect-consent sheet. */
  | { kind: 'consent' }
  /** Hand to the WebViewTransport → handleIncoming (read-only RPC / switch / signing). */
  | { kind: 'forward' };

const CONNECT_METHODS = new Set(['eth_requestAccounts', 'wallet_requestPermissions']);

const EIP2255_PERMISSION = { parentCapability: 'eth_accounts' } as const;

/**
 * Decide how the browser handles a provider request.
 *
 * @param method  the EIP-1193 method
 * @param granted the accounts currently exposed to this origin (from resolveGranted)
 */
export function classifyBrowserRequest(method: string, granted: string[]): BrowserAction {
  // `eth_accounts` reflects the current grant and NEVER prompts.
  if (method === 'eth_accounts') {
    return { kind: 'respond', result: granted };
  }
  // EIP-2255 introspection mirrors the grant.
  if (method === 'wallet_getPermissions') {
    return { kind: 'respond', result: granted.length ? [EIP2255_PERMISSION] : [] };
  }
  // Connect methods: return immediately if already granted, else ask for consent.
  if (CONNECT_METHODS.has(method)) {
    if (granted.length > 0) {
      return method === 'wallet_requestPermissions'
        ? { kind: 'respond', result: [EIP2255_PERMISSION] }
        : { kind: 'respond', result: granted };
    }
    return { kind: 'consent' };
  }
  // Read-only RPC, wallet_switchEthereumChain, and every signing method flow
  // through the transport to the existing pipeline.
  return { kind: 'forward' };
}

/** Whether the connect-consent result should also expose accounts (vs permissions). */
export function isConnectMethod(method: string): boolean {
  return CONNECT_METHODS.has(method);
}
