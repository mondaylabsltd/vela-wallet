import type { Component } from 'svelte';
import type { DocMeta, MarkdownModule } from './types';
import { flatSidebar, DOCS_INDEX_SLUG, type SidebarItem } from './sidebar';
import { seoConfig } from '$lib/seo';

/** Path (relative to the app root) where docs markdown lives. */
const DOCS_CONTENT_DIR = 'src/content/docs';

const modules = import.meta.glob<MarkdownModule<DocMeta>>('/src/content/docs/*.md', {
	eager: true
});

function slugFromPath(path: string): string {
	return path.split('/').pop()!.replace(/\.md$/, '');
}

const bySlug = new Map<string, MarkdownModule<DocMeta>>();
for (const [path, mod] of Object.entries(modules)) {
	bySlug.set(slugFromPath(path), mod);
}

export interface DocEntry {
	slug: string;
	meta: DocMeta;
	component: Component;
}

export function getDoc(slug: string): DocEntry | undefined {
	const mod = bySlug.get(slug);
	if (!mod) return undefined;
	return { slug, meta: mod.metadata, component: mod.default };
}

/** Previous/next docs in sidebar order, for the footer pager. */
export function getAdjacentDocs(slug: string): { prev?: SidebarItem; next?: SidebarItem } {
	const index = flatSidebar.findIndex((item) => item.slug === slug);
	if (index === -1) return {};
	return { prev: flatSidebar[index - 1], next: flatSidebar[index + 1] };
}

/** Slugs handled by the `/docs/[...slug]` route (everything but the index). */
export function getDocSlugs(): string[] {
	return flatSidebar.map((item) => item.slug).filter((slug) => slug !== DOCS_INDEX_SLUG);
}

/** GitHub "edit this file" URL for a docs page's source markdown. */
export function getDocEditUrl(slug: string): string {
	const { github, repoBranch, repoAppDir } = seoConfig;
	return `${github}/edit/${repoBranch}/${repoAppDir}/${DOCS_CONTENT_DIR}/${slug}.md`;
}
