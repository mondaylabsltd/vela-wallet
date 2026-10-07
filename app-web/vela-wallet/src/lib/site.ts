/**
 * Canonical production origin for absolute URLs (hreflang/canonical need
 * fully-qualified hrefs at prerender time), and the one origin the hosted
 * build reports usage analytics from (`$lib/analytics/gate.ts`).
 *
 * The web wallet is served at `wallet.getvela.app` — the host the iOS and
 * Android pay-link parsers and the repository README already name.
 */
export const SITE_ORIGIN = 'https://wallet.getvela.app';
