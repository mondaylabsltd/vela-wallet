<script lang="ts">
	import type { Locale } from '$lib/i18n/locales';
	import { catalog } from '$lib/i18n/resolve';

	/**
	 * "This page has not been translated yet" (spec 059, FR-017).
	 *
	 * The one string on the site that cannot itself fall back: it is shown to a
	 * reader who asked for their language and is about to get English, so if it
	 * appeared in English it would be telling them nothing they could read. That
	 * is why `notice.*` is required complete in all fifteen locales
	 * (contracts/translation-store.md), and why this component reads it directly
	 * rather than through any page namespace.
	 *
	 * It is deliberately quiet — a hairline strip, not a warning banner. The page
	 * below it is correct and useful; it is just in the wrong language.
	 */
	let { locale }: { locale: Locale } = $props();

	const m = $derived(catalog(locale));
</script>

<p class="notice" lang={locale}>
	<svg
		width="14"
		height="14"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="1.8"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="9" />
		<path d="M12 8h.01M11 12h1v4h1" stroke-linecap="round" stroke-linejoin="round" />
	</svg>
	{m.notice.fallback}
</p>

<style>
	.notice {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 9px 20px;
		background: var(--bg-sunken);
		border-bottom: 1px solid var(--border);
		color: var(--text-muted);
		font-size: 0.8rem;
		line-height: 1.5;
		text-align: center;
	}
	.notice svg {
		flex-shrink: 0;
	}
</style>
