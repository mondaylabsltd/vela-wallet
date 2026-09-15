<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import { DOCS_INDEX_SLUG, sidebar } from '$lib/content/sidebar';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	let menuOpen = $state(false);

	function hrefFor(slug: string) {
		return slug === DOCS_INDEX_SLUG ? resolve('/docs') : resolve(`/docs/${slug}`);
	}
	function isActive(slug: string) {
		const target = slug === DOCS_INDEX_SLUG ? '/docs' : `/docs/${slug}`;
		return page.url.pathname === target;
	}
</script>

<SiteHeader />

<div class="docs">
	<button
		class="sidebar-toggle"
		onclick={() => (menuOpen = !menuOpen)}
		aria-expanded={menuOpen}
	>
		<span class="bars" aria-hidden="true"></span>
		{menuOpen ? 'Hide' : 'Browse'} docs
	</button>

	<aside class="sidebar" class:open={menuOpen}>
		<nav aria-label="Documentation">
			{#each sidebar as group (group.title)}
				<div class="group">
					<p class="group-title">{group.title}</p>
					<ul>
						{#each group.items as item (item.slug)}
							<li>
								<a
									href={hrefFor(item.slug)}
									class:active={isActive(item.slug)}
									aria-current={isActive(item.slug) ? 'page' : undefined}
									onclick={() => (menuOpen = false)}
								>
									{item.title}
								</a>
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</nav>
	</aside>

	<div class="content">
		{@render children()}
	</div>
</div>

<SiteFooter />

<style>
	.docs {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 0 24px;
		display: grid;
		grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
		gap: 24px;
		align-items: start;
	}
	.sidebar {
		position: sticky;
		top: var(--header-h);
		align-self: start;
		max-height: calc(100vh - var(--header-h));
		overflow-y: auto;
		padding: 32px 0 48px;
	}
	.group {
		margin-bottom: 26px;
	}
	.group-title {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-muted);
		font-weight: 600;
		margin-bottom: 10px;
	}
	.group ul {
		list-style: none;
	}
	.group li a {
		display: block;
		padding: 6px 12px;
		margin: 1px 0;
		border-radius: var(--radius-sm);
		font-size: 0.92rem;
		color: var(--text-secondary);
		transition:
			color 0.15s ease,
			background 0.15s ease;
	}
	.group li a:hover {
		color: var(--text);
		background: var(--bg-raised);
	}
	.group li a.active {
		color: var(--accent);
		background: var(--accent-soft);
		font-weight: 500;
	}
	.content {
		min-width: 0;
		padding: 32px 0 0;
	}

	.sidebar-toggle {
		display: none;
		align-items: center;
		gap: 8px;
		margin: 20px 0 0;
		padding: 9px 16px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		color: var(--text);
		font-size: 0.9rem;
		font-weight: 500;
		cursor: pointer;
	}
	.sidebar-toggle .bars {
		width: 16px;
		height: 10px;
		border-top: 2px solid currentColor;
		border-bottom: 2px solid currentColor;
		position: relative;
	}
	.sidebar-toggle .bars::after {
		content: '';
		position: absolute;
		top: 3px;
		left: 0;
		right: 0;
		height: 2px;
		background: currentColor;
	}

	@media (max-width: 860px) {
		.docs {
			grid-template-columns: minmax(0, 1fr);
			gap: 0;
		}
		.sidebar-toggle {
			display: inline-flex;
		}
		.sidebar {
			position: static;
			max-height: none;
			overflow: visible;
			padding: 16px 0 8px;
			margin-bottom: 8px;
			border-bottom: 1px solid var(--border);
			display: none;
		}
		.sidebar.open {
			display: block;
		}
		.content {
			padding-top: 16px;
		}
	}
</style>
