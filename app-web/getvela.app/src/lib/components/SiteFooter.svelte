<script lang="ts">
	import { DEFAULT_LOCALE, pathFor, type Locale } from '$lib/i18n/locales';
	import { catalog } from '$lib/i18n/resolve';

	/**
	 * The shared footer. Like SiteHeader it defaults to English so the pages that
	 * stay English (blog, legal, registry) use it unchanged.
	 *
	 * Links to English-only destinations keep their unprefixed URL and carry
	 * `hreflang="en"`, so a reader in another language is not sent to a localized
	 * URL that does not exist (R5, FR-020).
	 */
	let { locale = DEFAULT_LOCALE }: { locale?: Locale } = $props();

	const m = $derived(catalog(locale));
	const year = new Date().getFullYear();
	const L = $derived((path: string) => pathFor(locale, path));
</script>

<footer class="site-footer">
	<div class="inner">
		<div class="left">
			<div class="brand">
				<img src="/vela-logo.png" alt="Vela" width="24" height="24" />
				<span>Vela Wallet</span>
			</div>
			<p class="tagline">{m.chrome.footer.tagline}</p>
			<p class="copy">&copy; {year} MONDAY LABS LTD</p>
		</div>

		<nav class="cols" aria-label={m.chrome.footer.label}>
			<div class="col">
				<h3>{m.chrome.footer.columns.resources}</h3>
				<a href={L('/docs')}>{m.chrome.footer.links.docs}</a>
				<a href={L('/docs/whitepaper')}>{m.chrome.footer.links.whitepaper}</a>
				<a href={L('/docs/security-audits')}>{m.chrome.footer.links.audits}</a>
				<a href={L('/roadmap')}>{m.chrome.footer.links.roadmap}</a>
				<a href="/blog" hreflang="en">{m.chrome.footer.links.blog}</a>
				<a href={L('/about')}>{m.chrome.footer.links.about}</a>
			</div>
			<div class="col">
				<h3>{m.chrome.footer.columns.infrastructure}</h3>
				<a href={L('/docs/self-hosting')}>{m.chrome.footer.links.selfHosting}</a>
				<a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener"
					>Vela Relay</a
				>
				<a href="https://github.com/mondaylabsltd/p256-index" target="_blank" rel="noopener"
					>Passkey Index</a
				>
				<a href="https://github.com/atshelchin/ethereum-data" target="_blank" rel="noopener"
					>Chain Data Index</a
				>
				<a href="https://github.com/mondaylabsltd/vela-currency" target="_blank" rel="noopener"
					>Vela Currency</a
				>
				<a href={L('/chain-setup')}>{m.chrome.footer.links.chainSetup}</a>
			</div>
			<div class="col">
				<h3>{m.chrome.footer.columns.community}</h3>
				<a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
					>GitHub</a
				>
				<a href="https://x.com/realvelawallet" target="_blank" rel="noopener">X / Twitter</a>
				<a href="https://t.me/velawallet" target="_blank" rel="noopener">Telegram</a>
				<a href="https://discord.gg/S6A8RyCk6" target="_blank" rel="noopener">Discord</a>
				<!-- rss.xml is a prerendered endpoint, invisible to the client router —
				     without a full-page load the URL falls through to blog/[slug] and 404s. -->
				<a href="/blog/rss.xml" data-sveltekit-reload>RSS</a>
			</div>
			<div class="col">
				<h3>{m.chrome.footer.columns.legal}</h3>
				<a href="/privacy" hreflang="en">{m.chrome.footer.links.privacy}</a>
				<a href="/terms" hreflang="en">{m.chrome.footer.links.terms}</a>
			</div>
		</nav>
	</div>
</footer>

<style>
	.site-footer {
		border-top: 1px solid var(--border);
		background: var(--bg);
		margin-top: 80px;
	}
	.inner {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 56px 24px 48px;
		display: flex;
		flex-wrap: wrap;
		gap: 48px;
		justify-content: space-between;
	}
	.left {
		max-width: 280px;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
		font-size: 1.05rem;
	}
	.brand img {
		border-radius: 6px;
	}
	.tagline {
		margin-top: 14px;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.copy {
		margin-top: 14px;
		color: var(--text-secondary);
		font-size: 0.85rem;
	}
	.cols {
		display: flex;
		flex-wrap: wrap;
		gap: 40px;
	}
	.col {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 110px;
	}
	.col h3 {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-muted);
		font-weight: 600;
		margin-bottom: 2px;
	}
	.col a {
		font-size: 0.9rem;
		color: var(--text-secondary);
		transition: color 0.15s ease;
	}
	.col a:hover {
		color: var(--text);
	}

	@media (max-width: 560px) {
		.inner {
			padding: 40px 24px;
			gap: 36px;
		}
		.cols {
			gap: 32px;
		}
	}
</style>
