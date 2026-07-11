/**
 * faviconForHost — derive a dApp's own /favicon.ico from its host, with no
 * third-party favicon service (privacy) and a safe undefined for non-registrable
 * hosts so the UI falls back to a monogram/globe. Shared by the signing banner
 * and the in-app browser's connect-consent sheet (issue #84).
 */
import { faviconForHost } from '@/services/favicon';

describe('faviconForHost', () => {
  it('derives the host /favicon.ico from a full URL', () => {
    expect(faviconForHost('https://biubiu.tools/some/path?q=1')).toBe('https://biubiu.tools/favicon.ico');
  });

  it('derives from a bare origin', () => {
    expect(faviconForHost('https://app.uniswap.org')).toBe('https://app.uniswap.org/favicon.ico');
  });

  it('derives from a bare host and strips a port', () => {
    expect(faviconForHost('example.com:8443')).toBe('https://example.com/favicon.ico');
  });

  it('returns undefined for non-registrable hosts (localhost, bare IP, dotless)', () => {
    expect(faviconForHost('http://localhost:3000')).toBeUndefined();
    expect(faviconForHost('http://127.0.0.1:8081')).toBeUndefined();
    expect(faviconForHost('clear-signing-test')).toBeUndefined();
  });

  it('returns undefined for empty/undefined input', () => {
    expect(faviconForHost(undefined)).toBeUndefined();
    expect(faviconForHost('')).toBeUndefined();
  });
});
