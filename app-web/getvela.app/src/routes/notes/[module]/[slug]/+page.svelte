<script lang="ts">
	import { resolve } from '$app/paths';
	import Prose from '$lib/components/Prose.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { adjacentNotes, relatedNotes, sourceLink } from '$lib/content/notes';
	import { getNoteModule } from '$lib/content/notes-modules';
	import { flatSidebar, DOCS_INDEX_SLUG } from '$lib/content/sidebar';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const note = $derived(data.note);
	const Body = $derived(data.body);
	const module = $derived(getNoteModule(note.module)!);
	const related = $derived(relatedNotes(note));
	const adjacent = $derived(adjacentNotes(note));
	const sources = $derived(note.sources.map((s) => sourceLink(s, note.commit)));
	const doc = $derived(note.docs ? flatSidebar.find((item) => item.slug === note.docs) : undefined);
	const docHref = $derived(
		doc ? (doc.slug === DOCS_INDEX_SLUG ? '/docs' : (`/docs/${doc.slug}` as const)) : undefined
	);
	const path = $derived(`/notes/${note.module}/${note.slug}`);
	const url = $derived(`${seoConfig.domain}${path}`);
	const checked = $derived(note.checked.slice(0, 10));

	// Dates in a note read the way its prose does: "9 October 2026".
	function longDate(iso: string): string {
		return new Date(`${iso}T00:00:00Z`).toLocaleDateString('en-GB', {
			day: 'numeric',
			month: 'long',
			year: 'numeric',
			timeZone: 'UTC'
		});
	}

	const jsonLd = $derived([
		{
			'@context': 'https://schema.org',
			'@type': 'TechArticle',
			headline: note.title,
			description: note.description,
			url,
			mainEntityOfPage: url,
			dateModified: checked,
			inLanguage: 'en',
			isPartOf: {
				'@type': 'CollectionPage',
				name: 'Vela Wallet notes',
				url: `${seoConfig.domain}/notes`
			},
			author: { '@type': 'Person', name: 'Shelchin' },
			publisher: { '@type': 'Organization', name: seoConfig.siteName }
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
				},
				{ '@type': 'ListItem', position: 3, name: note.title, item: url }
			]
		}
	]);
</script>

<Seo
	title={note.title}
	description={note.description}
	canonical={path}
	type="article"
	modified={checked}
	{jsonLd}
	noindex={note.draft}
/>

