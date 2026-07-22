/**
 * Native runtime polyfills for browser globals that Hermes does NOT provide.
 *
 * The Expo Web build already has crypto / btoa / atob / Buffer (browser +
 * Metro-web shims), so the web variant of this file (`polyfills.web.ts`) is a
 * no-op. On iOS/Android under Hermes these globals are ABSENT, and several
 * dependencies assume them:
 *   - WalletPair protocol + @noble/* → crypto.getRandomValues (X25519 keygen, nonces)
 *   - WalletPair protocol            → btoa / atob            (canonical base64url frames)
 *   - services/image-decode      → Buffer                  (gallery-QR decode fallback)
 *
 * Without these, WalletPair (Vela Connect) pairing throws on the first scan and
 * gallery QR-import decoding silently fails — neither surfaces in the web build.
 *
 * This module MUST load before any of the above, so it is the very first import
 * in `src/app/_layout.tsx`.
 */

// Installs global.crypto.getRandomValues (also backs @noble's randomBytes).
import 'react-native-get-random-values';

// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore — base-64 ships no type declarations
import base64 from 'base-64';
import { Buffer } from 'buffer';

const g = globalThis as unknown as {
  btoa?: (s: string) => string;
  atob?: (s: string) => string;
  Buffer?: typeof Buffer;
};

if (typeof g.btoa !== 'function') g.btoa = base64.encode;
if (typeof g.atob !== 'function') g.atob = base64.decode;
if (typeof g.Buffer === 'undefined') g.Buffer = Buffer;

// Fail LOUD if the RNG polyfill didn't install `crypto.getRandomValues`: @noble captures
// `globalThis.crypto` at module-load, so a miss here means WalletPair pairing will throw
// "crypto.getRandomValues must be defined" (docs/KNOWN-BUGS.md BUG-4). This file loads
// first (via index.js, before any @noble import), so the check should always pass; the
// warning only fires if the native RNG module itself failed to install.
{
  const cg = (globalThis as { crypto?: { getRandomValues?: unknown } }).crypto;
  if (!cg || typeof cg.getRandomValues !== 'function') {
    console.error('[polyfills] globalThis.crypto.getRandomValues MISSING after react-native-get-random-values — WalletPair crypto will fail. Check the native RNG module.');
  }
}
