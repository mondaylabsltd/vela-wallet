// Custom app entry — its ONLY job is to install the native polyfills BEFORE
// expo-router registers the app (and thus before any @noble/* module evaluates).
//
// Why (docs/KNOWN-BUGS.md BUG-4): `@noble/hashes/crypto` captures `globalThis.crypto`
// ONCE, at module evaluation —
//     exports.crypto = 'crypto' in globalThis ? globalThis.crypto : undefined
// If it runs before `react-native-get-random-values` has created `globalThis.crypto`,
// noble holds `undefined` forever and WalletPair's X25519 keygen throws
// "crypto.getRandomValues must be defined" on the first pairing. Importing the polyfills
// here — the very first thing the bundle runs — guarantees `globalThis.crypto` exists
// before ANY app / router / @noble code, so it wins the race unconditionally.
//
// Relative import on purpose: babel path aliases (`@/…`) don't apply to `package.json`
// `main`-field resolution (see node_modules/expo-router/entry.js).
import './src/polyfills';
import 'expo-router/entry';
