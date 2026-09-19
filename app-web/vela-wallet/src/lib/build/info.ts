/**
 * Which build this is — the two facts "About" shows and a bug report quotes.
 *
 * They come from the BUILD, never from a constant in a fixture: until spec 064
 * every web and extension build said `v1.0.0 (6ab8f)`, the design mock's
 * numbers, whatever had actually shipped (0.9.2 did). `vite.config.ts` fills
 * both in — see `buildIdentity()` there for where each comes from and in what
 * order. `unknown` is a legitimate answer; a plausible-looking constant is not.
 */
export const BUILD_VERSION: string = __VELA_VERSION__;
export const BUILD_COMMIT: string = __VELA_COMMIT__;

/** The mock's numbers. Named so a test can insist a live build never shows them. */
export const MOCK_VERSION = '1.0.0';
export const MOCK_COMMIT = '6ab8f';
