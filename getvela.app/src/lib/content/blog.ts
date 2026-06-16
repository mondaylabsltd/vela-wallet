import type { Component } from 'svelte';
import type { BlogMeta, MarkdownModule } from './types';

/**
 * All blog posts, eagerly compiled by mdsvex at build time. Eager loading keeps
 * the list page, RSS feed, sitemap and post pages working off a single source
 * with no async plumbing — the compiled markdown is small and prerendered.
 */
const modules = import.meta.glob<MarkdownModule<BlogMeta>>('/src/content/blog/*.md', {
	eager: true
});

/** Raw source of each post, used only to estimate reading time. */
const sources = import.meta.glob<string>('/src/content/blog/*.md', {
	eager: true,
	query: '?raw',
	import: 'default'
});

export interface BlogPost {
	slug: string;
	meta: BlogMeta;
	component: Component;
	readingMinutes: number;
}

function slugFromPath(path: string): string {
	return path.split('/').pop()!.replace(/\.md$/, '');
}

function estimateReadingMinutes(raw: string): number {
	const body = raw.replace(/^---[\s\S]*?---/, ''); // drop frontmatter
	const words = body.trim().split(/\s+/).filter(Boolean).length;
	return Math.max(1, Math.round(words / 200));
}

/**
 * Normalize a frontmatter date to the documented `YYYY-MM-DD` form. YAML parses
 * an unquoted `date: 2026-06-12` as a Date, which mdsvex then serializes to a
 * full ISO timestamp ("2026-06-12T00:00:00.000Z"). Downstream code (formatDate,
 * the RSS feed) anchors the date by appending `T00:00:00Z`, which turns that
 * already-complete timestamp into an invalid string. Trimming to the date stem
 * here keeps `meta.date` consistent however the frontmatter was written.
 */
function toIsoDate(value: string): string {
	return String(value).slice(0, 10);
}

const allPosts: BlogPost[] = Object.entries(modules)
	.filter(([path, mod]) => {
		// A post with broken/missing frontmatter compiles without `metadata`.
		// Skip it (with a build-time warning) rather than crash the whole build —
		// the usual cause is an unquoted ":" in a YAML value.
		if (!mod.metadata?.title || !mod.metadata?.date) {
			console.warn(
				`[blog] Skipping ${path}: missing frontmatter (needs at least "title" and "date"). ` +
					`If a value contains a colon, wrap it in quotes.`
			);
			return false;
		}
		return true;
	})
	.map(([path, mod]) => ({
		slug: slugFromPath(path),
		meta: { ...mod.metadata, date: toIsoDate(mod.metadata.date) },
		component: mod.default,
		readingMinutes: estimateReadingMinutes(sources[path] ?? '')
	}));

const publishedPosts = allPosts
	.filter((p) => !p.meta.draft)
	.sort((a, b) => (a.meta.date < b.meta.date ? 1 : -1));

/** Published posts, newest first. */
export function getAllPosts(): BlogPost[] {
	return publishedPosts;
}

/** Look up a single post by slug (includes drafts so previews still resolve). */
export function getPost(slug: string): BlogPost | undefined {
	return allPosts.find((p) => p.slug === slug);
}

/** Slugs to prerender. */
export function getAllPostSlugs(): string[] {
	return publishedPosts.map((p) => p.slug);
}

/** Newer/older posts for the in-article pager (posts are newest-first). */
export function getAdjacentPosts(slug: string): { newer?: BlogPost; older?: BlogPost } {
	const index = publishedPosts.findIndex((p) => p.slug === slug);
	if (index === -1) return {};
	return { newer: publishedPosts[index - 1], older: publishedPosts[index + 1] };
}
