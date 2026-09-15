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

	/**
	 * The colour of each comparison cell. Deliberately NOT in the message
	 * catalog: "is this good or bad" is a claim we make, and a translator
	 * shouldn't be able to change it — or silently lose it — while translating
	 * the words. Ordered to match `m.home.compare.rows`.
	 */
	const COMPARE_TONES = [
		{ vela: 'yes', metamask: 'warn', base: 'yes' },
		{ vela: 'yes', metamask: 'warn', base: 'yes' },
		{ vela: 'yes', metamask: 'warn', base: 'warn' },
		{ vela: 'yes', metamask: 'warn', base: 'warn' },
		{ vela: '', metamask: 'yes', base: 'warn' },
		{ vela: 'yes', metamask: 'yes', base: 'warn' },
		{ vela: 'yes', metamask: 'no', base: 'no' }
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
			{#each m.home.hero.facts as fact (fact.term)}
				<div class="fact">
					<dt>{fact.term}</dt>
					<dd>{fact.detail}</dd>
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

<!-- Does Less -->
<section id="minimal" class="does-less">
	<div class="container">
		<div class="does-less-content">
			<h2>{m.home.doesLess.heading}</h2>
			<p>{m.home.doesLess.p1}</p>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p>{@html m.home.doesLess.p2}</p>
			<p>{m.home.doesLess.p3}</p>
			<p>{m.home.doesLess.p4}</p>
		</div>
	</div>
</section>

<!-- Why -->
<section id="why" class="why">
	<div class="container">
		<div class="why-content">
			<h2>{m.home.why.heading}</h2>
			<p>{m.home.why.p1}</p>
			<p class="why-beat">{m.home.why.beat}</p>
			<p>{m.home.why.p2}</p>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p>{@html m.home.why.p3}</p>
			<p>{m.home.why.p4}</p>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p>{@html m.home.why.p5}</p>
			<!-- eslint-disable-next-line svelte/no-at-html-tags -->
			<p>{@html m.home.why.p6}</p>
			<p>{m.home.why.p7}</p>
			<p>{m.home.why.p8}</p>
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
						<th>MetaMask</th>
						<th>Base Account</th>
					</tr>
				</thead>
				<tbody>
					{#each m.home.compare.rows as row, i (row.feature)}
						<tr>
							<td>{row.feature}</td>
							<!-- eslint-disable svelte/no-at-html-tags -->
							<td class={COMPARE_TONES[i]?.vela}>{@html row.vela}</td>
							<td class={COMPARE_TONES[i]?.metamask}>{@html row.metamask}</td>
							<td class={COMPARE_TONES[i]?.base}>{@html row.base}</td>
							<!-- eslint-enable svelte/no-at-html-tags -->
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="compare-note">{m.home.compare.note}</p>
	</div>
</section>

<!-- How It Works -->
<section id="how-it-works" class="how-it-works">
	<div class="container">
		<h2>{m.home.how.heading}</h2>
		<p class="section-desc">{m.home.how.desc}</p>

		{#each m.home.how.steps as step, i (step.title)}
			<div class="pillar">
				<div class="pillar-number">{String(i + 1).padStart(2, '0')}</div>
				<div class="pillar-content">
					<h3>{step.title}</h3>
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					<p>{@html step.body}</p>
				</div>
			</div>
		{/each}

		<div class="tech-details">
			<h3>{m.home.how.tech.heading}</h3>
			<table>
				<tbody>
					<tr>
						<td>{m.home.how.tech.wallet}</td>
						<td
							><a
								href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
								target="_blank"
								rel="noopener">Safe v1.4.1</a
							></td
						>
					</tr>
					<tr>
						<td>{m.home.how.tech.authentication}</td>
						<td
							><a href="https://www.w3.org/TR/webauthn-2/" target="_blank" rel="noopener"
								>WebAuthn</a
							> / P-256</td
						>
					</tr>
					<tr>
						<td>{m.home.how.tech.accountType}</td>
						<td
							><a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener"
								>ERC-4337</a
							> {m.home.how.tech.accountTypeValue}</td
						>
					</tr>
					<tr>
						<td>{m.home.how.tech.signerModule}</td>
						<td
							><a
								href="https://github.com/safe-global/safe-modules/tree/main/modules/passkey/contracts/4337"
								target="_blank"
								rel="noopener">SafeWebAuthnSharedSigner</a
							></td
						>
					</tr>
					<tr>
						<td>{m.home.how.tech.networks}</td>
						<td>{m.home.how.tech.networksValue}</td>
					</tr>
					<tr>
						<td>{m.home.how.tech.sourceCode}</td>
						<td
							><a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
								>GitHub</a
							></td
						>
					</tr>
				</tbody>
			</table>

			<div class="tech-networks">
				<div class="network-row">
					{#each NETWORKS as net (net.chainId)}
						<span class="network-chip">
							<img
								class="network-logo"
								src={chainLogo(net.chainId)}
								alt=""
								width="22"
								height="22"
								loading="lazy"
							/>
							{net.name}
						</span>
					{/each}
				</div>
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				<p class="network-note">{@html m.home.how.networkNote}</p>
			</div>
		</div>
	</div>
</section>

<!-- Business Model -->
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

			<p class="bm-note">{m.home.pricing.note}</p>
		</div>
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

<!-- CTA -->
<section id="notify" class="notify">
	<div class="container">
		<h2>{m.home.cta.heading}</h2>
		<p class="notify-sub">{m.home.cta.sub}</p>
		<a
			href="https://wallet.getvela.app/"
			target="_blank"
			rel="noopener"
			class="btn btn-primary btn-cta-main"
			data-rybbit-event="cta_click"
			data-rybbit-prop-location="bottom">{m.home.cta.button}</a
		>

		<ul class="notify-cards">
			{#each m.home.cta.cards as card, i (card.title)}
				<li class="notify-card">
					<svg
						width="22"
						height="22"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						stroke-width="2"
					>
						{#if i === 0}
							<path d="M8 6l-5 6 5 6M16 6l5 6-5 6" stroke-linecap="round" stroke-linejoin="round" />
						{:else if i === 1}
							<rect x="3" y="4" width="18" height="7" rx="1.5" />
							<rect x="3" y="13" width="18" height="7" rx="1.5" />
							<path d="M7 7.5h.01M7 16.5h.01" stroke-linecap="round" />
						{:else if i === 2}
							<path
								d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						{:else}
							<path
								d="M7 3H5a2 2 0 00-2 2v2M17 3h2a2 2 0 012 2v2M7 21H5a2 2 0 01-2-2v-2M17 21h2a2 2 0 002-2v-2M9 10v1M15 10v1M9.5 15a3.5 3.5 0 005 0"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						{/if}
					</svg>
					<h4>{card.title}</h4>
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					<p>{@html card.body}</p>
				</li>
			{/each}
		</ul>

		<div class="notify-divider"><span>{m.home.cta.divider}</span></div>

		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<p class="notify-email-desc">{@html m.home.cta.followDesc}</p>

		<div class="notify-social">
			<a
				href="https://x.com/realvelawallet"
				target="_blank"
				rel="noopener"
				class="btn btn-outline btn-social"
				data-rybbit-event="social_click"
				data-rybbit-prop-network="x">{m.home.cta.followX}</a
			>
			<a
				href="https://t.me/velawallet"
				target="_blank"
				rel="noopener"
				class="btn btn-outline btn-social"
				data-rybbit-event="social_click"
				data-rybbit-prop-network="telegram">{m.home.cta.joinTelegram}</a
			>
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
		grid-template-columns: 1.1fr 0.9fr;
		gap: 72px;
		align-items: center;
	}
	.hero-text {
		text-align: left;
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
	.subtitle {
		color: var(--text-secondary);
		font-size: 1.05rem;
		line-height: 1.75;
		max-width: 520px;
		margin-bottom: 32px;
	}
	.hero-cta {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 16px;
	}
	.hero-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
	}
	.btn-hero {
		padding: 14px 28px;
		font-size: 0.95rem;
	}

	/* The fact column. One hairline carries the whole list — no cards, no
	   boxes — so the eye reads five statements, not five containers. */
	.hero-facts {
		margin: 0;
		padding: 4px 0 4px 32px;
		border-left: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		gap: 26px;
	}
	.hero-facts dt {
		font-weight: 600;
		font-size: 0.95rem;
		color: var(--text);
		margin-bottom: 5px;
	}
	.hero-facts dd {
		margin: 0;
		font-size: 0.95rem;
		line-height: 1.6;
		color: var(--text-secondary);
		max-width: 38ch;
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

	/* ── Live Stat (trust strip) ── */
	.live-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--green);
		box-shadow: 0 0 6px color-mix(in srgb, var(--green) 50%, transparent);
		animation: pulse-dot 2s ease-in-out infinite;
	}
	.stat-number {
		font-size: 0.92rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
		color: var(--accent);
	}
	/* Placeholder shown while the on-chain count is still loading. */
	.stat-skeleton {
		display: inline-block;
		width: 26px;
		height: 0.72rem;
		border-radius: 4px;
		background: linear-gradient(90deg, var(--border), var(--border-strong), var(--border));
		background-size: 200% 100%;
		animation: stat-shimmer 1.3s ease-in-out infinite;
	}
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
	.btn-cta-main {
		padding: 14px 36px;
		font-size: 1rem;
		margin-bottom: 40px;
	}
	.notify-cards {
		list-style: none;
		padding: 0;
		margin: 16px auto 56px;
		max-width: 760px;
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 14px;
		text-align: center;
	}
	.notify-card {
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 36px 18px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		box-shadow: var(--shadow-sm);
	}
	.notify-card svg {
		color: var(--accent);
	}
	.notify-card h4 {
		font-size: 0.92rem;
		font-weight: 600;
		color: var(--text);
	}
	.notify-card p {
		font-size: 0.78rem;
		color: var(--text-secondary);
		line-height: 1.55;
		margin: 0;
	}

	/* ── Text links ── */
	/* Inline links keep the surrounding ink color — no accent highlight. The
	   underline offset clears descenders (g, y) so the line stays unbroken. */
	.why-content :global(a),
	.does-less-content :global(a),
	.pillar-content :global(a),
	.tech-details a,
	.compare-table td :global(a),
	.bm-card :global(a),
	details :global(a) {
		color: inherit;
		text-decoration: underline;
		text-underline-offset: 4px;
		transition: color 0.15s;
	}
	.why-content :global(a:hover),
	.does-less-content :global(a:hover),
	.pillar-content :global(a:hover),
	.tech-details a:hover,
	.compare-table td :global(a:hover),
	.bm-card :global(a:hover),
	details :global(a:hover) {
		color: var(--accent);
	}
	/* External links get a trailing lucide external-link glyph, masked in
	   currentColor so it follows the link color. */
	.why-content :global(a[target='_blank']::after),
	.does-less-content :global(a[target='_blank']::after),
	.pillar-content :global(a[target='_blank']::after),
	.tech-details a[target='_blank']::after,
	.compare-table td :global(a[target='_blank']::after),
	.bm-card :global(a[target='_blank']::after),
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
		margin-top: 52px;
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

	/* ── Networks (inside Technical details) ── */
	.tech-networks {
		margin-top: 20px;
		padding-top: 20px;
		border-top: 1px solid var(--border);
	}
	.network-row {
		display: flex;
		flex-wrap: wrap;
		gap: 12px 18px;
	}
	.network-chip {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-size: 0.82rem;
		font-weight: 500;
		color: var(--text);
		white-space: nowrap;
	}
	.network-logo {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		flex-shrink: 0;
		background: var(--border);
	}
	.network-note {
		margin-top: 16px;
		color: var(--text-tertiary);
		font-size: 0.8rem;
		line-height: 1.65;
	}

	/* ── Does Less ── */
	.does-less {
		margin-top: 12px;
		padding: 96px 0;
		border-top: 1px solid var(--border);
	}
	.does-less-content {
		max-width: 640px;
		margin: 0 auto;
	}
	.does-less-content h2 {
		margin-bottom: 20px;
	}
	.does-less-content p {
		color: var(--text-secondary);
		font-size: 1.05rem;
		line-height: 1.8;
		margin-bottom: 16px;
	}
	.does-less-content p:last-child {
		margin-bottom: 0;
	}
	/* ── Why ── */
	.why {
		padding: 120px 0 60px;
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
	.why-content :global(strong) {
		color: var(--text);
	}
	.why-content .why-beat {
		color: var(--text);
		font-size: 1.05rem;
		font-weight: 400;
		line-height: 1.5;
		margin: 20px 0;
	}

	/* ── How It Works (Pillars) ── */
	.how-it-works h2,
	.how-it-works .section-desc {
		text-align: center;
	}
	.pillar {
		display: flex;
		gap: 32px;
		align-items: flex-start;
		max-width: 640px;
		margin: 0 auto 56px;
		padding: 28px 28px 28px 32px;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-left: 3px solid var(--accent);
		border-radius: 0 var(--radius) var(--radius) 0;
		box-shadow: var(--shadow-sm);
	}
	.pillar:last-child {
		margin-bottom: 0;
	}
	.pillar-number {
		font-size: 0.78rem;
		font-weight: 700;
		color: var(--accent);
		background: var(--accent-soft);
		border: 1px solid var(--border-accent);
		padding: 6px 12px;
		border-radius: 8px;
		flex-shrink: 0;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.02em;
	}
	.pillar-content h3 {
		font-size: 1.1rem;
		font-weight: 600;
		margin-bottom: 10px;
		line-height: 1.3;
	}
	.pillar-content p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.75;
		margin-bottom: 12px;
	}
	.pillar-content p:last-child,
	.pillar-content p:last-of-type {
		margin-bottom: 0;
	}
	/* ── Tech Details ── */
	.tech-details {
		max-width: 640px;
		margin: 56px auto 0;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 28px 28px;
		box-shadow: var(--shadow-sm);
	}
	.tech-details h3 {
		font-size: 0.82rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-secondary);
		margin-bottom: 16px;
	}
	.tech-details table {
		width: 100%;
		border-collapse: collapse;
	}
	.tech-details td {
		padding: 8px 0;
		font-size: 0.88rem;
		border-bottom: 1px solid var(--border);
	}
	.tech-details tr:last-child td {
		border-bottom: none;
	}
	.tech-details td:first-child {
		color: var(--text-secondary);
		width: 40%;
	}
	.tech-details td:last-child {
		color: var(--text);
		font-weight: 500;
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
		min-width: 640px;
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
		width: 28%;
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
	/* ── Business Model ── */
	.business-model h2 {
		text-align: center;
	}
	.bm-content {
		max-width: 720px;
		margin: 0 auto;
	}
	.bm-intro {
		text-align: center;
		color: var(--text-secondary);
		font-size: 1rem;
		line-height: 1.7;
		margin-bottom: 40px;
		max-width: 560px;
		margin-left: auto;
		margin-right: auto;
	}
	.bm-grid {
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 560px;
		margin: 0 auto 32px;
	}
	.bm-card {
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 24px 20px;
		box-shadow: var(--shadow-sm);
	}
	.bm-card h4 {
		font-size: 0.88rem;
		font-weight: 600;
		margin-bottom: 8px;
	}
	.bm-price {
		font-size: 1.1rem;
		font-weight: 700;
		color: var(--accent);
		margin-bottom: 12px;
	}
	.bm-card p {
		color: var(--text-secondary);
		font-size: 0.82rem;
		line-height: 1.6;
		margin-bottom: 8px;
	}
	.bm-card p:last-child {
		margin-bottom: 0;
	}
	.bm-note {
		text-align: center;
		color: var(--text-tertiary);
		font-size: 0.82rem;
		line-height: 1.6;
		max-width: 560px;
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
	/* ── Notify ── */
	.notify {
		text-align: center;
		padding: 112px 0 128px;
	}
	.notify h2 {
		margin-bottom: 28px;
	}
	.notify-social {
		display: flex;
		gap: 12px;
		justify-content: center;
		flex-wrap: wrap;
	}
	.btn-social {
		padding: 12px 28px;
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
	.notify-divider {
		display: flex;
		align-items: center;
		gap: 16px;
		margin: 48px auto;
		max-width: 400px;
	}
	.notify-divider::before,
	.notify-divider::after {
		content: '';
		flex: 1;
		height: 1px;
		background: var(--border);
	}
	.notify-divider span {
		color: var(--text-tertiary);
		font-size: 0.78rem;
	}
	/* Notify social links */
	.notify-sub {
		color: var(--text-secondary);
		font-size: 0.95rem;
		margin-bottom: 28px;
	}
	.notify-email-desc {
		color: var(--text-tertiary);
		font-size: 0.82rem;
		margin-bottom: 16px;
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
		}
		.hero-cta {
			align-items: center;
		}
		.hero-buttons {
			justify-content: center;
		}
		/* Stacked, the vertical hairline has nothing to divide, so each fact
		   gets its own rule above it instead. */
		.hero-facts {
			padding: 0;
			border-left: none;
			gap: 0;
			text-align: left;
		}
		.hero-facts .fact {
			padding: 18px 0;
			border-top: 1px solid var(--border);
		}
		.hero-facts dd {
			max-width: none;
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
		.trust-row {
			justify-content: center;
		}
		.trust-chip {
			font-size: 0.72rem;
		}
		.network-chip {
			font-size: 0.8rem;
		}
		.why-content {
			text-align: left;
		}
		.why-content h2 {
			font-size: 1.4rem;
		}
		.pillar {
			flex-direction: column;
			gap: 12px;
		}
		.pillar-number {
			align-self: flex-start;
		}
		.nav-links {
			display: none;
		}
		.notify-cards {
			grid-template-columns: repeat(2, 1fr);
		}
		.notify-social {
			flex-direction: column;
			padding: 0 16px;
		}
		.notify-social .btn-social {
			width: 100%;
		}
	}
</style>
