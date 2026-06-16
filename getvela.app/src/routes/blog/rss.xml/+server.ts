import type { RequestHandler } from './$types';
import { getAllPosts } from '$lib/content/blog';
import { seoConfig } from '$lib/seo';
import { escapeXml } from '$lib/format';

export const prerender = true;

export const GET: RequestHandler = () => {
	const { domain, siteName } = seoConfig;
	const posts = getAllPosts();
	const lastBuild =
		posts.length > 0 ? new Date(`${posts[0].meta.date}T00:00:00Z`).toUTCString() : '';

	const items = posts
		.map((post) => {
			const url = `${domain}/blog/${post.slug}`;
			return `
		<item>
			<title>${escapeXml(post.meta.title)}</title>
			<link>${url}</link>
			<guid isPermaLink="true">${url}</guid>
			<pubDate>${new Date(`${post.meta.date}T00:00:00Z`).toUTCString()}</pubDate>
			<description>${escapeXml(post.meta.description)}</description>
			${(post.meta.tags ?? []).map((t) => `<category>${escapeXml(t)}</category>`).join('')}
		</item>`;
		})
		.join('');

	const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
	<channel>
		<title>${escapeXml(siteName)} Blog</title>
		<link>${domain}/blog</link>
		<atom:link href="${domain}/blog/rss.xml" rel="self" type="application/rss+xml" />
		<description>Build notes, release notes and the story behind ${escapeXml(siteName)}.</description>
		<language>en</language>
		${lastBuild ? `<lastBuildDate>${lastBuild}</lastBuildDate>` : ''}${items}
	</channel>
</rss>`;

	return new Response(xml, {
		headers: {
			'Content-Type': 'application/xml; charset=utf-8',
			'Cache-Control': 's-maxage=3600, stale-while-revalidate'
		}
	});
};
