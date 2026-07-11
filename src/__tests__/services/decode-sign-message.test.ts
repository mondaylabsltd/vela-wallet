/**
 * decodePersonalMessage — the single decode path for the signing sheet's
 * personal_sign preview (issue #82). Guards that readable UTF-8 (emoji, CJK,
 * accented Latin, multi-line SIWE) renders as text, while genuinely binary
 * payloads (raw hashes / non-UTF-8) fall back to a short hex preview.
 */
import { decodePersonalMessage } from '@/services/decode-sign-message';

/** UTF-8 encode a string to a 0x-hex payload — what a dApp's stringToHex sends. */
function toHex(s: string): string {
  const bytes = new TextEncoder().encode(s);
  return '0x' + Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

describe('decodePersonalMessage', () => {
  it('decodes the biubiu default message with a trailing emoji (the #82 repro)', () => {
    expect(decodePersonalMessage(toHex('Hello from biubiu.tools 👋'))).toBe('Hello from biubiu.tools 👋');
  });

  it('decodes accented Latin', () => {
    expect(decodePersonalMessage(toHex('Café résumé'))).toBe('Café résumé');
  });

  it('decodes CJK', () => {
    expect(decodePersonalMessage(toHex('签名消息'))).toBe('签名消息');
  });

  it('still decodes pure ASCII', () => {
    expect(decodePersonalMessage(toHex('Test sign message'))).toBe('Test sign message');
  });

  it('preserves multi-line / tabbed text (SIWE is multi-line)', () => {
    const siwe = 'example.com wants you to sign in\n\nURI: https://example.com\ntab\tend';
    expect(decodePersonalMessage(toHex(siwe))).toBe(siwe);
  });

  it('falls back to hex for a raw 32-byte hash (binary, not text)', () => {
    const hash = '0x' + 'de1a'.repeat(16); // 32 bytes, contains control bytes → binary
    const out = decodePersonalMessage(hash);
    expect(out.startsWith('0x')).toBe(true);
    expect(out).not.toContain('�');
  });

  it('falls back to hex for a payload containing a NUL control byte', () => {
    // "AB" + NUL — a control char forces the binary fallback even though partly ASCII.
    expect(decodePersonalMessage('0x414200')).toMatch(/^0x/);
  });

  it('truncates a long hex fallback with an ellipsis', () => {
    const long = '0x' + '01'.repeat(64); // 64 bytes of control-ish binary
    const out = decodePersonalMessage(long);
    expect(out.endsWith('...')).toBe(true);
  });
});
