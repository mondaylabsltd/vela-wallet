/**
 * Sign-In with Ethereum (EIP-4361) message parsing + domain-binding check.
 *
 * personal_sign is the #1 phishing surface: a malicious site shows a SIWE prompt
 * whose `domain`/`uri` point at the *real* dApp, tricking the user into signing a
 * session for an account on a site they don't control. The only defense is to
 * verify the domain bound INSIDE the signed message equals the origin actually
 * making the request — exactly what wallets historically skipped.
 *
 * Pure + dependency-free so it can be unit-tested directly.
 */

export interface SiweFields {
  /** Authority the message binds to (RFC 3986 authority — host[:port]). */
  domain: string;
  /** Address the message signs in. */
  address?: string;
  /** Optional human statement line. */
  statement?: string;
  /** Full resource URI the session grants access to. */
  uri?: string;
  chainId?: number;
  nonce?: string;
}

/**
 * Parse an EIP-4361 message. Returns null when the text isn't a SIWE message
 * (so the caller falls back to plain-message display). Conservative: it requires
 * the canonical first line so arbitrary prose is never mis-parsed as a sign-in.
 */
export function parseSiwe(message: string): SiweFields | null {
  if (!message || typeof message !== 'string') return null;
  // Normalize CRLF/CR → LF. Web (eth-rpc / WalletConnect) SIWE payloads commonly
  // carry "\r\n"; splitting on "\n" alone would leave a trailing "\r" that breaks
  // the line-1 anchor and silently disables phishing detection.
  const lines = message.replace(/\r\n?/g, '\n').split('\n');
  // Line 1: "{domain} wants you to sign in with your Ethereum account:"
  const m = lines[0]?.match(/^([^\s]+) wants you to sign in with your Ethereum account:$/);
  if (!m) return null;
  const domain = m[1];
  // The domain must be a bare RFC-3986 authority (host[:port]). Reject userinfo
  // ("uniswap.org@evil.com"), a path, or a scheme — these let an attacker show a
  // trusted-looking prefix while `hostOf` resolves to their own host, defeating
  // the whole binding check. Treat such messages as non-SIWE (plain text).
  if (/[@/\\?#]|:\/\//.test(domain)) return null;

  const out: SiweFields = { domain };
  // Line 2 (if present and address-shaped) is the account.
  const maybeAddr = lines[1]?.trim();
  if (maybeAddr && /^0x[0-9a-fA-F]{40}$/.test(maybeAddr)) out.address = maybeAddr;

  for (const line of lines) {
    const uri = line.match(/^URI:\s*(.+)$/);
    if (uri) out.uri = uri[1].trim();
    const chain = line.match(/^Chain ID:\s*(\d+)$/);
    if (chain) out.chainId = parseInt(chain[1], 10);
    const nonce = line.match(/^Nonce:\s*(.+)$/);
    if (nonce) out.nonce = nonce[1].trim();
  }

  // The statement is the optional block sitting between two blank lines:
  //   <address>\n\n<statement>\n\nURI: …
  // When absent, the field list follows the first blank directly.
  const firstBlank = lines.indexOf('');
  if (firstBlank >= 2 && firstBlank + 1 < lines.length) {
    const after = lines[firstBlank + 1];
    const isField = /^(URI|Version|Chain ID|Nonce|Issued At|Expiration Time|Not Before|Request ID|Resources):/.test(after ?? '');
    if (after && !isField) {
      const secondBlank = lines.indexOf('', firstBlank + 1);
      const end = secondBlank > firstBlank ? secondBlank : firstBlank + 2;
      const stmt = lines.slice(firstBlank + 1, end).join(' ').trim();
      if (stmt) out.statement = stmt;
    }
  }

  return out;
}

/** Lowercased host (no port) of a URL/origin string, or null when unparseable. */
export function siweHost(value: string | undefined): string | null {
  if (!value) return null;
  try {
    // Accept bare hosts ("app.uniswap.org"), full origins, and authority[:port].
    const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(value) ? value : `https://${value}`;
    // hostname (not host) → drop the port so a non-default port can't cause a
    // false mismatch; strip a single trailing FQDN dot for the same reason.
    return new URL(withScheme).hostname.toLowerCase().replace(/\.$/, '');
  } catch {
    // Fail safe: an unparseable value is "unknown", never a half-parsed host that
    // could spuriously match.
    return null;
  }
}

export type SiweBinding = 'ok' | 'mismatch' | 'unknown';

/**
 * Compare the domain bound inside a SIWE message to the origin actually making
 * the request.
 *   - 'ok'       — they match (safe to show "Sign in to <domain>")
 *   - 'mismatch' — they differ (PHISHING: warn loudly)
 *   - 'unknown'  — we don't know the request origin, so can't verify
 */
export function checkSiweDomainBinding(
  siweDomain: string | undefined,
  requestOrigin: string | undefined,
): SiweBinding {
  const a = siweHost(siweDomain);
  const b = siweHost(requestOrigin);
  if (!a) return 'unknown';
  if (!b) return 'unknown';
  return a === b ? 'ok' : 'mismatch';
}
