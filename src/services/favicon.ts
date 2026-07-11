/**
 * Derive a dApp's favicon URL from its host — the site's OWN /favicon.ico, never
 * a third-party favicon service, so showing a dApp's logo never leaks which dApp
 * you're on to Google/DuckDuckGo/etc.
 *
 * Returns `undefined` for non-registrable hosts (the `clear-signing-test`
 * harness, `localhost`, bare IPs) so callers fall back to a monogram / globe
 * rather than requesting a broken image.
 *
 * Accepts a full URL, an origin, or a bare host — the protocol, path and port
 * are stripped.
 */
export function faviconForHost(domain?: string): string | undefined {
  if (!domain) return undefined;
  const host = domain.replace(/^[a-z]+:\/\//i, '').split('/')[0].split(':')[0].trim();
  if (!host || !host.includes('.') || /^\d+(\.\d+){3}$/.test(host)) return undefined;
  return `https://${host}/favicon.ico`;
}
