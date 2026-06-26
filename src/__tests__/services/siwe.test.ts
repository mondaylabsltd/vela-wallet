import { parseSiwe, checkSiweDomainBinding } from '@/services/siwe';

const SIWE = [
  'app.uniswap.org wants you to sign in with your Ethereum account:',
  '0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1',
  '',
  'Sign in to Uniswap',
  '',
  'URI: https://app.uniswap.org',
  'Version: 1',
  'Chain ID: 1',
  'Nonce: 8a3b9f2c',
  'Issued At: 2026-01-01T00:00:00.000Z',
].join('\n');

describe('parseSiwe', () => {
  it('parses a canonical EIP-4361 message', () => {
    const r = parseSiwe(SIWE);
    expect(r).not.toBeNull();
    expect(r!.domain).toBe('app.uniswap.org');
    expect(r!.address).toBe('0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1');
    expect(r!.uri).toBe('https://app.uniswap.org');
    expect(r!.chainId).toBe(1);
    expect(r!.nonce).toBe('8a3b9f2c');
    expect(r!.statement).toBe('Sign in to Uniswap');
  });

  it('returns null for plain prose (not a sign-in)', () => {
    expect(parseSiwe('gm, please sign this to continue')).toBeNull();
    expect(parseSiwe('')).toBeNull();
  });

  it('parses CRLF-terminated messages (web wallets) — phishing detection stays on', () => {
    const crlf = SIWE.replace(/\n/g, '\r\n');
    const r = parseSiwe(crlf);
    expect(r).not.toBeNull();
    expect(r!.domain).toBe('app.uniswap.org');
    // No stray "\r" leaking into fields.
    expect(r!.nonce).toBe('8a3b9f2c');
  });

  it('rejects an @-userinfo domain (spoofing) so it never gets a binding check', () => {
    const spoof = SIWE.replace('app.uniswap.org wants', 'app.uniswap.org@evil.com wants');
    expect(parseSiwe(spoof)).toBeNull();
  });

  it('rejects domains containing a path or scheme', () => {
    expect(parseSiwe(SIWE.replace('app.uniswap.org wants', 'evil.com/app.uniswap.org wants'))).toBeNull();
    expect(parseSiwe(SIWE.replace('app.uniswap.org wants', 'https://app.uniswap.org wants'))).toBeNull();
  });
});

describe('checkSiweDomainBinding', () => {
  it('ok when the SIWE domain matches the request origin', () => {
    expect(checkSiweDomainBinding('app.uniswap.org', 'https://app.uniswap.org')).toBe('ok');
    expect(checkSiweDomainBinding('app.uniswap.org', 'app.uniswap.org')).toBe('ok');
  });

  it('mismatch when domains differ (phishing)', () => {
    // The signed-in domain is the real dApp, but the request came from an evil host.
    expect(checkSiweDomainBinding('app.uniswap.org', 'https://uniswaq.app')).toBe('mismatch');
  });

  it('unknown when the request origin is missing', () => {
    expect(checkSiweDomainBinding('app.uniswap.org', undefined)).toBe('unknown');
  });

  it('matches host-only, ignoring port and trailing FQDN dot (no false mismatch)', () => {
    expect(checkSiweDomainBinding('app.uniswap.org', 'https://app.uniswap.org:443')).toBe('ok');
    expect(checkSiweDomainBinding('app.uniswap.org.', 'https://app.uniswap.org')).toBe('ok');
  });
});
