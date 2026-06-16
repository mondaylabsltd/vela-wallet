<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { seoConfig } from '$lib/seo';
	import { formatDate } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'Blog',
		name: `${seoConfig.siteName} Blog`,
		url: `${seoConfig.domain}/blog`,
		description: 'Build notes, release notes and the story behind Vela Wallet.',
		blogPost: data.posts.map((post) => ({
			'@type': 'BlogPosting',
			headline: post.meta.title,
			description: post.meta.description,
			datePublished: post.meta.date,
			url: `${seoConfig.domain}/blog/${post.slug}`
		}))
	});
</script>

<Seo
	title="Blog"
	description="Build notes, release notes and the story behind Vela Wallet — a self-custodial wallet with no seed phrase."
	canonical="/blog"
	{jsonLd}
/>

<SiteHeader />

<main class="wrap">
	<header class="intro">
		<h1>Blog</h1>
		<p>Build notes, release notes, and the story of building Vela in public.</p>
		<a class="rss" href="/blog/rss.xml">RSS feed</a>
	</header>

	{#if data.posts.length === 0}
		<p class="empty">No posts yet — check back soon.</p>
	{:else}
		<ul class="posts">
			{#each data.posts as post (post.slug)}
				<li>
					<a class="card" href={resolve(`/blog/${post.slug}`)}>
						<div class="meta">
							<time datetime={post.meta.date}>{formatDate(post.meta.date)}</time>
							<span class="dot">·</span>
							<span>{post.readingMinutes} min read</span>
						</div>
						<h2>{post.meta.title}</h2>
						<p class="excerpt">{post.meta.description}</p>
						{#if post.meta.tags?.length}
							<div class="tags">
								{#each post.meta.tags as tag (tag)}
									<span class="tag">{tag}</span>
								{/each}
							</div>
						{/if}
						<span class="read">Read post →</span>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</main>

<SiteFooter />

<style>
	.wrap {
		max-width: 820px;
		margin: 0 auto;
		padding: 56px 24px 0;
	}
	.intro {
		margin-bottom: 44px;
	}
	.intro h1 {
		font-size: 2.6rem;
		font-weight: 700;
		letter-spacing: -0.03em;
	}
	.intro p {
		margin-top: 10px;
		color: var(--text-secondary);
		font-size: 1.1rem;
	}
	.rss {
		display: inline-block;
		margin-top: 16px;
		font-size: 0.85rem;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--border);
		padding-bottom: 1px;
	}
	.rss:hover {
		color: var(--accent);
		border-color: var(--accent);
	}
	.posts {
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.card {
		display: block;
		padding: 24px 26px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-card);
		transition:
			border-color 0.15s ease,
			transform 0.15s ease;
	}
	.card:hover {
		border-color: var(--border-strong);
		transform: translateY(-2px);
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.82rem;
		color: var(--text-muted);
	}
	.card h2 {
		margin: 10px 0 8px;
		font-size: 1.4rem;
		font-weight: 650;
		letter-spacing: -0.01em;
		color: var(--text);
	}
	.excerpt {
		color: var(--text-secondary);
		font-size: 0.98rem;
		line-height: 1.6;
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 14px;
	}
	.tag {
		font-size: 0.74rem;
		color: var(--text-secondary);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 999px;
		padding: 2px 10px;
	}
	.read {
		display: inline-block;
		margin-top: 16px;
		font-size: 0.88rem;
		font-weight: 600;
		color: var(--accent);
	}
	.empty {
		color: var(--text-secondary);
	}

	@media (max-width: 560px) {
		.intro h1 {
			font-size: 2.1rem;
		}
		.card {
			padding: 20px;
		}
	}
</style>
