<script lang="ts">
	import '#lib/styles/tokens.css';
	import { page } from '$app/state';
	import { splitLocalePath } from '#lib/i18n/locales.ts';

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
		font-family: var(--font-ui);
		background: var(--bg);
		color: var(--text);
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
		/* Contextual alternates off (docs/FONT-POLICY.md §4): a face may draw the
		   `x` after a digit as `×`, and an address as "0×14fB…". */
		font-variant-ligatures: no-contextual;
	}

	/* Form controls do not inherit the body's font settings. */
	:global(:is(input, textarea, select, button)) {
		font-variant-ligatures: no-contextual;
	}

	/* Display headings. Components can opt out with an explicit font-family. */
	:global(h1),
	:global(h2) {
		font-family: var(--font-ui);
		font-weight: 600;
	}

	:global(a) {
		color: inherit;
		text-decoration: none;
	}
</style>
