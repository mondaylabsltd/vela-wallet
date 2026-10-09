<script lang="ts">
	import { resolve } from '$app/paths';
	import Seo from '$lib/components/Seo.svelte';
	import { featuredNotes, getNotes, partsWithNotes } from '$lib/content/notes';
	import { seoConfig } from '$lib/seo';

	// Every title on one page, as plain text, the way bun.sh/guides does it: the
	// whole design can be skimmed in one scroll, and the browser's own
	// find-in-page reaches every note from here.
	const parts = partsWithNotes();
	const featured = featuredNotes();
	const total = getNotes().length;

	const description =
		'Short notes on why Vela Wallet is built the way it is, and how to use it well: settings, fees, security, and the reason behind each decision.';

	const jsonLd = {
		'@context': 'https://schema.org',
		'@type': 'CollectionPage',
		name: 'Vela Wallet notes',
		url: `${seoConfig.domain}/notes`,
		description,
		hasPart: parts.flatMap(({ modules }) =>
			modules.map(({ module }) => ({
				'@type': 'CollectionPage',
				name: module.title,
				url: `${seoConfig.domain}/notes/${module.slug}`
			}))
		)
	};
</script>

<Seo title="Notes" {description} canonical="/notes" {jsonLd} />

<main class="index">
	<header class="intro">
		<h1>Notes</h1>
		<p>
			Why Vela is built the way it is, and how to use it well. Each note answers one question in a
			minute or two, and links to the code it describes.
		</p>
		{#if total === 0}
			<p class="empty">The first notes are being written.</p>
		{/if}
	</header>

	{#if featured.length}
		<section class="featured" aria-labelledby="featured-h">
			<h2 id="featured-h" class="label">Start here</h2>
			<ul>
				{#each featured as note (note.id)}
					<li>
						<a href={resolve(`/notes/${note.module}/${note.slug}`)}>
							<span class="t">{note.title}</span>
							<span class="arrow" aria-hidden="true">→</span>
						</a>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#each parts as { part, modules } (part.key)}
		<section class="part" aria-label={part.title}>
			<p class="label">{part.title}</p>
			{#each modules as { module, notes } (module.slug)}
				<div class="module">
					<h2><a href={resolve(`/notes/${module.slug}`)}>{module.title}</a></h2>
					<p class="module-intro">{module.intro}</p>
					<ul>
						{#each notes as note (note.id)}
							<li>
								<a href={resolve(`/notes/${note.module}/${note.slug}`)}>{note.title}</a>
								{#if note.draft}<span class="draft">draft</span>{/if}
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</section>
	{/each}
</main>

<style>
	.index {
		max-width: 980px;
		margin: 0 auto;
		padding-bottom: 48px;
	}
	.intro {
		margin-bottom: 44px;
	}
	.intro h1 {
		font-size: clamp(2.6rem, 2rem + 2vw, 3.4rem);
		line-height: 1.02;
		font-weight: 800;
		letter-spacing: -0.02em;
		margin-bottom: 14px;
	}
	.intro p {
		max-width: 620px;
		font-size: 1.15rem;
		line-height: 1.6;
		color: var(--text-secondary);
	}
	.intro .empty {
		margin-top: 12px;
		font-size: 1rem;
		color: var(--text-muted);
	}
	.label {
		font-family: var(--font-system);
		font-stretch: 100%;
		letter-spacing: 0;
		font-size: max(0.9rem, var(--floor-meta));
		font-weight: 500;
		color: var(--text-muted);
		margin-bottom: 12px;
	}
	.featured {
		margin-bottom: 52px;
	}
	.featured ul {
		list-style: none;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
		gap: 14px;
	}
	.featured a {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 16px;
		height: 100%;
		padding: 16px 18px;
		border-radius: var(--radius);
		background: color-mix(in srgb, var(--text) 4%, var(--bg));
		transition: background 0.15s ease;
	}
	.featured a:hover {
		background: color-mix(in srgb, var(--text) 7%, var(--bg));
	}
	.featured .t {
		font-weight: 600;
		line-height: 1.35;
		color: var(--text);
	}
	.featured .arrow {
		color: var(--text-muted);
		flex: none;
	}
	.part {
		margin-bottom: 40px;
	}
	.module {
		margin-bottom: 36px;
	}
	.module h2 {
		font-size: 1.75rem;
		line-height: 1.15;
		font-weight: 750;
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border);
		margin-bottom: 8px;
	}
	.module h2 a {
		color: var(--text);
	}
	.module h2 a:hover {
		color: var(--text-secondary);
	}
	.module-intro {
		font-size: max(0.95rem, var(--floor-note));
		line-height: 1.5;
		color: var(--text-muted);
		margin-bottom: 6px;
	}
	.module ul {
		list-style: none;
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		column-gap: 40px;
	}
	.module li {
		border-bottom: 1px solid var(--border);
	}
	.module li a {
		display: inline-block;
		padding: 11px 0;
		font-size: max(1rem, var(--floor-read));
		line-height: 1.45;
		color: var(--text-secondary);
	}
	.module li a:hover {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.draft {
		margin-left: 6px;
		font-size: 0.7rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-muted);
	}

	@media (max-width: 720px) {
		.module ul {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
