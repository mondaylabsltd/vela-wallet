/**
 * Which social-preview cards the site has, and what each says — read from the
 * content files at build time (vite.config.ts), never at request time.
 *
 * One card per page that has its own words: each blog post, each note, and
 * each docs page in every language it is written in. A card lives at
 * `/og/<page path>.png`; a docs page shown in English because its translation
 * is missing points at the English card (docs pages set `image` themselves).
 * The text matches what the page shows (og-cards.test.ts holds every card to
 * the compiled content).
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { formatDate } from '../format';
import { DEFAULT_LOCALE, pathFor, SUPPORTED_LOCALES, type Locale } from '../i18n/locales';
import en from '../i18n/messages/en';
import { seoConfig } from '../seo';
import { splitNote, toEntry } from './notes-front';
import { docHref } from './sidebar';

export interface OgCardEntry {
	/** The page path without its leading slash: `blog/<slug>`, `zh/docs/<slug>`, … */
	path: string;
	/** The page's language: picks the fallback fonts (scripts/og/fonts/). */
	lang: Locale;
	site: string;
	domain: string;
	section: string;
	title: string;
	description: string;
	meta: string;
}

const site = seoConfig.siteName;
const domain = seoConfig.domain.replace(/^https?:\/\//, '');

function markdown(dir: string): string[] {
	return existsSync(dir)
		? readdirSync(dir)
				.filter((f) => f.endsWith('.md'))
				.sort()
		: [];
}

function front(file: string): (key: string) => string {
	const { front } = splitNote(readFileSync(file, 'utf8'));
	return (k) => (typeof front[k] === 'string' ? (front[k] as string) : '');
}

/** The header's own label for a section, in `locale` (falls back to English). */
function navLabel(i18nDir: string, locale: Locale, key: 'docs' | 'blog' | 'notes'): string {
	if (locale !== DEFAULT_LOCALE) {
		const file = join(i18nDir, `${locale}.json`);
		if (existsSync(file)) {
			const label = JSON.parse(readFileSync(file, 'utf8'))?.chrome?.nav?.[key];
			if (typeof label === 'string' && label) return label;
		}
	}
	return en.chrome.nav[key];
}

/** A docs page's path in `locale`. */
export function docsPath(locale: Locale, slug: string): string {
	return pathFor(locale, docHref(slug));
}

/**
 * Every card; drafts only when asked (under `vite dev`, where they render).
 * `appDir` is the app root (it holds src/content and src/lib/i18n).
 */
export function ogCards(appDir: string, includeDrafts: boolean): OgCardEntry[] {
	const contentDir = join(appDir, 'src/content');
	const i18nDir = join(appDir, 'src/lib/i18n/messages');
	const base = { site, domain };
	const cards: OgCardEntry[] = [];

	for (const file of markdown(join(contentDir, 'blog'))) {
		const str = front(join(contentDir, 'blog', file));
		if (!str('title') || !str('date')) continue;
		if (str('draft') === 'true' && !includeDrafts) continue;
		const date = str('date').slice(0, 10);
		// The post page's own default author (blog/[slug]/+page.svelte).
		const author = str('author') || 'Vela';
		cards.push({
			...base,
			path: `blog/${file.replace(/\.md$/, '')}`,
			lang: DEFAULT_LOCALE,
			section: en.chrome.nav.blog,
			title: str('title'),
			description: str('description'),
			meta: `${author} · ${formatDate(date)}`
		});
	}

	const notesDir = join(contentDir, 'notes');
	if (existsSync(notesDir)) {
		for (const dir of readdirSync(notesDir, { withFileTypes: true })) {
			if (!dir.isDirectory()) continue;
			for (const file of markdown(join(notesDir, dir.name))) {
				const { front } = splitNote(readFileSync(join(notesDir, dir.name, file), 'utf8'));
				const entry = toEntry(dir.name, file.replace(/\.md$/, ''), front);
				if (!entry.title || (entry.draft && !includeDrafts)) continue;
				cards.push({
					...base,
					path: `notes/${entry.id}`,
					lang: DEFAULT_LOCALE,
					section: en.chrome.nav.notes,
					title: entry.title,
					description: entry.description,
					meta: ''
				});
			}
		}
	}

	const docsDir = join(contentDir, 'docs');
	for (const locale of SUPPORTED_LOCALES) {
		const dir = locale === DEFAULT_LOCALE ? docsDir : join(docsDir, locale);
		for (const file of markdown(dir)) {
			const str = front(join(dir, file));
			if (!str('title')) continue;
			const slug = file.replace(/\.md$/, '');
			cards.push({
				...base,
				path: docsPath(locale, slug).slice(1),
				lang: locale,
				section: navLabel(i18nDir, locale, 'docs'),
				title: str('title'),
				description: str('description'),
				meta: ''
			});
		}
	}
	return cards;
}

/** Every character a locale's cards draw: what its font subsets must hold. */
export function cardText(cards: OgCardEntry[], lang: Locale): string {
	return cards
		.filter((c) => c.lang === lang)
		.flatMap((c) => [
			c.site.toUpperCase(),
			c.section.toUpperCase(),
			c.title,
			c.description,
			c.meta,
			c.domain
		])
		.concat('…')
		.join('\n')
		.normalize('NFC');
}
