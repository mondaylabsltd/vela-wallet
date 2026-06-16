<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Snippet } from 'svelte';
	import Prose from './Prose.svelte';
	import Toc from './Toc.svelte';
	import { getAdjacentDocs, getDocEditUrl } from '$lib/content/docs';
	import { DOCS_INDEX_SLUG } from '$lib/content/sidebar';

	let { slug, children }: { slug: string; children: Snippet } = $props();

	const adjacent = $derived(getAdjacentDocs(slug));
	const editUrl = $derived(getDocEditUrl(slug));

	function hrefFor(target: string) {
		return target === DOCS_INDEX_SLUG ? resolve('/docs') : resolve(`/docs/${target}`);
	}
</script>

<div class="doc-layout">
	<article class="doc-body">
		<Prose>
			{@render children()}
		</Prose>

		<div class="doc-edit">
			<a href={editUrl} target="_blank" rel="noopener">
				<svg
					width="15"
					height="15"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"
				>
					<path d="M12 20h9" />
					<path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
				</svg>
				Edit this page on GitHub
			</a>
		</div>

		{#if adjacent.prev || adjacent.next}
			<nav class="pager" aria-label="Docs pages">
				{#if adjacent.prev}
					<a class="pager-link" href={hrefFor(adjacent.prev.slug)}>
						<span class="dir">← Previous</span>
						<span class="t">{adjacent.prev.title}</span>
					</a>
				{:else}
					<span></span>
				{/if}
				{#if adjacent.next}
					<a class="pager-link right" href={hrefFor(adjacent.next.slug)}>
						<span class="dir">Next →</span>
						<span class="t">{adjacent.next.title}</span>
					</a>
				{/if}
			</nav>
		{/if}
	</article>

	<aside class="doc-toc">
		<Toc containerSelector=".doc-body" />
	</aside>
</div>

<style>
	.doc-layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) var(--toc-w);
		gap: 48px;
		align-items: start;
	}
	.doc-body {
		min-width: 0;
		padding: 8px 0 24px;
	}
	.doc-edit {
		margin-top: 40px;
		padding-top: 22px;
		border-top: 1px solid var(--border);
	}
	.doc-edit a {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		font-size: 0.88rem;
		color: var(--text-secondary);
		transition: color 0.15s ease;
	}
	.doc-edit a:hover {
		color: var(--accent);
	}
	.doc-edit svg {
		opacity: 0.85;
	}
	.pager {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin-top: 24px;
	}
	.pager-link {
		display: flex;
		flex-direction: column;
		gap: 5px;
		padding: 16px 18px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-card);
		transition: border-color 0.15s ease;
	}
	.pager-link:hover {
		border-color: var(--border-strong);
	}
	.pager-link.right {
		text-align: right;
	}
	.pager-link .dir {
		font-size: 0.76rem;
		color: var(--text-muted);
	}
	.pager-link .t {
		font-size: 0.95rem;
		font-weight: 600;
		color: var(--text);
	}
	.doc-toc {
		display: block;
	}

	@media (max-width: 1024px) {
		.doc-layout {
			grid-template-columns: minmax(0, 1fr);
		}
		.doc-toc {
			display: none;
		}
	}
	@media (max-width: 560px) {
		.pager {
			grid-template-columns: 1fr;
		}
		.pager-link.right {
			text-align: left;
		}
	}
</style>