<article class="note">
	<nav class="crumbs" aria-label="Breadcrumb">
		<a href={resolve('/notes')}>Notes</a>
		<span class="sep" aria-hidden="true">›</span>
		<a class="here" href={resolve(`/notes/${module.slug}`)}>{module.title}</a>
	</nav>

	{#if note.draft}
		<p class="draft">Draft. Visible only in development until it is reviewed.</p>
	{/if}

	<h1>{note.title}</h1>
	<!-- The answer first, so a reader who stops here still leaves with it. -->
	<p class="answer">{note.description}</p>

	{#if note.facts.length}
		<dl class="facts">
			{#each note.facts as fact (fact.label)}
				<div class="fact">
					<dt>{fact.label}</dt>
					<dd>{fact.value}</dd>
				</div>
			{/each}
		</dl>
	{/if}

	{#key note.id}
		<div class="body">
			<Prose>
				<Body />
			</Prose>
		</div>
	{/key}

	<footer class="note-footer">
		{#if related.length || doc}
			<section aria-labelledby="related-h">
				<h2 id="related-h">Read next</h2>
				<ul class="related">
					{#each related as r (r.id)}
						<li>
							<a href={resolve(`/notes/${r.module}/${r.slug}`)}>
								<span class="t">{r.title}</span>
								<span class="arrow" aria-hidden="true">→</span>
							</a>
						</li>
					{/each}
					{#if doc && docHref}
						<li>
							<a href={resolve(docHref)}>
								<span class="t">{doc.title} <span class="kind">· in the docs</span></span>
								<span class="arrow" aria-hidden="true">→</span>
							</a>
						</li>
					{/if}
				</ul>
			</section>
		{/if}

		<section class="sources" aria-labelledby="sources-h">
			<h2 id="sources-h">Where this lives in the code</h2>
			<ul>
				{#each sources as source (source.href)}
					<li>
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- GitHub, another origin -->
						<a href={source.href} rel="noopener" target="_blank"><code>{source.label}</code></a>
					</li>
				{/each}
			</ul>
			<p class="checked">
				Checked against the code on <time datetime={checked}>{longDate(checked)}</time>. Links point
				at commit
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- GitHub, another origin -->
				<a href={`${seoConfig.github}/tree/${note.commit}`} rel="noopener" target="_blank"
					><code>{note.commit}</code></a
				>, the code this note was checked against. If the code has moved on since, the note may be
				out of date; the code is right.
			</p>
		</section>

		{#if adjacent.prev || adjacent.next}
			<nav class="pager" aria-label={`More in ${module.title}`}>
				{#if adjacent.prev}
					<a
						class="pager-link"
						href={resolve(`/notes/${adjacent.prev.module}/${adjacent.prev.slug}`)}
					>
						<span class="dir">← Previous</span>
						<span class="t">{adjacent.prev.title}</span>
					</a>
				{:else}
					<span></span>
				{/if}
				{#if adjacent.next}
					<a
						class="pager-link right"
						href={resolve(`/notes/${adjacent.next.module}/${adjacent.next.slug}`)}
					>
						<span class="dir">Next →</span>
						<span class="t">{adjacent.next.title}</span>
					</a>
				{/if}
			</nav>
		{/if}
	</footer>
</article>

<style>
	.note {
		/* Ink a shade off full black and a tint a shade off the page: the two
		   colours a long read rests on, in both themes. */
		--note-ink: var(--ink-read);
		--note-tint: color-mix(in srgb, var(--text) 4%, var(--bg));
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding-bottom: 40px;
	}
	.crumbs {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		font-size: max(0.9rem, var(--floor-note));
		margin-bottom: 16px;
	}
	.crumbs a {
		color: var(--text-secondary);
	}
	.crumbs a.here {
		color: var(--text-secondary);
	}
	.crumbs a:hover {
		color: var(--text);
	}
	.crumbs .sep {
		color: var(--text-muted);
	}
	.draft {
		display: inline-block;
		margin-bottom: 14px;
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		background: var(--accent-soft);
		color: var(--accent);
		font-size: max(0.84rem, var(--floor-meta));
	}
	h1 {
		font-size: clamp(2.1rem, 1.7rem + 1.4vw, 2.65rem);
		line-height: 1.06;
		font-weight: 800;
		text-wrap: balance;
		margin-bottom: 12px;
	}
	/* One calm reading surface. Nothing on this page competes with the text:
	   no accent colour, no boxes, no capitals. Orange is the site's signal for
	   the action that moves money, and there is none here. */
	.answer {
		font-size: clamp(1.15rem, 1.05rem + 0.4vw, 1.28rem);
		line-height: 1.6;
		color: var(--note-ink);
		text-wrap: pretty;
		margin-bottom: 22px;
	}
	.facts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(128px, 1fr));
		gap: 14px 24px;
		margin: 0 0 34px;
		padding: 16px 20px;
		border-radius: var(--radius);
		background: var(--note-tint);
	}
	.fact dt {
		font-size: max(0.84rem, var(--floor-meta));
		color: var(--text-muted);
		margin-bottom: 2px;
	}
	.fact dd {
		font-size: 1rem;
		font-weight: 500;
		line-height: 1.4;
		color: var(--note-ink);
		font-variant-numeric: tabular-nums;
	}
	/* The same reading surface as the docs and the blog (Prose). */
	.body :global(.prose p) {
		text-wrap: pretty;
	}
	/* Section labels inside a note ("Why", "The trade-off") are signposts, not
	   chapter titles: small, sans, close to the text they introduce. */
	.body :global(.prose h2) {
		font-family: var(--font-system);
		font-stretch: 100%;
		font-size: 1rem;
		font-weight: 600;
		letter-spacing: 0;
		color: var(--text-secondary);
		margin: 30px 0 6px;
		padding: 0;
		border: 0;
	}
	.body :global(.prose strong) {
		font-weight: 600;
		color: inherit;
	}
	.body :global(.prose li::marker) {
		color: var(--text-muted);
	}
	.body :global(.prose > :first-child) {
		margin-top: 0;
	}
	.body :global(.prose ul) {
		margin-top: 6px;
	}
	.body :global(.prose li) {
		margin: 6px 0;
	}
	.note-footer {
		margin-top: 48px;
		display: flex;
		flex-direction: column;
		gap: 36px;
	}
	.note-footer h2 {
		font-family: var(--font-system);
		font-stretch: 100%;
		letter-spacing: 0;
		font-size: max(0.88rem, var(--floor-meta));
		color: var(--text-muted);
		font-weight: 500;
		margin-bottom: 8px;
	}
	.note-footer ul {
		list-style: none;
	}
	.related li {
		border-bottom: 1px solid var(--border);
	}
	.related li:first-child {
		border-top: 1px solid var(--border);
	}
	.related a {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 16px;
		padding: 14px 2px;
		color: var(--note-ink);
		font-size: max(1rem, var(--floor-read));
		font-weight: 500;
		line-height: 1.45;
	}
	.related a:hover {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.related .kind {
		font-weight: 400;
		color: var(--text-muted);
	}
	.related .arrow {
		color: var(--text-muted);
		flex: none;
	}
	.sources li {
		padding: 3px 0;
		font-size: max(0.92rem, var(--floor-read));
		line-height: 1.5;
	}
	.sources code {
		font-size: 0.86em;
		word-break: break-all;
	}
	.sources a,
	.checked a {
		color: var(--text-secondary);
	}
	.sources a:hover,
	.checked a:hover {
		color: var(--text);
	}
	.checked {
		margin-top: 10px;
		font-size: max(0.86rem, var(--floor-note));
		line-height: 1.55;
		color: var(--text-muted);
	}
	.pager {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
	}
	.pager-link {
		display: flex;
		flex-direction: column;
		gap: 6px;
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
		font-size: 0.78rem;
		color: var(--text-muted);
	}
	.pager-link .t {
		font-size: 0.95rem;
		font-weight: 600;
		line-height: 1.4;
		color: var(--text);
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
