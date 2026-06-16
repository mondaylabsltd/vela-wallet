<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import Prose from '$lib/components/Prose.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { getPost, getAdjacentPosts } from '$lib/content/blog';
	import { seoConfig } from '$lib/seo';
	import { formatDate } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// The compiled component is looked up client-side (and during prerender) so we
	// never have to serialize it through `load`.
	const post = $derived(getPost(data.slug)!);
	const Content = $derived(post.component);
	const adjacent = $derived(getAdjacentPosts(data.slug));

	const author = $derived(data.meta.author ?? 'Vela');

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'BlogPosting',
		headline: data.meta.title,
		description: data.meta.description,
		datePublished: data.meta.date,
		author: { '@type': 'Person', name: author },
		publisher: {
			'@type': 'Organization',
			name: seoConfig.siteName,
			logo: { '@type': 'ImageObject', url: `${seoConfig.domain}/vela-logo.png` }
		},
		mainEntityOfPage: `${seoConfig.domain}/blog/${data.slug}`,
		url: `${seoConfig.domain}/blog/${data.slug}`
	});
</script>

<Seo
	title={data.meta.title}
	description={data.meta.description}
	canonical={`/blog/${data.slug}`}
	type="article"
	published={data.meta.date}
	{author}
	image={data.meta.cover ? seoConfig.domain + data.meta.cover : undefined}
	{jsonLd}
/>

<SiteHeader />

<main class="article-wrap">
	<a class="back" href={resolve('/blog')}>← All posts</a>

	<article>
		<header class="post-header">
			<div class="meta">
				<time datetime={data.meta.date}>{formatDate(data.meta.date)}</time>
				<span class="dot">·</span>
				<span>{data.readingMinutes} min read</span>
				<span class="dot">·</span>
				<span>{author}</span>
			</div>
			<h1>{data.meta.title}</h1>
			<p class="lede">{data.meta.description}</p>
		</header>

		<Prose>
			<Content />
		</Prose>
	</article>

	{#if adjacent.newer || adjacent.older}
		<nav class="pager" aria-label="More posts">
			{#if adjacent.older}
				<a class="pager-link" href={resolve(`/blog/${adjacent.older.slug}`)}>
					<span class="dir">← Older</span>
					<span class="t">{adjacent.older.meta.title}</span>
				</a>
			{:else}
				<span></span>
			{/if}
			{#if adjacent.newer}
				<a class="pager-link right" href={resolve(`/blog/${adjacent.newer.slug}`)}>
					<span class="dir">Newer →</span>
					<span class="t">{adjacent.newer.meta.title}</span>
				</a>
			{/if}
		</nav>
	{/if}
</main>

<SiteFooter />

<style>
	.article-wrap {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding: 40px 24px 0;
	}
	.back {
		display: inline-block;
		margin-bottom: 28px;
		font-size: 0.88rem;
		color: var(--text-secondary);
	}
	.back:hover {
		color: var(--text);
	}
	.post-header {
		margin-bottom: 36px;
	}
	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		font-size: 0.84rem;
		color: var(--text-muted);
	}
	.post-header h1 {
		margin: 14px 0 12px;
		font-size: 2.4rem;
		line-height: 1.15;
		font-weight: 700;
		letter-spacing: -0.03em;
	}
	.lede {
		font-size: 1.15rem;
		line-height: 1.6;
		color: var(--text-secondary);
	}
	.pager {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin: 56px 0 0;
		padding-top: 32px;
		border-top: 1px solid var(--border);
	}
	.pager-link {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 18px 20px;
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
		color: var(--text);
	}

	@media (max-width: 560px) {
		.post-header h1 {
			font-size: 1.9rem;
		}
		.lede {
			font-size: 1.05rem;
		}
		.pager {
			grid-template-columns: 1fr;
		}
		.pager-link.right {
			text-align: left;
		}
	}
</style>
