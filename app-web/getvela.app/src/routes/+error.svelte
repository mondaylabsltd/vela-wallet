<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';

	/**
	 * The site's own 404 (spec 059, FR-010).
	 *
	 * An unknown locale prefix — `/xx/`, `/en/`, `/ja/blog` — matches no route,
	 * so SvelteKit falls back to this page. Without it the reader gets the
	 * framework's bare error text, which is exactly the "blank locale shell" the
	 * routing contract says must not happen. It is written in English on purpose:
	 * a request for an unsupported locale has no locale to be polite in.
	 */
</script>

<svelte:head>
	<title>{page.status} — Vela Wallet</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<SiteHeader />

<main class="wrap">
	<p class="code">{page.status}</p>
	<h1>{page.status === 404 ? 'That page does not exist' : 'Something went wrong'}</h1>
	<p class="body">
		{#if page.status === 404}
			The link may be old, or the language code may not be one Vela speaks.
		{:else}
			{page.error?.message ?? 'An unexpected error occurred.'}
		{/if}
	</p>
	<div class="links">
		<a class="btn" href={resolve('/')}>Go to the home page</a>
		<a href={resolve('/docs')}>Read the docs</a>
	</div>
</main>

<SiteFooter />

<style>
	.wrap {
		max-width: 640px;
		margin: 0 auto;
		padding: 120px 24px 80px;
		text-align: center;
	}
	.code {
		font-family: var(--font-mono);
		color: var(--text-tertiary);
		font-size: 0.85rem;
		letter-spacing: 0.08em;
	}
	h1 {
		font-size: clamp(1.8rem, 4vw, 2.4rem);
		margin: 10px 0 14px;
		letter-spacing: -0.02em;
	}
	.body {
		color: var(--text-secondary);
		line-height: 1.7;
	}
	.links {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 20px;
		margin-top: 32px;
		flex-wrap: wrap;
	}
	.links a {
		color: var(--text-secondary);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.btn {
		background: var(--accent);
		color: var(--text-on-accent);
		padding: 11px 22px;
		border-radius: 10px;
		font-weight: 600;
		font-size: 0.88rem;
		text-decoration: none;
	}
</style>
