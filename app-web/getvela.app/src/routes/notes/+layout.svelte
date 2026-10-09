<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import ReadingFonts from '$lib/components/ReadingFonts.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import { partsWithNotes } from '$lib/content/notes';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	// Every module and every note, always open, the way bun.sh/guides does it:
	// the whole section is one scroll of short labels, and the browser's own
	// find-in-page reaches all of them from any notes page.
	const parts = partsWithNotes();
	let menuOpen = $state(false);

	const activeModule = $derived(page.params.module);
	const activeSlug = $derived(page.params.slug);
</script>

<ReadingFonts />
<SiteHeader />

<div class="notes reading">
	<button class="sidebar-toggle" onclick={() => (menuOpen = !menuOpen)} aria-expanded={menuOpen}>
		<span class="bars" aria-hidden="true"></span>
		{menuOpen ? 'Hide all notes' : 'All notes'}
	</button>

	<aside class="sidebar" class:open={menuOpen}>
		<nav aria-label="Notes">
			<a class="overview" href={resolve('/notes')} class:active={!activeModule}>Overview</a>
			{#each parts as { part, modules } (part.key)}
				<p class="part">{part.title}</p>
				{#each modules as { module, notes } (module.slug)}
					<div class="module">
						<a
							class="module-link"
							href={resolve(`/notes/${module.slug}`)}
							class:active={activeModule === module.slug && !activeSlug}
							aria-current={activeModule === module.slug && !activeSlug ? 'page' : undefined}
							onclick={() => (menuOpen = false)}>{module.title}</a
						>
						<ul>
							{#each notes as note (note.id)}
								<li>
									<a
										href={resolve(`/notes/${note.module}/${note.slug}`)}
										class:active={activeModule === note.module && activeSlug === note.slug}
										aria-current={activeModule === note.module && activeSlug === note.slug
											? 'page'
											: undefined}
										onclick={() => (menuOpen = false)}>{note.nav}</a
									>
								</li>
							{/each}
						</ul>
					</div>
				{/each}
			{/each}
		</nav>
	</aside>

	<div class="content">
		{@render children()}
	</div>
</div>

<SiteFooter />

<style>
	.notes {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 0 24px;
		display: grid;
		grid-template-columns: 280px minmax(0, 1fr);
		gap: 48px;
		align-items: start;
	}
	.sidebar {
		position: sticky;
		top: var(--header-h);
		align-self: start;
		max-height: calc(100vh - var(--header-h));
		overflow-y: auto;
		padding: 32px 4px 48px 0;
		border-right: 1px solid var(--border);
	}
	nav a {
		display: block;
		border-radius: var(--radius-sm);
		transition:
			color 0.15s ease,
			background 0.15s ease;
	}
	.overview {
		padding: 6px 12px;
		margin-bottom: 6px;
		font-size: max(0.95rem, var(--floor-read));
		font-weight: 500;
		color: var(--text-secondary);
	}
	.part {
		margin: 22px 0 4px;
		padding: 0 12px;
		font-size: max(0.82rem, var(--floor-meta));
		color: var(--text-muted);
	}
	.module {
		margin-bottom: 12px;
	}
	.module-link {
		padding: 6px 12px;
		font-size: max(0.95rem, var(--floor-read));
		font-weight: 500;
		color: var(--text-secondary);
	}
	ul {
		list-style: none;
		margin-left: 12px;
		border-left: 1px solid var(--border);
	}
	ul a {
		margin-left: -1px;
		padding: 5px 12px 5px 14px;
		border-left: 1px solid transparent;
		border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
		font-size: max(0.9rem, var(--floor-note));
		line-height: 1.4;
		color: var(--text-muted);
	}
	nav a:hover {
		color: var(--text);
		background: var(--bg-raised);
	}
	/* Where you are is shown in ink, not in orange. */
	nav a.active {
		color: var(--text);
		background: var(--bg-raised);
	}
	ul a.active {
		border-left-color: var(--text-secondary);
	}
	.content {
		min-width: 0;
		padding: 36px 0 0;
	}

	.sidebar-toggle {
		display: none;
		align-items: center;
		gap: 8px;
		margin: 16px 0 0;
		padding: 9px 16px;
		background: var(--bg-card);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		color: var(--text);
		font-size: max(0.9rem, var(--floor-note));
		font-weight: 500;
		cursor: pointer;
		justify-self: start;
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

	@media (max-width: 960px) {
		.notes {
			grid-template-columns: minmax(0, 1fr);
			gap: 0;
			padding: 0 16px;
		}
		.sidebar-toggle {
			display: inline-flex;
		}
		.sidebar {
			position: static;
			max-height: none;
			overflow: visible;
			padding: 12px 0 8px;
			margin-bottom: 8px;
			border-right: 0;
			border-bottom: 1px solid var(--border);
			display: none;
		}
		.sidebar.open {
			display: block;
		}
		.content {
			padding-top: 20px;
		}
	}
</style>
