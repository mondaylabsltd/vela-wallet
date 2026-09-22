<script lang="ts">
	import '$lib/styles/tokens.css';
	import { page } from '$app/state';
	import { splitLocalePath } from '$lib/i18n/locales';

	let { children } = $props();

	// `<html lang>` is written once, by the server (hooks.server.ts). A
	// client-side navigation to another locale — the language switcher is one —
	// kept the previous page's lang, so a Chinese page reached from English
	// rendered its headline without the CJK size rule and in a different
	// fallback font until the reader reloaded. Follow the URL on every navigation.
	const lang = $derived(splitLocalePath(page.url.pathname).locale);
	$effect(() => {
		document.documentElement.lang = lang;
	});

	// The chain-setup page keeps a funded deployer key in this browser's
	// localStorage. A third-party script has no business on the same page as a
	// key (spec 080), so the analytics tag is left off there.
	const analytics = $derived(!page.url.pathname.endsWith('/chain-setup'));
</script>

<svelte:head>
	<link rel="icon" type="image/png" href="/favicon-96x96.png" sizes="96x96" />
	<link rel="shortcut icon" href="/favicon.ico" />
	<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png" />
	<link rel="manifest" href="/site.webmanifest" />
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Source+Serif+4:ital,opsz,wght@0,8..60,400..700;1,8..60,400&display=swap"
		rel="stylesheet"
	/>
	{#if analytics}
		<script src="https://tj.appsdata.org/api/script.js" data-site-id="d9a1055d13df" defer></script>
	{/if}
</svelte:head>

{@render children()}

<style>
	:global(*) {
		margin: 0;
		padding: 0;
		box-sizing: border-box;
	}

	:global(body) {
		font-family: var(--font-sans);
		background: var(--bg);
		color: var(--text);
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
	}

	/* Display headings read in a warm serif; smaller UI headings stay sans.
	   Components can opt out with an explicit font-family. */
	:global(h1),
	:global(h2) {
		font-family: var(--font-serif);
		font-weight: 600;
	}

	:global(a) {
		color: inherit;
		text-decoration: none;
	}
</style>
