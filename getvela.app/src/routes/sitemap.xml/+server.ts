import type { RequestHandler } from './$types';
import { getAllPosts } from '$lib/content/blog';
import { flatSidebar, docHref } from '$lib/content/sidebar';
import { seoConfig } from '$lib/seo';

export const prerender = true;

interface UrlEntry {
	path: string;
	lastmod?: string;
	changefreq?: string;
	priority?: string;
}

export const GET: RequestHandler = () => {
	const { domain } = seoConfig;

	const staticPages: UrlEntry[] = [
		{ path: '/', changefreq: 'weekly', priority: '1.0' },
		{ path: '/about', changefreq: 'monthly', priority: '0.7' },
		{ path: '/blog', changefreq: 'weekly', priority: '0.8' },
		{ path: '/docs', changefreq: 'weekly', priority: '0.8' },
		{ path: '/privacy', changefreq: 'yearly', priority: '0.3' },
		{ path: '/terms', changefreq: 'yearly', priority: '0.3' }
	];

	const docPages: UrlEntry[] = flatSidebar
		.filter((item) => item.slug !== 'introduction')
		.map((item) => ({ path: docHref(item.slug), changefreq: 'monthly', priority: '0.6' }));

	const blogPages: UrlEntry[] = getAllPosts().map((post) => ({
		path: `/blog/${post.slug}`,
		lastmod: post.meta.date,
		changefreq: 'monthly',
		priority: '0.7'
	}));

	const urls = [...staticPages, ...docPages, ...blogPages];

	const body = urls
		.map(
			(u) => `	<url>
		<loc>${domain}${u.path}</loc>${u.lastmod ? `\n\t\t<lastmod>${u.lastmod}</lastmod>` : ''}
		<changefreq>${u.changefreq}</changefreq>
		<priority>${u.priority}</priority>
	</url>`
		)
		.join('\n');

	const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${body}
</urlset>`;

	return new Response(xml, {
		headers: {
			'Content-Type': 'application/xml; charset=utf-8',
			'Cache-Control': 's-maxage=3600, stale-while-revalidate'
		}
	});
};
