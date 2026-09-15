<script lang="ts">
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { catalog, namespaceState } from '$lib/i18n/resolve';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const m = $derived(catalog(data.locale));
	const roadmapState = $derived(namespaceState('roadmap', data.locale));

	/**
	 * Status and date are DATA, not copy: the badge colour is a judgement and the
	 * ship dates are facts. Neither travels through the translation files, where
	 * a translator could localize "Jun 2026" into something the CSS cannot key
	 * off. They are ordered to match `m.roadmap.upcoming` / `m.roadmap.shipped`.
	 */
	const UPCOMING_STATUS = ['now', 'now', 'next', 'next', 'later'] as const;
	const SHIPPED_DATES = [
		'Jun 2026',
		'Jun 13, 2026',
		'Jun 9, 2026',
		'Jun 4, 2026',
		'May 28, 2026',
		'May 2026',
		'Apr 22, 2026'
	] as const;
</script>

<svelte:head>
	<title>{m.roadmap.meta.title}</title>
	<meta name="description" content={m.roadmap.meta.description} />
</svelte:head>

{#if roadmapState === 'fallback'}
	<TranslationNotice locale={data.locale} />
{/if}

<SiteHeader locale={data.locale} />

<main class="container">
	<h1>{m.roadmap.heading}</h1>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	<p class="lede">{@html m.roadmap.lede}</p>

	<h2 class="phase-title">{m.roadmap.upcomingHeading}</h2>
	<ol class="track">
		{#each m.roadmap.upcoming as item, i (item.title)}
			<li class="node {UPCOMING_STATUS[i]}">
				<span class="badge {UPCOMING_STATUS[i]}">{m.roadmap.statusLabels[UPCOMING_STATUS[i]]}</span>
				<h3>{item.title}</h3>
				<p>{item.body}</p>
			</li>
		{/each}
	</ol>

	<h2 class="phase-title shipped-title">{m.roadmap.shippedHeading}</h2>
	<ol class="track">
		{#each m.roadmap.shipped as item, i (item.title)}
			<li class="node ship">
				<span class="when">{SHIPPED_DATES[i]}</span>
				<h3>{item.title}</h3>
				<p>{item.body}</p>
			</li>
		{/each}
	</ol>
</main>

<SiteFooter locale={data.locale} />

<style>
	/* The sticky SiteHeader occupies its own flow space, so no fixed-nav offset. */
	main.container {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding: 48px 24px 72px;
	}

	h1 {
		font-size: 2rem;
		margin-bottom: 12px;
		letter-spacing: -0.02em;
	}
	.lede {
		color: var(--text-secondary);
		line-height: 1.7;
		font-size: 0.98rem;
		margin-bottom: 44px;
	}
	.lede :global(a) {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 2px;
	}
	.lede :global(a:hover) {
		color: var(--accent);
	}

	.phase-title {
		font-size: 0.78rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
		margin-bottom: 24px;
	}
	.shipped-title {
		margin-top: 16px;
	}

	.track {
		list-style: none;
		position: relative;
		margin: 0 0 48px;
		padding: 0;
	}
	.track::before {
		content: '';
		position: absolute;
		left: 6px;
		top: 8px;
		bottom: 8px;
		width: 2px;
		background: var(--border);
	}
	.node {
		position: relative;
		padding: 0 0 30px 34px;
	}
	.node:last-child {
		padding-bottom: 0;
	}
	.node::before {
		content: '';
		position: absolute;
		left: 0;
		top: 3px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--bg);
		border: 2px solid var(--border);
		box-sizing: border-box;
	}
	.node.ship::before {
		background: var(--green);
		border-color: var(--green);
	}
	.node.now::before {
		background: var(--accent);
		border-color: var(--accent);
		box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 15%, transparent);
		animation: pulse 2s ease-in-out infinite;
	}
	.node.next::before {
		background: var(--bg);
		border-color: var(--accent);
	}
	.node.later::before {
		background: var(--bg);
		border-color: var(--text-tertiary);
	}
	@keyframes pulse {
		0%,
		100% {
			box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 15%, transparent);
		}
		50% {
			box-shadow: 0 0 0 7px color-mix(in srgb, var(--accent) 5%, transparent);
		}
	}

	.node h3 {
		font-size: 1.05rem;
		font-weight: 600;
		color: var(--text);
		margin: 6px 0 6px;
		letter-spacing: -0.01em;
	}
	.node p {
		color: var(--text-secondary);
		font-size: 0.93rem;
		line-height: 1.65;
	}

	.badge {
		display: inline-block;
		font-size: 0.68rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		padding: 2px 9px;
		border-radius: 999px;
		border: 1px solid var(--border);
	}
	.badge.now {
		color: var(--accent);
		border-color: var(--border-accent);
		background: var(--accent-soft);
	}
	.badge.next {
		color: var(--text-secondary);
	}
	.badge.later {
		color: var(--text-tertiary);
	}
	.when {
		font-size: 0.78rem;
		font-weight: 600;
		color: var(--text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
