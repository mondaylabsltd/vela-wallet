<script lang="ts">
	import Seo from '$lib/components/Seo.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { catalog, namespaceState, translatedLocales } from '$lib/i18n/resolve';
	import type { PageData } from './$types';

	/**
	 * "Get Vela" — the page the hero's first button now opens.
	 *
	 * It exists because the same wallet ships four ways, and the honest answer to
	 * "how do I start" is a question back: on what. The order is deliberate and
	 * is the founder's: web first because it needs no install and no store,
	 * desktop, then mobile, then the extension.
	 *
	 * Every claim here is checked against the repo rather than the roadmap:
	 * the web wallet is live, and the desktop, mobile and extension packaging
	 * workflows exist (`.github/workflows/*-package*.yml`) with a `desktop-v*`
	 * tag already cut. Nothing is on a store yet, so nothing here says it is —
	 * the store row says "coming soon" without a date, and GitHub Releases is
	 * offered as the thing that actually works today.
	 */
	let { data }: { data: PageData } = $props();

	const m = $derived(catalog(data.locale));
	const state = $derived(namespaceState('getStarted', data.locale));
	const alternates = $derived(translatedLocales('getStarted'));

	const RELEASES = 'https://github.com/mondaylabsltd/vela-wallet/releases';
	const BUILD_FROM_SOURCE = 'https://github.com/mondaylabsltd/vela-wallet#where-to-get-it';
	const WEB_WALLET = 'https://wallet.getvela.app/';
</script>

<Seo
	title={m.getStarted.meta.title}
	description={m.getStarted.meta.description}
	canonical={pathFor(data.locale, '/get-started')}
	locale={data.locale}
	{alternates}
	englishPath="/get-started"
/>

