<script lang="ts">
	import type { Locale } from '$lib/i18n/locales';

	/**
	 * The two web fonts of the reading pages (docs, blog, notes; spec 101):
	 * Archivo, with its width axis for the condensed headings, and Martian Mono
	 * for code. Loaded on those pages only, so the rest of the site pays
	 * nothing for them.
	 *
	 * Chinese, Japanese and Korean pages load neither (founder, 2026-09-22): the
	 * headings there use the platform's own CJK sans, and `--font-display`
	 * falls back to the system face for those `lang`s in tokens.css.
	 */
	let { locale = 'en' }: { locale?: Locale | string } = $props();

	const cjk = $derived(/^(zh|ja|ko)/.test(locale));
</script>

<svelte:head>
	{#if !cjk}
		<link
			href="https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,100..900&family=Martian+Mono:wdth,wght@75..112.5,100..800&display=swap"
			rel="stylesheet"
		/>
	{/if}
</svelte:head>
