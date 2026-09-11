<script lang="ts">
	import '@fontsource/plus-jakarta-sans/400.css';
	import '@fontsource/plus-jakarta-sans/500.css';
	import '@fontsource/plus-jakarta-sans/600.css';
	import '@fontsource/plus-jakarta-sans/700.css';
	import '@fontsource/noto-sans-sc/400.css';
	import '@fontsource/noto-sans-sc/500.css';
	import '@fontsource/noto-sans-sc/700.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '$lib/tokens/tokens.css';
	import '../app.css';
	import { onMount } from 'svelte';
	import LaunchAnimation from '$lib/launch/LaunchAnimation.svelte';
	import { markPlayed, shouldPlay, type Appearance } from '$lib/launch/constants';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { updated } from '$app/state';
	import { invalidateAllPools } from '$lib/services/rpc-pool';
	import { normalizePackagedUrl } from '$lib/extension/page-url';
	import ParallelSpaceBadge from '$lib/dev/ParallelSpaceBadge.svelte';
	import { parallelFlagSet } from '$lib/dev/parallel-flag.svelte';

	let { children } = $props();

	/**
	 * The offline line's words. The layout has no corpus of its own (it is the
	 * one route outside `[locale]`), so the sentence is the settings screen's
	 * `offline` string, resolved by the page data when a locale page is up and
	 * English otherwise.
	 */
	const offlineLabel = $derived(
		(page.data as { flow?: Record<string, string> } | undefined)?.flow?.[
			'onboarding.common.networkBody'
		] ?? 'The request never arrived — check your network and try again.'
	);

	/**
	 * Spec 027 D42. Under the packaged extension a route path is not a file, so
	 * after a client navigation the address bar names a document that does not
	 * exist and a reload dies on `chrome-error://`. Put the document's own name
	 * back. Identity on the hosted site — decided by this page's origin, not by
	 * a build flag.
	 */
	afterNavigate(() => normalizePackagedUrl());

	const parallelHref = $derived(
		resolve('/[locale]/parallel', { locale: page.params.locale ?? 'en' })
	);

	// Spec 012. Client-only by construction: `launching` starts false, so the
	// prerendered HTML contains no overlay and the page is complete without it
	// — it can never be the LCP element, and a visitor without scripting sees
	// the normal page (spec Edge Cases).
	let launching = $state(false);
	let pageOpacity = $state(1);
	let appearance = $state<Appearance>('dark');

	/**
	 * Spec 038 (Part B): the browser's own word on connectivity. Offline is
	 * named on screen instead of surfacing as a dozen RPC endpoints banned on
	 * their cooldown schedule; a reconnect lifts the pools so the next read
	 * does not wait out a ban earned while the cable was unplugged.
	 */
	let offline = $state(false);

	/**
	 * A deploy while the wallet is open: the next lazy chunk would be a 404
	 * and the app broken until somebody thinks to reload. SvelteKit polls the
	 * version (`kit.version.pollInterval` in vite.config.ts); when it moved,
	 * the next navigation is a full document load rather than a client one.
	 */
	beforeNavigate(({ to, willUnload }) => {
		if (updated.current && to?.url && !willUnload) location.href = to.url.href;
	});

	onMount(() => {
		offline = typeof navigator !== 'undefined' && navigator.onLine === false;
		const wentOffline = () => (offline = true);
		const cameBack = () => {
			offline = false;
			// Bans and cooldowns earned while offline are not facts about the
			// endpoints; drop them so the first read after reconnect is live.
			invalidateAllPools();
		};
		window.addEventListener('offline', wentOffline);
		window.addEventListener('online', cameBack);
		// A lazy import that failed to fetch (a deploy moved the hashes) is
		// unrecoverable in place; the document is.
		const preloadFailed = () => location.reload();
		window.addEventListener('vite:preloadError', preloadFailed);

		// The dev/e2e console (spec 025) is a DYNAMIC import behind its gate:
		// a static one would drag the core's JS glue into the first-paint
		// chunk of every page, Welcome included.
		try {
			if (import.meta.env.DEV || localStorage.getItem('vela.dev.console') === '1') {
				void import('$lib/services/dev-console').then((m) => m.maybeInstallDevConsole());
			}
		} catch {
			// storage denied — no console, no harm
		}
		// The parallel space re-arms itself on EVERY boot, gate or no gate
		// (spec 026 US4): a reload inside it must stay inside it, and a wallet
		// wearing fixture keys must never boot unmarked. The fixture signer and
		// its key material stay behind this dynamic import, so a real visit
		// never loads them.
		if (parallelFlagSet()) {
			void import('$lib/dev/parallel-space').then((m) => {
				void m.applyParallelSpaceOnBoot();
				m.installParallelConsole();
			});
		}
		// The inline script in app.html already made this decision before paint
		// and recorded it on <html>. Reading it back — rather than re-deciding —
		// is what guarantees the two cannot disagree and leave the page hidden
		// behind an animation that never starts.
		if (document.documentElement.dataset.launch !== 'playing') return;
		if (!shouldPlay()) return;
		markPlayed();
		// The effective appearance the rest of the page already resolves, not the
		// raw OS setting (FR-009): tokens.css keys off `data-theme` first and
		// `prefers-color-scheme` second, so read it the same way round.
		const pinned = document.documentElement.dataset.theme;
		appearance =
			pinned === 'light' || pinned === 'dark'
				? (pinned as Appearance)
				: matchMedia('(prefers-color-scheme: light)').matches
					? 'light'
					: 'dark';
		pageOpacity = 0;
		launching = true;
	});
</script>


<!--
	One continuous surface. The page content and the launch screen sit on this
	exact colour, which is what lets them cross-dissolve without a washed-out
	middle where both are half-transparent over the bare document (FR-012).
-->
<div class="surface">
	<div class="page" data-launch-page style:opacity={pageOpacity}>
		{#if offline}
			<p class="offline" role="status">{offlineLabel}</p>
		{/if}
		{@render children()}
	</div>

	<ParallelSpaceBadge onopen={() => goto(parallelHref)} />

	{#if launching}
		<LaunchAnimation
			{appearance}
			onprogress={(value) => (pageOpacity = value)}
			onfinished={() => {
				pageOpacity = 1;
				launching = false;
				// Release the pre-paint hold, or the CSS rule keeps the page at
				// opacity 0 for the rest of the visit.
				delete document.documentElement.dataset.launch;
			}}
		/>
	{/if}
</div>

<style>
	.surface {
		min-height: 100dvh;
		background: var(--color-bg-base);
	}

	.page {
		/* Driven per-frame by the overlay's dissolve, so no CSS transition here —
		   two animations on one property would fight. */
		min-height: 100dvh;
	}
	.offline {
		margin: 0;
		padding: var(--space-sm) var(--layout-screenPaddingX);
		background: var(--color-warning-soft, var(--color-bg-sunken));
		color: var(--color-warning-base);
		font-size: var(--text-sm);
		text-align: center;
	}
</style>
