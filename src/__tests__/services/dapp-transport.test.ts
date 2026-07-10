/**
 * Tests for parseRemoteInjectURL — the entry point of the dApp connect flow
 * (US 5.1). A scanned/pasted pairing link must yield a complete session
 * {serverUrl, sessionId, nonce, secret} for the two supported formats, and must
 * return null (→ "invalid pairing link") for anything missing a piece, so a
 * malformed QR can never open a half-formed session.
 */
import { parseRemoteInjectURL, coerceBrowserUrl } from '@/services/dapp-transport';

describe('parseRemoteInjectURL', () => {
  test('path format: /s/{sessionId}?n=&k=', () => {
    expect(parseRemoteInjectURL('https://relay.example.com/s/sess123?n=NONCE&k=SECRET')).toEqual({
      serverUrl: 'https://relay.example.com',
      sessionId: 'sess123',
      nonce: 'NONCE',
      secret: 'SECRET',
    });
  });

  test('query format: /bridge?session=&n=&k=', () => {
    expect(parseRemoteInjectURL('https://relay.example.com/bridge?session=abc&n=N1&k=K1')).toEqual({
      serverUrl: 'https://relay.example.com',
      sessionId: 'abc',
      nonce: 'N1',
      secret: 'K1',
    });
  });

  test('serverUrl is protocol + host only (path and query stripped)', () => {
    const s = parseRemoteInjectURL('https://relay.example.com:8443/s/x?n=a&k=b');
    expect(s?.serverUrl).toBe('https://relay.example.com:8443');
  });

  test('the /s/ path form wins when both a path id and ?session= are present', () => {
    const s = parseRemoteInjectURL('https://r.io/s/frompath?session=fromquery&n=a&k=b');
    expect(s?.sessionId).toBe('frompath');
  });

  test('missing nonce (n) → null', () => {
    expect(parseRemoteInjectURL('https://r.io/s/sess?k=SECRET')).toBeNull();
  });

  test('missing secret (k) → null', () => {
    expect(parseRemoteInjectURL('https://r.io/s/sess?n=NONCE')).toBeNull();
  });

  test('has credentials but no sessionId (neither /s/ nor ?session=) → null', () => {
    expect(parseRemoteInjectURL('https://r.io/bridge?n=NONCE&k=SECRET')).toBeNull();
  });

  test('not a URL at all → null (never throws)', () => {
    expect(parseRemoteInjectURL('not a url')).toBeNull();
    expect(parseRemoteInjectURL('')).toBeNull();
    expect(parseRemoteInjectURL('wc:deadbeef@2?relay=x')).toBeNull();
  });
});

/**
 * coerceBrowserUrl — the FALLBACK predicate for the two in-app browser entry
 * points (scan / paste). It must open a bare host like `app.uniswap.org` (the
 * common "type a URL" case), pass full http(s) URLs through, and reject anything
 * that isn't a web address so the caller still shows "invalid link".
 */
describe('coerceBrowserUrl', () => {
  test('full http(s) URL passes through', () => {
    expect(coerceBrowserUrl('https://app.uniswap.org/swap')).toBe('https://app.uniswap.org/swap');
    expect(coerceBrowserUrl('http://example.com')).toBe('http://example.com/');
  });

  test('bare host defaults to https (the core "type a URL" fix)', () => {
    expect(coerceBrowserUrl('app.uniswap.org')).toBe('https://app.uniswap.org/');
    expect(coerceBrowserUrl('uniswap.org/swap')).toBe('https://uniswap.org/swap');
    expect(coerceBrowserUrl('  opensea.io  ')).toBe('https://opensea.io/'); // trims first
  });

  test('non-web schemes are rejected (never loaded in the WebView)', () => {
    expect(coerceBrowserUrl('javascript:alert(1)')).toBeNull();
    expect(coerceBrowserUrl('file:///etc/passwd')).toBeNull();
    expect(coerceBrowserUrl('ftp://host/x')).toBeNull();
    expect(coerceBrowserUrl('velawallet://sign')).toBeNull();
    expect(coerceBrowserUrl('data:text/html,<h1>x</h1>')).toBeNull();
  });

  test('non-addresses (no dot / whitespace / empty) → null', () => {
    expect(coerceBrowserUrl('hello')).toBeNull();
    expect(coerceBrowserUrl('two words')).toBeNull();
    expect(coerceBrowserUrl('')).toBeNull();
    expect(coerceBrowserUrl('   ')).toBeNull();
  });

  test('order coupling: a remote-inject pairing link is https, so it must be caught by parseRemoteInjectURL FIRST', () => {
    // Guards the load-bearing branch order in Connect/Home: coerceBrowserUrl would
    // happily open the pairing link as a web page if it ran before the parser.
    const link = 'https://relay.example.com/s/sess123?n=NONCE&k=SECRET';
    expect(parseRemoteInjectURL(link)).not.toBeNull(); // parser claims it first
    expect(coerceBrowserUrl(link)).toBe(link); // but is also a valid URL — hence order matters
  });
});
