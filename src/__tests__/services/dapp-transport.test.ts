/**
 * Tests for parseRemoteInjectURL — the entry point of the dApp connect flow
 * (US 5.1). A scanned/pasted pairing link must yield a complete session
 * {serverUrl, sessionId, nonce, secret} for the two supported formats, and must
 * return null (→ "invalid pairing link") for anything missing a piece, so a
 * malformed QR can never open a half-formed session.
 */
import { parseRemoteInjectURL } from '@/services/dapp-transport';

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
