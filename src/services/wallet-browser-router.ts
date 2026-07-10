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

/** Methods that move value or produce a signature. */
const SIGNING_METHODS = new Set([
  'eth_sendTransaction',
  'personal_sign',
  'eth_sign',
  'eth_signTypedData',
  'eth_signTypedData_v1',
  'eth_signTypedData_v3',
  'eth_signTypedData_v4',
  'wallet_sendCalls',
]);

/**
 * A loopback / private-LAN / link-local host — the ONLY http origins exempt from
 * the insecure-signing block (dev + the on-device test dApp served over the LAN).
 *
 * Matches EXACT IPs only (a fully-anchored dotted quad or IPv6), never a hostname
 * that merely starts with those digits: `10.0.0.1.evil.com` is a public FQDN an
 * attacker can register (DNS labels may start with a digit) and MUST NOT be exempt.
 */
function isLoopbackOrPrivateHost(host: string): boolean {
  // URL.hostname returns IPv6 in bracketed form ("[::1]") — strip the brackets.
  const h = host.toLowerCase().replace(/^\[|\]$/g, '');
  if (h === 'localhost' || h.endsWith('.local')) return true;
  // IPv6
  if (h === '::1') return true;
  if (/^f[cd][0-9a-f]{2}:/.test(h)) return true; // fc00::/7 unique-local
  if (/^fe80:/.test(h)) return true; // link-local
  // IPv4 — must be a complete dotted quad, each octet 0–255
  const m = h.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (!m) return false;
  const o = m.slice(1, 5).map(Number);
  if (o.some((n) => n > 255)) return false;
  const [a, b] = o;
  if (a === 127) return true; // loopback
  if (a === 10) return true; // private
  if (a === 192 && b === 168) return true; // private
  if (a === 172 && b >= 16 && b <= 31) return true; // private
  if (a === 169 && b === 254) return true; // link-local
  return false;
}

/**
 * A PUBLIC http (non-TLS) origin, where a MITM can inject page script. Loopback /
 * private-LAN / link-local hosts and `.local` are exempt so local/dev dApps (and the
 * on-device test dApp, served over the LAN) still work.
 */
export function isInsecurePublicOrigin(origin: string): boolean {
  try {
    const u = new URL(origin);
    if (u.protocol !== 'http:') return false;
    return !isLoopbackOrPrivateHost(u.hostname);
  } catch {
    return true; // unparseable origin → treat as insecure
  }
}

/** Whether a signing/value-moving request must be refused on this origin (insecure http). */
export function shouldBlockInsecureSigning(method: string, origin: string): boolean {
  return SIGNING_METHODS.has(method) && isInsecurePublicOrigin(origin);
}

/**
 * The FULL browser decision for one provider request — the single source of truth
 * the BrowserScreen executes. Pure (no storage / no UI) so every branch is unit-
 * testable; the screen only performs the side effect the decision names.
 *
 * `granted` is the post-isMainFrame exposure ([] for a subframe), `pendingConsentOrigin`
 * is the origin of the connect sheet already open (or null).
 */
export type BrowserDecision =
  | { kind: 'respond'; result: unknown }
  | { kind: 'reject'; code: number; message: string }
  | { kind: 'open-consent' } // show a fresh connect sheet
  | { kind: 'merge-consent' } // coalesce into the sheet already open for this origin
  | { kind: 'forward' }; // hand to the WebViewTransport → signing pipeline

export function decideBrowserRequest(input: {
  method: string;
  origin: string;
  isMainFrame: boolean;
  granted: string[];
  hasActiveAccount: boolean;
  pendingConsentOrigin: string | null;
}): BrowserDecision {
  const { method, origin, isMainFrame, granted, hasActiveAccount, pendingConsentOrigin } = input;
  const action = classifyBrowserRequest(method, granted);

  if (action.kind === 'respond') return { kind: 'respond', result: action.result };

  if (action.kind === 'consent') {
    // §5.2 — a cross-origin iframe can never request accounts.
    if (!isMainFrame) return { kind: 'reject', code: 4100, message: 'Unauthorized frame' };
    if (!hasActiveAccount) return { kind: 'reject', code: 4001, message: 'No account available' };
    // Coalesce duplicate prompts from the same origin so the earlier promise never hangs;
    // reject a colliding second origin (a page can't queue two connect sheets).
    if (pendingConsentOrigin === origin) return { kind: 'merge-consent' };
    if (pendingConsentOrigin != null) {
      return { kind: 'reject', code: 4001, message: 'Another connection request is pending' };
    }
    return { kind: 'open-consent' };
  }

  // forward: read-only RPC / chain switch / signing — but never sign on insecure http.
  if (shouldBlockInsecureSigning(method, origin)) {
    return { kind: 'reject', code: 4100, message: 'Signing is disabled on insecure (http) sites' };
  }
  return { kind: 'forward' };
}
