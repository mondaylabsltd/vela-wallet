<script lang="ts">
	import { page } from '$app/state';
	import { DEFAULT_LOCALE, negotiate, switchTo, type Locale } from '$lib/i18n/locales';
	import { catalog, namespaceState } from '$lib/i18n/resolve';

	/**
	 * "This page is also available in 日本語" (spec 059, FR-013).
	 *
	 * Three rules, and each one is load-bearing:
	 *
	 *  1. **It never navigates.** The reader stays on the URL they asked for.
	 *     Auto-redirecting by `Accept-Language` is the obvious alternative and it
	 *     is what hands crawlers and shared links the wrong language (R3).
	 *  2. **It is written in the language being offered.** It is shown to someone
	 *     who may not read the page they are on, so an English banner offering
	 *     Japanese helps nobody.
	 *  3. **It only offers a locale that genuinely has content.** Sending a
	 *     Japanese reader to an English page with a Japanese apology on top is
	 *     worse than saying nothing. `home` stands in for the whole locale here:
	 *     translations land per locale, not per page, so a locale with a
	 *     translated landing page has the rest too.
	 *
	 * It is client-side because the pages are prerendered and served from a CDN —
	 * one HTML file per URL, identical for every visitor, which is also why it
	 * can be cached at the edge at all.
	 */
	let { locale }: { locale: Locale } = $props();

	const DISMISS_KEY = 'vela-locale-offer-dismissed';

	let offered = $state<Locale | null>(null);

	$effect(() => {
		let dismissed = '';
		try {
			dismissed = localStorage.getItem(DISMISS_KEY) ?? '';
		} catch {
			/* private mode — treat as not dismissed */
		}

		const preferred = negotiate(navigator.languages);
		if (preferred === locale) return;
		if (dismissed.split(',').includes(preferred)) return;
		// Nothing to offer if that locale's pages are English anyway.
		if (preferred !== DEFAULT_LOCALE && namespaceState('home', preferred) === 'fallback') return;

		offered = preferred;
	});

	function dismiss() {
		if (!offered) return;
		try {
			const prev = localStorage.getItem(DISMISS_KEY) ?? '';
			const next = prev ? `${prev},${offered}` : offered;
			localStorage.setItem(DISMISS_KEY, next);
		} catch {
			/* storage unavailable — the banner simply returns next visit */
		}
		offered = null;
	}
</script>

{#if offered}
	{@const om = catalog(offered)}
	<aside class="offer" lang={offered}>
		<span class="text">{om.notice.offer.message}</span>
		<a class="accept" href={switchTo(offered, page.url.pathname)} hreflang={offered}>
			{om.notice.offer.accept}
		</a>
		<button type="button" class="dismiss" onclick={dismiss}>{om.notice.offer.dismiss}</button>
	</aside>
{/if}

<style>
	.offer {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-wrap: wrap;
		gap: 6px 14px;
		padding: 10px 20px;
		background: var(--bg-sunken);
		border-bottom: 1px solid var(--border);
		font-size: 0.85rem;
	}
	.text {
		color: var(--text-secondary);
	}
	.accept {
		color: var(--accent);
		font-weight: 600;
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.dismiss {
		background: none;
		border: none;
		padding: 2px 4px;
		font: inherit;
		color: var(--text-tertiary);
		cursor: pointer;
	}
	.dismiss:hover {
		color: var(--text);
	}
</style>
