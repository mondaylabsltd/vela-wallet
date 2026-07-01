// React Native / Expo define __DEV__ as a global at runtime; the jest node
// environment does not. Define it so modules that branch on it (e.g. the dev-only
// passkey override seam in modules/passkey) run without a ReferenceError. Tests run
// as a dev build.
globalThis.__DEV__ = true;