{#if state === 'fallback'}
	<TranslationNotice locale={data.locale} />
{/if}

<SiteHeader locale={data.locale} />

<main class="wrap">
	<header class="intro">
		<h1>{m.getStarted.heading}</h1>
		<p class="lede">{m.getStarted.lede}</p>
	</header>

	<ol class="platforms">
		<!-- Web — the recommended one, and the only row with a live primary action. -->
		<li class="platform primary">
			<div class="meta">
				<div class="line">
					<h2>{m.getStarted.platforms.web.title}</h2>
					<span class="tag tag-now">{m.getStarted.recommended}</span>
				</div>
				<p>{m.getStarted.platforms.web.blurb}</p>
			</div>
			<div class="actions">
				<a
					class="btn btn-primary"
					href={WEB_WALLET}
					target="_blank"
					rel="noopener"
					data-rybbit-event="cta_click"
					data-rybbit-prop-location="get-started-web"
				>
					{m.getStarted.platforms.web.action}
				</a>
			</div>
		</li>

		<!-- Which platforms have packages on GitHub is a ruling, not a layout
		     choice (spec 063 §0): desktop and the extension do, and everything
		     attached there installs as downloaded. The phone apps do NOT — they
		     ship through the stores, which are paid — so that card must never
		     point at Releases: for a week it did, and what people found there
		     was an APK the installer refused. Its second route is the source. -->
		{#each [{ platform: m.getStarted.platforms.desktop, onGithub: true }, { platform: m.getStarted.platforms.mobile, onGithub: false }, { platform: m.getStarted.platforms.extension, onGithub: true }] as { platform, onGithub }, i (platform.title)}
			<li class="platform">
				<div class="meta">
					<div class="line">
						<h2>{platform.title}</h2>
						<span class="systems">{platform.systems}</span>
					</div>
					<p>{platform.blurb}</p>
				</div>
				<div class="actions">
					<!-- The store is the preferred route and it is not open yet, so it
					     is shown as what it is: a named destination that is not ready.
					     A disabled-looking chip, never a link that goes nowhere. -->
					<span class="store">
						<span class="store-name">{platform.stores}</span>
						<span class="tag tag-soon">{m.getStarted.comingSoon}</span>
					</span>
					<a
						class="github"
						href={onGithub ? RELEASES : BUILD_FROM_SOURCE}
						target="_blank"
						rel="noopener"
						data-rybbit-event="cta_click"
						data-rybbit-prop-location="get-started-{onGithub ? 'github' : 'source'}-{i}"
					>
						{onGithub ? m.getStarted.githubCta : m.getStarted.sourceCta}
						<svg
							width="13"
							height="13"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							aria-hidden="true"
						>
							<path d="M15 3h6v6M10 14 21 3" stroke-linecap="round" stroke-linejoin="round" />
							<path
								d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					</a>
				</div>
			</li>
		{/each}
	</ol>

	<p class="note">{m.getStarted.storeNote}</p>
	<p class="note funding">{m.getStarted.fundingNote}</p>
</main>

<SiteFooter locale={data.locale} />

<style>
	.wrap {
		max-width: 860px;
		margin: 0 auto;
		padding: 64px 24px 40px;
	}
	.intro h1 {
		font-size: clamp(2.1rem, 4.5vw, 2.9rem);
		letter-spacing: -0.02em;
		line-height: 1.1;
		margin-bottom: 16px;
	}
	.lede {
		color: var(--text-secondary);
		font-size: 1.02rem;
		line-height: 1.7;
		max-width: 60ch;
	}

	/* Hairline-separated rows, no cards: the page is a list of four answers to
	   one question, and boxing each of them would make them look like four
	   products. */
	.platforms {
		list-style: none;
		margin: 48px 0 0;
		padding: 0;
		border-top: 1px solid var(--border);
	}
	.platform {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 32px;
		padding: 28px 0;
		border-bottom: 1px solid var(--border);
	}
	.platform.primary {
		padding-top: 30px;
		padding-bottom: 30px;
	}
	.meta {
		min-width: 0;
	}
	.line {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 10px;
		margin-bottom: 8px;
	}
	.line h2 {
		font-size: 1.22rem;
		letter-spacing: -0.01em;
	}
	.systems {
		font-size: 0.8rem;
		color: var(--text-tertiary);
	}
	.meta p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.65;
		max-width: 52ch;
	}

	.actions {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 10px;
		flex-shrink: 0;
	}
	.btn {
		display: inline-block;
		padding: 11px 22px;
		border-radius: 10px;
		font-size: 0.88rem;
		font-weight: 600;
		white-space: nowrap;
		transition: all 0.15s;
	}
	.btn-primary {
		background: var(--accent);
		color: var(--text-on-accent);
	}
	.btn-primary:hover {
		transform: translateY(-1px);
		box-shadow: 0 4px 16px color-mix(in srgb, var(--accent) 30%, transparent);
	}

	.store {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-size: 0.85rem;
		color: var(--text-tertiary);
		white-space: nowrap;
	}
	.store-name {
		font-weight: 500;
	}

	.tag {
		display: inline-block;
		padding: 2px 8px;
		border-radius: 999px;
		font-size: 0.66rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		white-space: nowrap;
	}
	.tag-now {
		color: var(--accent);
		background: var(--accent-soft);
	}
	.tag-soon {
		color: var(--text-tertiary);
		border: 1px solid var(--border);
	}
	:global(html[lang^='zh']) .tag,
	:global(html[lang='ja']) .tag,
	:global(html[lang='ko']) .tag {
		text-transform: none;
		letter-spacing: 0.02em;
	}

	.github {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-size: 0.82rem;
		color: var(--text-secondary);
		text-decoration: underline;
		text-underline-offset: 3px;
		white-space: nowrap;
	}
	.github:hover {
		color: var(--accent);
	}

	.note {
		margin-top: 28px;
		color: var(--text-muted);
		font-size: 0.85rem;
		line-height: 1.7;
		max-width: 62ch;
	}
	.note.funding {
		margin-top: 10px;
	}

	@media (max-width: 720px) {
		.platform {
			flex-direction: column;
			gap: 16px;
		}
		.actions {
			align-items: flex-start;
		}
		.meta p {
			max-width: none;
		}
	}
</style>
