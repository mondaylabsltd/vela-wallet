<script lang="ts">
	import { resolve } from '$app/paths';
	import LanguageSwitcher from '$lib/components/LanguageSwitcher.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { LOCALES, pathFor } from '$lib/i18n/locales';
	import { catalog, namespaceState, translatedLocales } from '$lib/i18n/resolve';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// Every string on this page comes from the catalog. `home` is translated as a
	// WHOLE or not at all — see resolve.ts — so this is either fully the reader's
	// language or fully English under a notice, never a mixture.
	const m = $derived(catalog(data.locale));
	const homeState = $derived(namespaceState('home', data.locale));
	const alternates = $derived(translatedLocales('home'));
	/** Internal links written English-relative, served under the reader's prefix. */
	const L = $derived((path: string) =>
		path.startsWith('#') ? path : pathFor(data.locale, path)
	);

	/**
	 * The colour of each comparison cell. Deliberately NOT in the message
	 * catalog: "is this good or bad" is a claim we make, and a translator
	 * shouldn't be able to change it — or silently lose it — while translating
	 * the words. Ordered to match `m.home.compare.rows`; an empty string is a
	 * neutral cell, which the first row needs because all three wallets keep the
	 * key in the same place and pretending otherwise would be the cheapest
	 * possible lie.
	 */
	/**
	 * Where each hero claim is proved. Ordered to match `m.home.hero.facts`; the
	 * catalog holds the label, this holds the destination, so a translation can
	 * never point a reader somewhere else.
	 */
	/**
	 * Where each hook's proof lives, in the hooks' own order. The hrefs are here
	 * rather than in the catalog so a translation can change the LABEL and never
	 * the destination.
	 */
	const FACT_LINKS = [
		{ href: '/docs/account-contract', external: false },
		{ href: '/docs/signers', external: false },
		{ href: '/docs/bybit-attack', external: false },
		{
			href: 'https://github.com/mondaylabsltd/vela-wallet#self-deploy-service-endpoints',
			external: true
		}
	] as const;

	const COMPARE_TONES = [
		{ vela: '', base: '', safe: '' },
		{ vela: 'yes', base: '', safe: 'yes' },
		{ vela: 'warn', base: 'no', safe: 'warn' },
		{ vela: 'yes', base: '', safe: 'yes' },
		{ vela: 'yes', base: 'warn', safe: 'warn' },
		{ vela: 'yes', base: 'no', safe: 'warn' },
		{ vela: 'yes', base: 'warn', safe: 'warn' },
		{ vela: '', base: '', safe: '' },
		{ vela: 'yes', base: 'no', safe: 'yes' },
		{ vela: 'yes', base: 'no', safe: 'warn' },
		{ vela: '', base: '', safe: '' }
	] as const;

	// Analytics helper
	interface RybbitWindow extends Window {
		rybbit?: { event: (name: string, props?: Record<string, string | number>) => void };
	}
	function track(event: string, props?: Record<string, string | number>) {
		try {
			(globalThis as unknown as RybbitWindow).rybbit?.event(event, props);
		} catch {
			/* noop */
		}
	}

	$effect(() => {
		// Section visibility tracking
		const seen: Record<string, boolean> = {};
		const observer = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					const id = entry.target.id;
					if (entry.isIntersecting && id && !seen[id]) {
						seen[id] = true;
						track('section_viewed', { section: id });
					}
				}
			},
			{ threshold: 0.3 }
		);
		for (const el of document.querySelectorAll('section[id]')) observer.observe(el);

		// FAQ click tracking
		function onFaqToggle(e: Event) {
			const details = (e.target as HTMLElement).closest('details');
			if (details?.open) {
				const q = details.querySelector('summary')?.textContent?.trim() ?? '';
				track('faq_opened', { question: q.slice(0, 80) });
			}
		}
		const faqList = document.querySelector('.faq-list');
		faqList?.addEventListener('toggle', onFaqToggle, true);

		return () => {
			observer.disconnect();
			faqList?.removeEventListener('toggle', onFaqToggle, true);
		};
	});

	const FALLBACK_RPCS = [
		'https://rpc.gnosischain.com',
		'https://rpc.gnosis.gateway.fm',
		'https://gnosis-rpc.publicnode.com',
		'https://rpc.ankr.com/gnosis',
		'https://gnosis-mainnet.public.blastapi.io',
		'https://gnosis.blockpi.network/v1/rpc/public',
		'https://gnosis.drpc.org',
		'https://1rpc.io/gnosis',
		'https://gnosis.oat.farm'
	];

	const RPC_SOURCE = 'https://ethereum-data.awesometools.dev/chains/eip155-100.json';
	// Wallets created on-chain = the current possession-proven registry's group
	// count PLUS the legacy index's count, so the number reflects the new
	// contract without dropping the pre-migration wallets.
	const CURRENT_CONTRACT = '0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9';
	// getGroupsByRpId("getvela.app", 0, 0, false) — the wallet (group) count under
	// Vela's rpId. NOT getTotalUnits(): that is a global count across every rpId,
	// so probe/localhost dev wallets would inflate the number and it would disagree
	// with the /registry page, which filters by rpId. limit=0 returns (total, []).
	const CURRENT_CALLDATA =
		'0x847c874f0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b67657476656c612e617070000000000000000000000000000000000000000000';
	const LEGACY_CONTRACT = '0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3';
	// getTotalCredentialsByRpId("getvela.app") on the legacy index.
	const LEGACY_CALLDATA =
		'0x3ebcb2150000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b67657476656c612e617070000000000000000000000000000000000000000000';

	let rpcs = [...FALLBACK_RPCS];
	let displayCount = $state(0);
	let countReady = $state(false); // true once a count (even 0) has been read
	let countFailed = $state(false); // true after repeated failures with no read yet
	let failCount = 0;
	let rpcIndex = 0;

	async function refreshRpcs() {
		try {
			const res = await fetch(RPC_SOURCE);
			const data = await res.json();
			const urls: string[] = (data?.rpc ?? [])
				.filter((r: { url: string }) => r.url.startsWith('https://') && !r.url.includes('${'))
				.map((r: { url: string }) => r.url);
			if (urls.length > 0) {
				// Augment, don't replace: always keep the vetted built-ins in the pool so
				// a degraded community list can't shadow the known-good endpoints.
				rpcs = [...new Set([...urls, ...FALLBACK_RPCS])];
				rpcIndex = 0;
			}
		} catch {
			// keep using current rpcs
		}
	}

	/** One `eth_call`, reading the first 32-byte return word as a uint, with RPC
	 *  failover. Slicing the first word (not parsing the whole payload) keeps this
	 *  correct for multi-word returns like getGroupsByRpId's `(total, unitIds)`. */
	async function callUint(to: string, data: string): Promise<number | null> {
		let attempts = 0;
		while (attempts < rpcs.length) {
			const rpc = rpcs[rpcIndex % rpcs.length];
			attempts++;
			try {
				const res = await fetch(rpc, {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({
						jsonrpc: '2.0',
						id: 1,
						method: 'eth_call',
						params: [{ to, data }, 'latest']
					})
				});
				const json = await res.json();
				if (typeof json.result === 'string' && json.result.length >= 66) {
					return parseInt(json.result.slice(0, 66), 16);
				}
			} catch {
				// failover to next RPC
			}
			rpcIndex++;
		}
		return null;
	}

	/** Wallets created on-chain = current registry groups + legacy index count.
	 *  Both must answer; a single missing read fails the whole number rather than
	 *  quietly under-reporting. */
	async function fetchCount(): Promise<number | null> {
		const current = await callUint(CURRENT_CONTRACT, CURRENT_CALLDATA);
		if (current === null) return null;
		const legacy = await callUint(LEGACY_CONTRACT, LEGACY_CALLDATA);
		if (legacy === null) return null;
		return current + legacy;
	}

	function animateCount(target: number) {
		const start = displayCount;
		const diff = target - start;
		if (diff === 0) return;
		const duration = Math.min(2000, Math.max(800, Math.abs(diff) * 80));
		const startTime = performance.now();

		function step(now: number) {
			const elapsed = now - startTime;
			const progress = Math.min(elapsed / duration, 1);
			const eased = 1 - Math.pow(1 - progress, 3);
			displayCount = Math.round(start + diff * eased);
			if (progress < 1) requestAnimationFrame(step);
		}
		requestAnimationFrame(step);
	}

	async function poll() {
		const count = await fetchCount();
		if (count !== null) {
			failCount = 0;
			countFailed = false;
			countReady = true;
			animateCount(count);
		} else if (!countReady) {
			// Stop shimmering after a run of failures with no successful read, so the
			// chip doesn't loop forever; a later successful poll self-heals it.
			failCount++;
			if (failCount >= 4) countFailed = true;
		}
	}

	$effect(() => {
		refreshRpcs().then(() => poll());
		const pollInterval = setInterval(poll, 5_000);
		const rpcRefreshInterval = setInterval(refreshRpcs, 10 * 60_000);
		return () => {
			clearInterval(pollInterval);
			clearInterval(rpcRefreshInterval);
		};
	});

	// Built-in networks (mirrors the wallet's DEFAULT_NETWORKS). Logos are the
	// same source the wallet itself uses, so they always match in-app.
	const NETWORKS = [
		{ name: 'Ethereum', chainId: 1 },
		{ name: 'BNB Chain', chainId: 56 },
		{ name: 'Polygon', chainId: 137 },
		{ name: 'Arbitrum', chainId: 42161 },
		{ name: 'Optimism', chainId: 10 },
		{ name: 'Base', chainId: 8453 },
		{ name: 'Avalanche', chainId: 43114 },
		{ name: 'Gnosis', chainId: 100 },
		{ name: 'Unichain', chainId: 130 },
		{ name: 'Monad', chainId: 143 },
		{ name: 'World Chain', chainId: 480 },
		{ name: 'Tempo', chainId: 4217 }
	];
	const chainLogo = (chainId: number) =>
		`https://ethereum-data.awesometools.dev/chainlogos/eip155-${chainId}.png`;

	// Structured data on the root page binds the "Vela Wallet" brand entity to
	// getvela.app. It feeds Google the canonical site name (so results read
	// "Vela Wallet", not "getvela.app") and the organization behind it — the
	// signals that make us eligible for branded-query sitelinks. No SearchAction:
	// Google deprecated the sitelinks searchbox in 2024.
	const structuredData = $derived([
		{
			'@context': 'https://schema.org',
			'@type': 'Organization',
			'@id': `${seoConfig.domain}/#organization`,
			name: seoConfig.siteName,
			legalName: 'MONDAY LABS LTD',
			url: seoConfig.domain,
			logo: `${seoConfig.domain}/vela-logo.png`,
			description: m.home.meta.organization,
			sameAs: [
				'https://github.com/mondaylabsltd/vela-wallet',
				'https://x.com/realvelawallet',
				'https://t.me/velawallet'
			]
		},
		{
			'@context': 'https://schema.org',
			'@type': 'WebSite',
			'@id': `${seoConfig.domain}/#website`,
			name: seoConfig.siteName,
			url: seoConfig.domain,
			inLanguage: data.locale,
			publisher: { '@id': `${seoConfig.domain}/#organization` }
		}
	]);
	// Serialize the structured data into a JSON-LD script block for the document
	// head. The closing tag is split across two string literals ("</scr" + "ipt>")
	// so the complete closing-script token never appears literally anywhere in this
	// module's source — if it did, the Svelte parser would read it as the end of the
	// component's own script block and orphan everything after it. Every less-than
	// char in the JSON payload is escaped so the data can never break out of the tag.
	const structuredDataHtml = $derived(
		`<script type="application/ld+json">${JSON.stringify(structuredData).replace(/</g, '\\u003c')}</scr` +
			`ipt>`
	);
