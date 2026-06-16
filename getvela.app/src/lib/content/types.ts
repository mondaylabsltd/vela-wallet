import type { Component } from 'svelte';

/** Frontmatter for a blog post (`src/content/blog/<slug>.md`). */
export interface BlogMeta {
	title: string;
	description: string;
	/** ISO date, e.g. "2026-06-12". */
	date: string;
	author?: string;
	tags?: string[];
	/** Optional cover image path (served from /static or an absolute URL). */
	cover?: string;
	/** Set to true to keep a post out of the list, RSS and sitemap. */
	draft?: boolean;
}

/** Frontmatter for a docs page (`src/content/docs/<slug>.md`). */
export interface DocMeta {
	title: string;
	description?: string;
}

/** A compiled markdown module as produced by mdsvex. */
export interface MarkdownModule<M> {
	default: Component;
	metadata: M;
}
