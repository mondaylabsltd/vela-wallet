<script lang="ts">
	import LocaleOfferBanner from '$lib/components/LocaleOfferBanner.svelte';
	import type { Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	/**
	 * The localized subtree's layout. It carries the locale and the one piece of
	 * chrome that belongs to every localized page: the offer banner (FR-013).
	 * The fallback notice is per page, not per layout — only the page knows
	 * whether ITS namespace fell back.
	 */
	let { children, data }: { children: Snippet; data: LayoutData } = $props();
</script>

<svelte:head>
	<!-- `<html lang>` is set in hooks.server.ts (it lives outside the app root);
	     this is the app-level record of the same fact, for anything reading the DOM. -->
	<meta name="content-language" content={data.locale} />
</svelte:head>

<LocaleOfferBanner locale={data.locale} />

{@render children()}