</script>

<svelte:head>
	<title>{m.home.meta.title}</title>
	<meta name="description" content={m.home.meta.description} />
	<meta property="og:title" content={m.home.meta.ogTitle} />
	<meta property="og:description" content={m.home.meta.ogDescription} />
	<meta property="og:image" content="https://getvela.app/getvela-app-preview.png" />
	<meta property="og:url" content="{seoConfig.domain}{pathFor(data.locale, '/')}" />
	<meta property="og:locale" content={LOCALES[data.locale].ogLocale} />
	<link rel="canonical" href="{seoConfig.domain}{pathFor(data.locale, '/')}" />
	<!-- Only the locales that genuinely HAVE this page are advertised: a page in
	     fallback is an English page at a localized URL, and telling a crawler
	     otherwise is what earns a duplicate-content penalty (FR-018). -->
	{#each alternates as alt (alt)}
		<link rel="alternate" hreflang={alt} href="{seoConfig.domain}{pathFor(alt, '/')}" />
	{/each}
	<link rel="alternate" hreflang="x-default" href="{seoConfig.domain}/" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:image" content="https://getvela.app/getvela-app-preview.png" />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html structuredDataHtml}
</svelte:head>

{#if homeState === 'fallback'}
	<TranslationNotice locale={data.locale} />
{/if}

<!-- Nav -->
<nav>
	<div class="nav-inner">
		<div class="brand">
			<a href={resolve('/')} class="logo">
				<img src="/vela-logo.png" alt="Vela Wallet" width="28" height="28" />
				<span>Vela Wallet</span>
			</a>
			<a
				href={resolve('/blog/vela-is-in-alpha')}
				class="logo-tag"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="logo-tag"
			>
				<span class="logo-tag-dot"></span>
				{m.chrome.nav.alpha}
			</a>
		</div>
		<div class="nav-links">
			<a href="#why">{m.chrome.nav.whyVela}</a>
			<a href="#how-it-works">{m.chrome.nav.howItWorks}</a>
			<a href="#pricing">{m.chrome.nav.pricing}</a>
			<a href="#faq">{m.chrome.nav.faq}</a>

			<a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
				>GitHub</a
			>
			<a
				href="https://wallet.getvela.app/"
				target="_blank"
				rel="noopener"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="nav-signin">{m.chrome.nav.signIn}</a
			>
		</div>
		<LanguageSwitcher locale={data.locale} />
		<ThemeToggle />
	</div>
</nav>

<!-- Hero -->
<section class="hero">
	<div class="container hero-grid">
		<div class="hero-text">
			<h1>{m.home.hero.headline}</h1>
			<p class="subtitle">{m.home.hero.subtitle}</p>
			<div class="hero-cta">
				<div class="hero-buttons">
					<!-- Not straight to the web wallet any more: the same wallet now ships
					     as a desktop app, two mobile apps and an extension, so the first
					     click is a choice, not a destination. -->
					<a
						href={pathFor(data.locale, '/get-started')}
						class="btn btn-primary btn-hero"
						data-rybbit-event="cta_click"
						data-rybbit-prop-location="hero">{m.home.hero.ctaCreate}</a
					>
					<a
						href="https://github.com/mondaylabsltd/vela-wallet"
						target="_blank"
						rel="noopener"
						class="btn btn-outline btn-hero"
						data-rybbit-event="cta_click"
						data-rybbit-prop-location="hero-code">{m.home.hero.ctaCode}</a
					>
				</div>
			</div>

			<!-- The on-chain wallet count, stamped rather than stated. It is the one
			     number on this page that nobody can fake: it is read live from the
			     registry contract, so it is presented like a seal on a document —
			     and it links to the registry so the claim can be checked. Hidden
			     entirely when every RPC fails: a seal with no number is worse than
			     no seal. -->
			{#if !countFailed}
				<a
					class="seal"
					href={resolve('/registry')}
					title={m.home.seal.verify}
					data-rybbit-event="registry_open"
				>
					<span class="seal-dot" aria-hidden="true"></span>
					{#if countReady}
						<span class="seal-count">{displayCount.toLocaleString()}</span>
					{:else}
						<span class="seal-skeleton" aria-label={m.home.seal.loading}></span>
					{/if}
					<span class="seal-label">{m.home.seal.label}</span>
				</a>
			{/if}
		</div>
		<!-- The three facts a sceptic checks before trusting a wallet with money.
		     Every line is verifiable from the page below it or from the repo —
		     nothing here is a claim the docs don't already make. ERC-4337 and
		     "100% open source" used to be rows of their own; both are still stated
		     in the trust strip and the technical-details table below. -->
		<dl class="hero-facts">
			{#each m.home.hero.facts as fact, i (fact.term)}
				<div class="fact">
					<dt>{fact.term}</dt>
					<dd>
						{#if FACT_LINKS[i]?.external}
							<a class="fact-link" href={FACT_LINKS[i].href} target="_blank" rel="noopener"
								>{fact.link}</a
							>
						{:else}
							<a class="fact-link" href={L(FACT_LINKS[i].href)}>{fact.link}</a>
						{/if}
					</dd>
				</div>
			{/each}
		</dl>
	</div>
	<div class="scroll-hint">
		<svg
			width="20"
			height="20"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			stroke-width="2"
			><path
				d="M19.5 13.5L12 21m0 0l-7.5-7.5M12 21V3"
				stroke-linecap="round"
				stroke-linejoin="round"
			/></svg
		>
	</div>
</section>

<!-- Why (the short version; the essay lives at /docs/why-vela) -->
<section id="why" class="why">
	<div class="container">
		<div class="why-content">
			<h2>{m.home.why.heading}</h2>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p>{@html m.home.why.p1}</p>
			<p class="why-beat">{m.home.why.p2}</p>
			<a class="more-link" href={L('/docs/why-vela')} data-rybbit-event="why_long_version"
				>{m.home.why.more}</a
			>
		</div>
	</div>
</section>

<!-- The trade-offs, before the pitch goes any further -->
<section id="trade-offs" class="tradeoffs">
	<div class="container">
		<div class="tradeoffs-content">
			<h2>{m.home.tradeoffs.heading}</h2>
			<p class="tradeoffs-lede">{m.home.tradeoffs.lede}</p>
			<ul class="tradeoff-list">
				{#each m.home.tradeoffs.items as item (item.title)}
					<li class="tradeoff">
						<h3>{item.title}</h3>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -->
						<p>{@html item.body}</p>
					</li>
				{/each}
			</ul>
			<p class="tradeoffs-close">{m.home.tradeoffs.close}</p>
		</div>
	</div>
</section>

<!-- Sign what you see -->
<section id="signing" class="signing">
	<div class="container">
		<div class="signing-content">
			<h2>{m.home.signing.heading}</h2>
			<p class="signing-lede">{m.home.signing.lede}</p>

			<div class="signing-block">
				<span class="signing-label">{m.home.signing.today.label}</span>
				<h3>{m.home.signing.today.title}</h3>
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				<p>{@html m.home.signing.today.body}</p>
			</div>

			<div class="signing-block signing-next">
				<span class="signing-label signing-label-soon">{m.home.signing.next.label}</span>
				<h3>{m.home.signing.next.title}</h3>
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				<p>{@html m.home.signing.next.body}</p>
				<div class="signing-aside">
					<h4>{m.home.signing.aside.title}</h4>
					<ul>
						{#each m.home.signing.aside.items as line (line)}
							<li>{line}</li>
						{/each}
					</ul>
				</div>
			</div>
		</div>
	</div>
</section>

<!-- Compare -->
<section id="compare" class="compare">
	<div class="container">
		<h2>{m.home.compare.heading}</h2>
		<p class="section-desc">{m.home.compare.desc}</p>
		<div class="compare-table-wrap">
			<table class="compare-table">
				<thead>
					<tr>
						<th></th>
						<th>Vela</th>
						<th>Base Account</th>
						<th>Safe&#123;Wallet&#125; + passkey</th>
					</tr>
				</thead>
				<tbody>
					{#each m.home.compare.rows as row, i (row.feature)}
						<tr>
							<td>{row.feature}</td>
							<!-- eslint-disable svelte/no-at-html-tags -->
							<td class={COMPARE_TONES[i]?.vela}>{@html row.vela}</td>
							<td class={COMPARE_TONES[i]?.base}>{@html row.base}</td>
							<td class={COMPARE_TONES[i]?.safe}>{@html row.safe}</td>
							<!-- eslint-enable svelte/no-at-html-tags -->
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="compare-note">{m.home.compare.note}</p>
	</div>
</section>

<!-- Pricing -->
<section id="pricing" class="business-model">
	<div class="container">
		<div class="bm-content">
			<h2>{m.home.pricing.heading}</h2>
			<p class="bm-intro">{m.home.pricing.intro}</p>

			<div class="bm-grid">
				{#each m.home.pricing.cards as card (card.title)}
					<div class="bm-card">
						<h4>{card.title}</h4>
						<div class="bm-price">{card.price}</div>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -->
						<p>{@html card.body}</p>
					</div>
				{/each}
			</div>

			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p class="bm-note">{@html m.home.pricing.note}</p>
		</div>
	</div>
</section>

<!-- How It Works -->
<section id="how-it-works" class="how-it-works">
	<div class="container">
		<h2>{m.home.how.heading}</h2>
		<p class="section-desc">{m.home.how.desc}</p>

		<ol class="steps">
			{#each m.home.how.steps as step, i (step.title)}
				<li class="step">
					<span class="step-number">{String(i + 1).padStart(2, '0')}</span>
					<h3>{step.title}</h3>
					<p>{step.body}</p>
				</li>
			{/each}
		</ol>

		<!-- The spec line. Every claim above is one of these five links; the detail
		     that used to be a six-row table now points at the pages that own it. -->
		<div class="stack">
			<span class="stack-label">{m.home.how.stack.label}</span>
			<ul class="stack-list">
				<li>
					<a
						href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
						target="_blank"
						rel="noopener">Safe v1.4.1</a
					>
				</li>
				<li>
					<a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener"
						>ERC-4337</a
					>
				</li>
				<li>
					<a href="https://www.w3.org/TR/webauthn-2/" target="_blank" rel="noopener">WebAuthn</a> / P-256
				</li>
				<li>
					<a
						href="https://github.com/safe-global/safe-modules/tree/main/modules/passkey/contracts/4337"
						target="_blank"
						rel="noopener">SafeWebAuthnSharedSigner</a
					>
				</li>
				<li>
					<a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
						>GitHub</a
					>
				</li>
			</ul>
			<a class="more-link" href={L('/docs/whitepaper')}>{m.home.how.stack.link}</a>
		</div>
	</div>
</section>

<!-- Networks -->
<section id="networks" class="networks">
	<div class="container">
		<h2>{m.home.networks.heading}</h2>
		<ul class="network-grid">
			{#each NETWORKS as net (net.chainId)}
				<li class="network-chip">
					<img
						class="network-logo"
						src={chainLogo(net.chainId)}
						alt=""
						width="26"
						height="26"
						loading="lazy"
					/>
					{net.name}
				</li>
			{/each}
		</ul>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<p class="network-note">{@html m.home.networks.body}</p>
		<a class="more-link" href={L('/docs/networks-and-fees')}>{m.home.networks.link}</a>
	</div>
</section>

<!-- FAQ -->
<section id="faq" class="faq">
	<div class="container">
		<h2>{m.home.faq.heading}</h2>
		<p class="section-desc">{m.home.faq.desc}</p>
		<div class="faq-list">
			{#each m.home.faq.items as item (item.q)}
				<details>
					<summary>{item.q}</summary>
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					<p>{@html item.a}</p>
				</details>
			{/each}
		</div>
	</div>
</section>

<!-- Footer -->
<SiteFooter />

<style>
	/* Palette comes entirely from the global tokens (src/lib/styles/tokens.css). */

	/* ── Base ── */
	.container {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 0 24px;
	}
	section {
		padding: 120px 0;
	}

	h2 {
		font-size: 2rem;
		letter-spacing: -0.02em;
		margin-bottom: 12px;
		line-height: 1.2;
	}

	.section-desc {
		color: var(--text-secondary);
		max-width: 480px;
		margin: 0 auto 56px;
		font-size: 1rem;
		line-height: 1.7;
	}

	/* ── Nav ── */
	nav {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 100;
		background: var(--bg-header);
		backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--border);
	}
	.nav-inner {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 0 24px;
		height: 52px;
		display: flex;
		align-items: center;
		gap: 18px;
	}
	.logo {
		display: flex;
		align-items: center;
		gap: 9px;
		font-weight: 700;
		font-size: 1.1rem;
		letter-spacing: 0.5px;
	}
	.logo img {
		border-radius: 7px;
	}
	.nav-links {
		display: flex;
		gap: 24px;
	}
	.nav-links a {
		color: var(--text-secondary);
		font-size: 0.82rem;
		font-weight: 500;
		transition: color 0.15s;
	}
	.nav-links a:hover {
		color: var(--text);
	}

	/* ── Hero ── */
	.hero {
		padding: 80px 0 80px;
		min-height: 100vh;
		display: flex;
		align-items: center;
		position: relative;
	}
	.hero-grid {
		display: grid;
		grid-template-columns: 1.05fr 0.95fr;
		gap: 64px;
		/* Both columns run the full height of the spread, so the headline and the
		   first claim share a top edge and the seal and the last claim share a
		   bottom one. Nothing floats. */
		align-items: stretch;
	}
	.hero-text {
		text-align: left;
		display: flex;
		flex-direction: column;
	}
	h1 {
		font-size: clamp(2.75rem, 6vw, 4.5rem);
		font-weight: 700;
		line-height: 1.02;
		letter-spacing: -0.03em;
		margin-bottom: 28px;
		/* The headline is a sentence now, not two two-word lines, so it has no
		   hard break: `balance` keeps the wrap even at every width instead of
		   leaving one orphan word on the last line. Browsers without it simply
		   wrap normally. */
		text-wrap: balance;
	}
	/* CJK glyphs fill their em box, so a headline set at the Latin size reads
	   noticeably larger and runs out of line much sooner — 真正属于你的以太坊钱包
	   wrapped mid-word (你 | 的) at the Latin scale. `word-break: auto-phrase`
	   is the nominally correct fix and measurably does nothing here, so this
	   uses the answer CJK typography has always used: a slightly smaller scale
	   and a little more leading. `html[lang]` is set per locale in
	   hooks.server.ts, so this follows the page, not the browser. */
	:global(html[lang^='zh']) h1,
	:global(html[lang='ja']) h1,
	:global(html[lang='ko']) h1 {
		font-size: clamp(2.35rem, 4.6vw, 3.6rem);
		line-height: 1.2;
		letter-spacing: -0.01em;
	}
	/* One sentence now, so it can carry the weight of a standfirst rather than
	   reading as body text under a headline. */
	.subtitle {
		color: var(--text-secondary);
		font-size: 1.2rem;
		line-height: 1.6;
		max-width: 460px;
		margin-bottom: 36px;
	}
	.hero-cta {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
		margin-bottom: 24px;
	}
	.hero-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
	}
	.btn.btn-hero {
		padding: 20px 44px;
		min-width: 230px;
		text-align: center;
		font-size: 1.02rem;
		border-radius: 12px;
	}

	/* The fact column. Each claim sits on its own rule, the way a well-set page
	   of an annual report does — an indented list hanging off one vertical line
	   read as a spec sheet, which is exactly what these five lines are not. */
	.hero-facts {
		/* The rules must not outrun the words: the column is capped at the text
		   measure so every hairline ends where the line above it does. */
		max-width: 520px;
		height: 100%;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
	}
	.hero-facts .fact {
		padding: 20px 0;
		border-top: 1px solid var(--border);
	}
	.hero-facts .fact:first-child {
		border-top: none;
		padding-top: 0;
	}
	.hero-facts dt {
		font-weight: 600;
		font-size: 1rem;
		line-height: 1.4;
		letter-spacing: -0.01em;
		color: var(--text);
		margin-bottom: 7px;
	}
	/* The proof, not the explanation: every claim above is one link away from
	   the page that can check it. */
	.hero-facts dd {
		margin: 0;
	}
	.fact-link {
		font-size: 0.82rem;
		color: var(--text-tertiary);
		text-decoration: none;
		transition: color 0.15s;
	}
	.fact-link::after {
		content: ' →';
		transition: margin-left 0.15s;
	}
	.fact-link:hover {
		color: var(--accent);
	}
	.fact-link:hover::after {
		margin-left: 3px;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		/* Pushes .nav-links + the theme toggle to the right edge; the toggle
		   stays visible on mobile even when .nav-links is hidden. */
		margin-right: auto;
	}
	.logo-tag {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 3px 9px;
		border-radius: 999px;
		font-size: 0.68rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--text-secondary);
		background: var(--bg-raised);
		border: 1px solid var(--border);
		transition:
			color 0.15s,
			border-color 0.15s;
	}
	.logo-tag:hover {
		color: var(--text);
		border-color: var(--text-tertiary);
	}
	.logo-tag-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--accent);
		box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 50%, transparent);
		animation: pulse-dot 2s ease-in-out infinite;
	}

	/* ── Scroll Hint ── */
	.scroll-hint {
		position: absolute;
		bottom: 32px;
		left: 50%;
		transform: translateX(-50%);
		color: var(--text-tertiary);
		opacity: 0.5;
		animation: breathe 2.5s ease-in-out infinite;
	}
	@keyframes breathe {
		0%,
		100% {
			opacity: 0.2;
			transform: translateX(-50%) translateY(0);
		}
		50% {
			opacity: 0.6;
			transform: translateX(-50%) translateY(6px);
		}
	}

	/* Shimmer for the seal's loading placeholder. */
	@keyframes stat-shimmer {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}
	@keyframes pulse-dot {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.35;
		}
	}

	/* ── Buttons ── */
	.btn {
		display: inline-block;
		padding: 11px 22px;
		border-radius: 10px;
		font-size: 0.88rem;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s;
		border: none;
		font-family: inherit;
	}
	.btn-primary {
		background: var(--accent);
		color: var(--text-on-accent);
	}
	.btn-primary:hover {
		transform: translateY(-1px);
		box-shadow: 0 4px 16px color-mix(in srgb, var(--accent) 30%, transparent);
	}
	.btn-outline {
		background: transparent;
		color: var(--text);
		border: 1px solid var(--border);
	}
	.btn-outline:hover {
		border-color: var(--accent);
		color: var(--accent);
		transform: translateY(-1px);
	}

	/* The one link shape that carries a reader off this page and into the docs.
	   Not a button: the page is not asking, it is offering. */
	.more-link {
		display: inline-block;
		margin-top: 8px;
		font-size: 0.88rem;
		font-weight: 600;
		color: var(--accent);
		text-decoration: none;
	}
	.more-link::after {
		content: ' →';
		transition: margin-left 0.15s;
	}
	.more-link:hover::after {
		margin-left: 3px;
	}

	/* ── Text links ── */
	/* Inline links keep the surrounding ink color — no accent highlight. The
	   underline offset clears descenders (g, y) so the line stays unbroken. */
	.why-content :global(a:not(.more-link)),
	.tradeoff :global(a),
	.signing-block :global(a),
	.network-note :global(a),
	.compare-table td :global(a),
	.bm-card :global(a),
	.bm-note :global(a),
	details :global(a) {
		color: inherit;
		text-decoration: underline;
		text-underline-offset: 4px;
		transition: color 0.15s;
	}
	.why-content :global(a:not(.more-link):hover),
	.tradeoff :global(a:hover),
	.signing-block :global(a:hover),
	.network-note :global(a:hover),
	.compare-table td :global(a:hover),
	.bm-card :global(a:hover),
	.bm-note :global(a:hover),
	details :global(a:hover) {
		color: var(--accent);
	}
	/* External links get a trailing lucide external-link glyph, masked in
	   currentColor so it follows the link color. */
	.why-content :global(a[target='_blank']::after),
	.tradeoff :global(a[target='_blank']::after),
	.signing-block :global(a[target='_blank']::after),
	.network-note :global(a[target='_blank']::after),
	.compare-table td :global(a[target='_blank']::after),
	.bm-card :global(a[target='_blank']::after),
	.bm-note :global(a[target='_blank']::after),
	details :global(a[target='_blank']::after) {
		content: '';
		display: inline-block;
		width: 0.72em;
		height: 0.72em;
		margin-left: 0.28em;
		vertical-align: -0.02em;
		background: currentColor;
		mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M15 3h6v6'/%3E%3Cpath d='M10 14 21 3'/%3E%3Cpath d='M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6'/%3E%3C/svg%3E")
			center / contain no-repeat;
	}

	/* ── The on-chain seal ──
	   A stamp, not a chip: two hairline rings, a slight rotation, and the number
	   set in the brand serif. Rotated back to square on hover so it reads as a
	   thing you can press. */
	.seal {
		position: relative;
		display: inline-flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		margin-top: auto;
		align-self: flex-start;
		padding: 15px 26px 13px;
		border: 1.5px solid color-mix(in srgb, var(--accent) 55%, transparent);
		border-radius: 14px;
		background: var(--accent-soft);
		color: var(--text-secondary);
		text-decoration: none;
		transform: rotate(-2.5deg);
		transition:
			transform 0.25s cubic-bezier(0.2, 0.9, 0.3, 1),
			border-color 0.2s,
			box-shadow 0.25s;
	}
	/* The inner ring — the detail that makes it read as a stamp rather than a
	   card. Inset so it never touches the outer one. */
	.seal::before {
		content: '';
		position: absolute;
		inset: 4px;
		border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
		border-radius: 10px;
		pointer-events: none;
	}
	.seal:hover {
		transform: rotate(0deg);
		border-color: var(--accent);
		box-shadow: 0 6px 22px color-mix(in srgb, var(--accent) 16%, transparent);
	}
	.seal-count {
		font-family: var(--font-serif);
		font-size: 2.25rem;
		font-weight: 700;
		line-height: 1.05;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
		color: var(--accent);
	}
	.seal-label {
		font-size: 0.66rem;
		font-weight: 600;
		letter-spacing: 0.09em;
		text-transform: uppercase;
		color: color-mix(in srgb, var(--accent) 75%, var(--text-secondary));
		text-align: center;
		/* One line. A stamp's band does not wrap — and "ON-CHAIN" breaking at its
		   own hyphen was the first thing the eye caught. */
		white-space: nowrap;
		line-height: 1.4;
	}
	/* CJK has no case, so the uppercase transform does nothing and the wide
	   tracking just pulls the characters apart. */
	:global(html[lang^='zh']) .seal-label,
	:global(html[lang='ja']) .seal-label,
	:global(html[lang='ko']) .seal-label {
		text-transform: none;
		letter-spacing: 0.02em;
		font-size: 0.72rem;
	}
	.seal-dot {
		position: absolute;
		top: 10px;
		right: 10px;
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--green);
		box-shadow: 0 0 6px color-mix(in srgb, var(--green) 50%, transparent);
		animation: pulse-dot 2s ease-in-out infinite;
	}
	.seal-skeleton {
		display: block;
		width: 72px;
		height: 2.25rem;
		border-radius: 6px;
		background: linear-gradient(90deg, var(--border), var(--border-strong), var(--border));
		background-size: 200% 100%;
		animation: stat-shimmer 1.3s ease-in-out infinite;
	}

	/* ── Networks ── */
	.networks {
		padding-top: 0;
	}
	.networks h2 {
		max-width: 760px;
		margin: 0 auto 32px;
		text-align: center;
		/* Two chains of words, not one line plus an orphan. */
		text-wrap: balance;
	}
	.network-grid {
		list-style: none;
		padding: 0;
		margin: 0 auto;
		max-width: 760px;
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 18px 24px;
	}
	.network-chip {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 0.88rem;
		font-weight: 500;
		color: var(--text);
		white-space: nowrap;
	}
	.network-logo {
		width: 26px;
		height: 26px;
		border-radius: 50%;
		flex-shrink: 0;
		background: var(--border);
	}
	.network-note {
		max-width: 640px;
		margin: 40px auto 0;
		color: var(--text-secondary);
		font-size: 0.88rem;
		line-height: 1.75;
	}
	.networks .more-link {
		display: block;
		max-width: 640px;
		margin: 12px auto 0;
	}

	/* ── Why ── */
	.why {
		margin-top: 12px;
		padding: 96px 0 72px;
		border-top: 1px solid var(--border);
	}
	.why-content {
		max-width: 640px;
		margin: 0 auto;
	}
	.why-content h2 {
		text-align: left;
		margin-bottom: 20px;
	}
	.why-content p {
		color: var(--text-secondary);
		font-size: 1rem;
		line-height: 1.8;
		margin-bottom: 16px;
	}
	.why-content .why-beat {
		color: var(--text);
		font-size: 1.05rem;
		line-height: 1.6;
		margin: 20px 0 4px;
	}

	/* ── The trade-offs ──
	   Hairlines, not cards. Three admissions in a row inside three boxes would
	   read as a feature grid; stacked on rules they read as a list of things we
	   are telling you. */
	.tradeoffs {
		padding: 0 0 96px;
	}
	.tradeoffs-content {
		max-width: 640px;
		margin: 0 auto;
	}
	.tradeoffs-lede {
		color: var(--text-secondary);
		font-size: 0.95rem;
		margin: 8px 0 0;
	}
	.tradeoff-list {
		list-style: none;
		padding: 0;
		margin: 28px 0 0;
	}
	.tradeoff {
		padding: 26px 0;
		border-top: 1px solid var(--border);
	}
	.tradeoff h3 {
		font-size: 1rem;
		font-weight: 600;
		line-height: 1.45;
		margin-bottom: 10px;
	}
	.tradeoff p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.75;
		margin: 0;
	}
	.tradeoffs-close {
		margin: 26px 0 0;
		padding-top: 22px;
		border-top: 1px solid var(--border);
		color: var(--text);
		font-size: 0.95rem;
		line-height: 1.7;
	}

	/* ── Sign what you see ──
	   Two blocks that must never blur into one: what the wallet does today, and
	   what is built but not deployed. The second one says so on its label. */
	.signing {
		padding: 0 0 96px;
	}
	.signing-content {
		max-width: 640px;
		margin: 0 auto;
	}
	.signing-lede {
		color: var(--text-secondary);
		font-size: 1rem;
		line-height: 1.8;
		margin: 16px 0 0;
	}
	.signing-block {
		margin-top: 36px;
		padding-top: 26px;
		border-top: 1px solid var(--border);
	}
	.signing-label {
		display: inline-block;
		margin-bottom: 12px;
		padding: 3px 9px;
		border-radius: 999px;
		border: 1px solid var(--border-accent, var(--border));
		background: var(--accent-soft);
		color: var(--accent);
		font-size: 0.68rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	/* Not accent-coloured: this one is a caveat, not a badge of honour. */
	.signing-label-soon {
		border: 1px dashed var(--border-strong, var(--border));
		background: transparent;
		color: var(--text-tertiary);
	}
	.signing-block h3 {
		font-size: 1.05rem;
		font-weight: 600;
		line-height: 1.45;
		margin-bottom: 10px;
	}
	.signing-block p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.75;
		margin: 0;
	}
	.signing-aside {
		margin-top: 22px;
		padding: 18px 20px;
		border-left: 2px solid var(--border-accent, var(--border));
		background: var(--bg-raised);
		border-radius: 0 var(--radius) var(--radius) 0;
	}
	.signing-aside h4 {
		font-size: 0.78rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-secondary);
		margin-bottom: 10px;
	}
	.signing-aside ul {
		margin: 0;
		padding-left: 18px;
		color: var(--text-secondary);
		font-size: 0.88rem;
		line-height: 1.7;
	}

	/* ── How It Works ── */
	.how-it-works h2,
	.how-it-works .section-desc {
		text-align: center;
	}
	.section-desc {
		margin-bottom: 48px;
	}
	.steps {
		list-style: none;
		padding: 0;
		margin: 0 auto;
		max-width: 900px;
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 32px;
	}
	.step {
		padding-top: 18px;
		border-top: 1px solid var(--border);
	}
	.step-number {
		display: block;
		font-size: 0.72rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		color: var(--accent);
		font-variant-numeric: tabular-nums;
		margin-bottom: 10px;
	}
	.step h3 {
		font-size: 1.05rem;
		font-weight: 600;
		margin-bottom: 8px;
	}
	.step p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.75;
		margin: 0;
	}

	/* The spec line: five links, one row, no table. */
	.stack {
		max-width: 900px;
		margin: 48px auto 0;
		padding-top: 20px;
		border-top: 1px solid var(--border);
		text-align: center;
	}
	.stack-label {
		display: block;
		font-size: 0.72rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-tertiary);
		margin-bottom: 12px;
	}
	.stack-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 8px 0;
		font-size: 0.85rem;
		color: var(--text-secondary);
	}
	.stack-list li + li::before {
		content: '·';
		margin: 0 12px;
		color: var(--text-tertiary);
	}
	.stack-list a {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 4px;
	}
	.stack-list a:hover {
		color: var(--accent);
	}

	/* ── Compare ── */
	.compare h2,
	.compare .section-desc {
		text-align: center;
	}
	.compare-table-wrap {
		max-width: 960px;
		margin: 0 auto;
		overflow-x: auto;
	}
	.compare-note {
		max-width: 640px;
		margin: 18px auto 0;
		text-align: center;
		font-size: 0.8rem;
		line-height: 1.6;
		color: var(--text-tertiary);
	}
	.compare-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.82rem;
		min-width: 720px;
	}
	.compare-table th,
	.compare-table td {
		padding: 10px 12px;
		text-align: left;
		border-bottom: 1px solid var(--border);
	}
	.compare-table thead th {
		font-size: 0.78rem;
		font-weight: 600;
		color: var(--text-secondary);
		white-space: normal;
		vertical-align: bottom;
	}
	.compare-table thead th:first-child {
		width: 24%;
	}
	.compare-table tbody td:first-child {
		color: var(--text-secondary);
	}
	.compare-table td.yes {
		color: var(--text);
		font-weight: 600;
	}
	.compare-table td.warn {
		color: var(--text-tertiary);
	}
	.compare-table td.no {
		color: var(--text-tertiary);
	}
	/* ── Pricing ── */
	.business-model h2 {
		text-align: center;
	}
	.bm-content {
		max-width: 900px;
		margin: 0 auto;
	}
	.bm-intro {
		text-align: center;
		color: var(--text-secondary);
		font-size: 1rem;
		line-height: 1.7;
		margin: 0 auto 48px;
		max-width: 560px;
	}
	.bm-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 32px;
		margin-bottom: 36px;
	}
	.bm-card {
		padding-top: 18px;
		border-top: 1px solid var(--border);
	}
	.bm-card h4 {
		font-size: 0.78rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-secondary);
		margin-bottom: 10px;
	}
	.bm-price {
		font-size: 1.25rem;
		font-weight: 700;
		letter-spacing: -0.01em;
		margin-bottom: 12px;
	}
	.bm-card p {
		color: var(--text-secondary);
		font-size: 0.85rem;
		line-height: 1.7;
		margin: 0;
	}
	.bm-note {
		text-align: center;
		color: var(--text-tertiary);
		font-size: 0.82rem;
		line-height: 1.7;
		max-width: 620px;
		margin: 0 auto;
	}

	/* ── FAQ ── */
	.faq h2 {
		text-align: center;
	}
	.faq .section-desc {
		text-align: center;
	}
	.faq-list {
		max-width: 600px;
		margin: 0 auto;
	}
	details {
		border-bottom: 1px solid var(--border);
	}
	summary {
		padding: 16px 0;
		cursor: pointer;
		font-weight: 600;
		font-size: 0.92rem;
		list-style: none;
		display: flex;
		justify-content: space-between;
		align-items: center;
		color: var(--text);
	}
	summary::after {
		content: '+';
		font-size: 1.2rem;
		color: var(--text-tertiary);
	}
	details[open] summary::after {
		content: '\2212';
	}
	details p {
		padding-bottom: 16px;
		color: var(--text-secondary);
		line-height: 1.7;
		font-size: 0.88rem;
	}
	/* ── Responsive ── */
	@media (max-width: 768px) {
		.hero {
			padding-top: 80px;
			padding-bottom: 24px;
			min-height: 100vh;
		}
		.hero-grid {
			grid-template-columns: 1fr;
			gap: 40px;
		}
		.hero-text {
			text-align: center;
			display: block;
		}
		.hero-cta {
			align-items: center;
		}
		.hero-buttons {
			justify-content: center;
		}
		.hero-facts {
			max-width: none;
			height: auto;
			text-align: left;
		}
		.hero-facts .fact:first-child {
			padding-top: 18px;
			border-top: 1px solid var(--border);
		}
		.hero-facts dd {
			max-width: none;
		}
		/* Side by side at phone width only if neither label has to break: a
		   two-line button reads as a mistake. */
		.btn.btn-hero {
			min-width: 0;
			padding: 16px 22px;
			white-space: nowrap;
		}
		.scroll-hint {
			display: none;
		}
		h1 {
			font-size: 2.35rem;
		}
		h2 {
			font-size: 1.5rem;
		}
		.subtitle {
			font-size: 0.95rem;
			margin-left: auto;
			margin-right: auto;
		}
		.why-content h2 {
			font-size: 1.4rem;
		}
		.steps,
		.bm-grid {
			grid-template-columns: 1fr;
			gap: 24px;
		}
		.network-grid {
			grid-template-columns: repeat(2, 1fr);
		}
		.nav-links {
			display: none;
		}
	}
</style>
