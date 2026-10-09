<script lang="ts">
	import { resolve } from '$app/paths';
	import Seo from '$lib/components/Seo.svelte';
	import { notesIn } from '$lib/content/notes';
	import { getNoteModule } from '$lib/content/notes-modules';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const module = $derived(getNoteModule(data.module)!);
	const notes = $derived(notesIn(data.module));

	const jsonLd = $derived([
		{
			'@context': 'https://schema.org',
			'@type': 'CollectionPage',
			name: `${module.title} — Vela Wallet notes`,
			url: `${seoConfig.domain}/notes/${module.slug}`,
			description: module.intro
		},
		{
			'@context': 'https://schema.org',
			'@type': 'BreadcrumbList',
			itemListElement: [
				{ '@type': 'ListItem', position: 1, name: 'Notes', item: `${seoConfig.domain}/notes` },
				{
					'@type': 'ListItem',
					position: 2,
					name: module.title,
					item: `${seoConfig.domain}/notes/${module.slug}`
				}
			]
		}
	]);
</script>

<Seo
	title={`${module.title} — Notes`}
	description={module.intro}
	canonical={`/notes/${module.slug}`}
	{jsonLd}
/>

<main class="module">
	<nav class="crumbs" aria-label="Breadcrumb">
		<a href={resolve('/notes')}>Notes</a>
	</nav>
	<h1>{module.title}</h1>
	<p class="intro">{module.intro}</p>

	<ol class="list">
		{#each notes as note (note.id)}
			<li>
				<a href={resolve(`/notes/${note.module}/${note.slug}`)}>
					<span class="t">{note.title}</span>
					<span class="d">{note.description}</span>
				</a>
			</li>
		{/each}
	</ol>
</main>

<style>
	.module {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding-bottom: 48px;
	}
	.crumbs {
		font-size: max(0.86rem, var(--floor-note));
		margin-bottom: 14px;
	}
	.crumbs a {
		color: var(--text-secondary);
	}
	.crumbs a:hover {
		color: var(--text);
	}
	h1 {
		font-size: clamp(2.2rem, 1.8rem + 1.4vw, 2.75rem);
		line-height: 1.06;
		font-weight: 800;
		letter-spacing: -0.02em;
		margin-bottom: 10px;
	}
	.intro {
		font-size: 1.1rem;
		line-height: 1.6;
		color: var(--text-secondary);
		margin-bottom: 28px;
	}
	.list {
		list-style: none;
	}
	.list li {
		border-top: 1px solid var(--border);
	}
	.list li:last-child {
		border-bottom: 1px solid var(--border);
	}
	.list a {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 16px 0;
	}
	.list .t {
		font-size: 1.05rem;
		font-weight: 600;
		line-height: 1.4;
		color: var(--text);
	}
	.list .d {
		font-size: max(0.92rem, var(--floor-note));
		line-height: 1.55;
		color: var(--text-secondary);
	}
	.list a:hover .t {
		color: var(--accent);
	}

	@media (max-width: 560px) {
		h1 {
			font-size: 1.8rem;
		}
	}
</style>
