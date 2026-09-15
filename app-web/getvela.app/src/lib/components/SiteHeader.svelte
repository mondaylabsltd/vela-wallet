<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import LanguageSwitcher from '$lib/components/LanguageSwitcher.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { DEFAULT_LOCALE, pathFor, splitLocalePath, type Locale } from '$lib/i18n/locales';
	import { catalog } from '$lib/i18n/resolve';

	/**
	 * The shared header.
	 *
	 * `locale` is NULL on the pages that have no translated version — blog,
	 * privacy, terms, registry (R5). That is not the same as `'en'`: an English
	 * page inside the locale subtree has fourteen siblings to switch to, and
	 * these have none. Offering the switcher there would produce links to URLs
	 * that 404 — which is exactly what the prerender crawler caught.
	 *
	 * `englishOnly` marks a destination that has no translation and never will —
	 * the reader is told before they click, rather than discovering it when the
	 * page arrives in a language they did not ask for (FR-020).
	 */
	let { locale = null }: { locale?: Locale | null } = $props();

	const active = $derived(locale ?? DEFAULT_LOCALE);
	const m = $derived(catalog(active));
	let open = $state(false);

	const links = $derived([
		{ href: '/blog', label: m.chrome.nav.blog, englishOnly: true },
		{ href: '/docs', label: m.chrome.nav.docs, englishOnly: false },
		{ href: '/about', label: m.chrome.nav.about, englishOnly: false }
	]);

	function isActive(href: string): boolean {
		// `page.url.pathname` carries the locale prefix; the links table does not.
		const path = splitLocalePath(page.url.pathname).path;
		return path === href || path.startsWith(href + '/');
	}
</script>

<header class="site-header">
	<div class="bar">
		<a href={resolve('/')} class="logo" onclick={() => (open = false)}>
			<img src="/vela-logo.png" alt="Vela Wallet" width="28" height="28" />
			<span>Vela Wallet</span>
		</a>

		<nav class="links" class:open aria-label={m.chrome.nav.primary}>
			{#each links as link (link.href)}
				<a
					href={link.englishOnly ? link.href : pathFor(active, link.href)}
					hreflang={link.englishOnly ? 'en' : active}
					class:active={isActive(link.href)}
					onclick={() => (open = false)}
				>
					{link.label}
					{#if link.englishOnly && active !== DEFAULT_LOCALE}
						<span class="en-badge" title={m.chrome.englishOnly.title}
							>{m.chrome.englishOnly.badge}</span
						>
					{/if}
				</a>
			{/each}
			<a
				href="https://github.com/mondaylabsltd/vela-wallet"
				target="_blank"
				rel="noopener"
				onclick={() => (open = false)}>GitHub</a
			>
			<a
				class="cta"
				href="https://wallet.getvela.app/"
				target="_blank"
				rel="noopener"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="header"
				onclick={() => (open = false)}>{m.chrome.nav.createWallet}</a
			>
		</nav>

		<div class="actions">
			{#if locale}
				<LanguageSwitcher {locale} />
			{/if}
			<ThemeToggle />
			<button
				class="menu"
				aria-label={m.chrome.nav.toggleMenu}
				aria-expanded={open}
				onclick={() => (open = !open)}
			>
				<span></span><span></span><span></span>
			</button>
		</div>
	</div>
</header>

<style>
	.en-badge {
		display: inline-block;
		margin-left: 5px;
		padding: 1px 4px;
		border: 1px solid var(--border);
		border-radius: 4px;
		font-size: 0.62rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		color: var(--text-tertiary);
		vertical-align: middle;
	}

	.site-header {
		position: sticky;
		top: 0;
		z-index: 50;
		background: var(--bg-header);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--border);
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.bar {
		max-width: var(--max-w);
		margin: 0 auto;
		height: var(--header-h);
		padding: 0 24px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
	}
	.logo {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 700;
		font-size: 1.05rem;
		letter-spacing: -0.01em;
	}
	.logo img {
		border-radius: 7px;
	}
	.links {
		display: flex;
		align-items: center;
		gap: 26px;
	}
	.links a {
		font-size: 0.92rem;
		color: var(--text-secondary);
		transition: color 0.15s ease;
	}
	.links a:hover {
		color: var(--text);
	}
	.links a.active {
		color: var(--text);
	}
	.links a.cta {
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		background: var(--accent);
		color: var(--text-on-accent);
		font-weight: 600;
	}
	.links a.cta:hover {
		background: var(--accent-hover);
	}
	.menu {
		display: none;
		flex-direction: column;
		justify-content: center;
		gap: 5px;
		width: 40px;
		height: 40px;
		background: none;
		border: none;
		cursor: pointer;
	}
	.menu span {
		display: block;
		height: 2px;
		width: 22px;
		margin: 0 auto;
		background: var(--text);
		border-radius: 2px;
	}

	@media (max-width: 820px) {
		.menu {
			display: flex;
		}
		.links {
			position: absolute;
			top: var(--header-h);
			left: 0;
			right: 0;
			flex-direction: column;
			align-items: stretch;
			gap: 0;
			padding: 8px 24px 20px;
			background: var(--bg-header-solid);
			backdrop-filter: blur(12px);
			-webkit-backdrop-filter: blur(12px);
			border-bottom: 1px solid var(--border);
			display: none;
		}
		.links.open {
			display: flex;
		}
		.links a {
			padding: 13px 0;
			font-size: 1rem;
			border-bottom: 1px solid var(--border);
		}
		.links a.cta {
			margin-top: 14px;
			text-align: center;
			border-bottom: none;
		}
	}
</style>
