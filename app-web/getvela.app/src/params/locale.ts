import { PREFIXED_LOCALES } from '$lib/i18n/locales';
import type { ParamMatcher } from '@sveltejs/kit';

/**
 * The `[[locale=locale]]` route param (spec 059, contracts/routing.md).
 *
 * Matches the FOURTEEN prefixed tags in canonical case, and nothing else:
 *
 *  - `en` is rejected on purpose. English lives at the unprefixed root, so if
 *    `/en/docs` also resolved, every English page would have two URLs and the
 *    pair would compete in search (FR-008).
 *  - A mis-cased or aliased tag (`/pt-br/`, `/pt/`, `/zh-CN/`) is rejected here
 *    too. It is not a page — `hooks.server.ts` 308s it to the canonical tag, so
 *    one page keeps one URL (contracts/routing.md §Must 308).
 *  - Anything else — `/blog`, `/about`, `/xx` — is not a locale, so the router
 *    falls through to the real route or to 404 (FR-010).
 */
const PREFIXES = new Set<string>(PREFIXED_LOCALES);

export const match: ParamMatcher = (param) => PREFIXES.has(param);
