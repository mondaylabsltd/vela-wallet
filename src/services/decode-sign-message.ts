/**
 * Decode a `personal_sign` hex payload to readable text for the signing sheet.
 *
 * dApps send `personal_sign` with `params[0] = stringToHex(message)`, so the
 * bytes are (almost always) UTF-8 text. We show that text — including emoji,
 * CJK and accented Latin — and fall back to a short hex preview only for
 * genuinely binary payloads (raw 32-byte hashes / non-UTF-8).
 *
 * The readability guard is Unicode-aware. The previous guard accepted only
 * printable ASCII (`[\x20-\x7E]`), so ANY non-ASCII character — an emoji, a
 * Chinese character, an accented letter — forced the raw-hex fallback even
 * though the bytes decoded to perfectly valid text (issue #82: biubiu's default
 * "Hello from biubiu.tools 👋" rendered as hex). Now we reject only control
 * characters (C0/C1, except tab/newline/CR so multi-line SIWE stays legal) and
 * the U+FFFD replacement char a non-fatal TextDecoder emits for invalid UTF-8.
 */

/**
 * A char that marks a payload as binary, not text: a C0 control (< 0x20) other
 * than tab/newline/CR, DEL (0x7F), a C1 control (0x80–0x9F), or the U+FFFD
 * replacement char TextDecoder emits for invalid UTF-8 (e.g. a raw 32-byte
 * hash). Emoji are UTF-16 surrogate pairs (0xD800–0xDFFF), which are not in any
 * of these ranges, so they read as text.
 */
function isBinaryChar(code: number): boolean {
  if (code < 0x20) return code !== 0x09 && code !== 0x0a && code !== 0x0d;
  if (code === 0x7f) return true;
  if (code >= 0x80 && code <= 0x9f) return true;
  return code === 0xfffd;
}

export function decodePersonalMessage(hexMsg: string): string {
  try {
    const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes); // non-fatal → U+FFFD on invalid bytes
    for (let i = 0; i < decoded.length; i++) {
      if (isBinaryChar(decoded.charCodeAt(i))) {
        return `0x${clean.slice(0, 64)}${clean.length > 64 ? '...' : ''}`;
      }
    }
    return decoded;
  } catch {
    return hexMsg.slice(0, 66) + (hexMsg.length > 66 ? '...' : '');
  }
}
