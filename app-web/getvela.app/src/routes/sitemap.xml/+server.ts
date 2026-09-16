import type { RequestHandler } from './$types';
import { getAllPosts } from '$lib/content/blog';
import { ENGLISH_ONLY_PAGES, localizedPages, urlsFor } from '$lib/i18n/urls';
import { pathFor } from '$lib/i18n/locales';
import { seoConfig } from '$lib/seo';

export const prerender = true;

/**
 * The sitemap (spec 059, contracts/head-and-sitemap.md).
 *
 * Two rules it exists to keep:
 *
 *  1. **It lists what exists, not what is supported.** A localized URL appears
 *     only when that locale genuinely has that page — `localizedPages()` reads
 *     the same translation record the router and the `hreflang` block read, so
 *     the three cannot disagree (FR-025).
 *  2. **Every entry carries its alternates.** Fifteen versions of one page
 *     without `xhtml:link` annotations are fifteen duplicate-content candidates;
 *     with them they are one page in fifteen languages.
 */
export const GET: RequestHandler = () => {
	const { domain } = seoConfig;

	const blocks: string[] = [];

	for (const page of localizedPages()) {
		const alternates = urlsFor(page)
			.map(
				(u) =>
					`\t\t<xhtml:link rel="alternate" hreflang="${u.locale}" href="${domain}${u.path}"/>`
			)
			.join('\n');
		const xDefault = `\t\t<xhtml:link rel="alternate" hreflang="x-default" href="${domain}${page.englishPath}"/>`;

		for (const { locale } of urlsFor(page)) {
			blocks.push(`	<url>
		<loc>${domain}${pathFor(locale, page.englishPath)}</loc>
${alternates}
${xDefault}
		<changefreq>${page.changefreq}</changefreq>
		<priority>${page.priority}</priority>
	</url>`);
		}
	}

	// English-only pages (R5): one URL each, no alternates to declare.
	for (const page of ENGLISH_ONLY_PAGES) {
		blocks.push(`	<url>
		<loc>${domain}${page.path}</loc>
		<changefreq>${page.changefreq}</changefreq>
		<priority>${page.priority}</priority>
	</url>`);
	}

	for (const post of getAllPosts()) {
		blocks.push(`	<url>
		<loc>${domain}/blog/${post.slug}</loc>
		<lastmod>${post.meta.date}</lastmod>
		<changefreq>monthly</changefreq>
		<priority>0.7</priority>
	</url>`);
	}

	const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${blocks.join('\n')}
</urlset>`;

	return new Response(xml, {
		headers: {
			'Content-Type': 'application/xml; charset=utf-8',
			'Cache-Control': 's-maxage=3600, stale-while-revalidate'
		}
	});
};
