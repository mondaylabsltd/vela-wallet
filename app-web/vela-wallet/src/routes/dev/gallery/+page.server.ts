/**
 * Dev-only onboarding state gallery (spec 014, contract §4).
 *
 * Gate: dev server only — a production build 404s (FR-013). Strings come
 * from the generated corpus JSON (`assets/i18n/*.json`) read RAW here; the
 * wasm i18n engine (`engine.server.ts`) is deliberately NOT imported
 * anywhere under this route so it can never poison the worker bundle
 * (research D4).
 */
import { dev } from '$app/environment';
import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const prerender = false;

/** Same artifact the engine loads — copy verification is not weakened. */
const CATALOGS = import.meta.glob('../../../../../../assets/i18n/*.json', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

const GALLERY_LOCALES = ['zh', 'en'] as const;

export type GalleryLocale = (typeof GALLERY_LOCALES)[number];

export const load: PageServerLoad = () => {
	if (!dev) error(404);

	const catalogs: Record<string, unknown> = {};
	for (const locale of GALLERY_LOCALES) {
		const entry = Object.entries(CATALOGS).find(([path]) => path.endsWith(`/${locale}.json`));
		// Defensive: a missing catalog must not break the gallery — the
		// resolver falls back to the key string (new keys may still be
		// landing in the corpus in parallel).
		catalogs[locale] = entry ? JSON.parse(entry[1]) : {};
	}
	return { catalogs, locales: [...GALLERY_LOCALES] };
};
