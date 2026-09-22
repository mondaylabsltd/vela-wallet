<script lang="ts">
	import { resolve } from '$app/paths';
	import LanguageSwitcher from '$lib/components/LanguageSwitcher.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import Globe from '$lib/components/Globe.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { LOCALES, pathFor } from '$lib/i18n/locales';
	import { catalog, namespaceState, translatedLocales } from '$lib/i18n/resolve';
	import { seoConfig } from '$lib/seo';
	import { BUILT_IN_NETWORKS, chainLogo } from '$lib/networks';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// Every string on this page comes from the catalog. `home` is translated as a
	// WHOLE or not at all — see resolve.ts — so this is either fully the reader's
	// language or fully English under a notice, never a mixture.
	const m = $derived(catalog(data.locale));
	const homeState = $derived(namespaceState('home', data.locale));
	const alternates = $derived(translatedLocales('home'));
	/** Internal links written English-relative, served under the reader's prefix. */
	const L = $derived((path: string) => (path.startsWith('#') ? path : pathFor(data.locale, path)));

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
		{ href: '/docs/self-hosting#if-getvela-app-disappears', external: false }
	] as const;

	/**
	 * The colour of each comparison cell, ordered to match `m.home.compare.rows`.
	 * Deliberately NOT in the message catalog: "is this good or bad" is a claim
	 * we make, and a translator shouldn't be able to change it — or silently
	 * lose it — while translating the words. An empty string is a neutral cell.
	 *
	 * Vela is not green down the column, because it is not better down the
	 * column: it is the most expensive of the three per transaction, it sponsors
	 * almost nothing, and it is the youngest. MetaMask wins gas and track
	 * record; Base Account wins sponsorship. A table where our column is green
	 * eleven times is an advertisement, not a comparison.
	 */
	const COMPARE_TONES = [
		{ vela: '', metamask: '', base: '' }, // account type
		{ vela: 'yes', metamask: '', base: 'warn' }, // full self-hosting
		{ vela: 'yes', metamask: 'warn', base: 'warn' }, // source code
		{ vela: 'yes', metamask: 'yes', base: 'warn' }, // custom networks
		{ vela: 'yes', metamask: '', base: 'yes' }, // signing key
		{ vela: 'warn', metamask: 'warn', base: 'yes' }, // adding a key later
		{ vela: 'yes', metamask: '', base: 'yes' }, // losing one key
		{ vela: 'warn', metamask: 'yes', base: '' }, // transaction gas
		{ vela: 'warn', metamask: '', base: 'yes' }, // sponsored gas
		{ vela: 'yes', metamask: 'yes', base: 'yes' }, // batched transactions
		{ vela: 'yes', metamask: 'yes', base: 'yes' }, // decoded before signing
		// The signing page exists but no app sends it requests yet (spec 080):
		// an extra check you cannot use today is not a green cell.
		{ vela: 'warn', metamask: 'yes', base: '' }, // an extra check
		{ vela: 'warn', metamask: 'yes', base: 'yes' } // maturity
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

	// Built-in networks: one list, checked against vela-core by networks.test.ts.
	const NETWORKS = BUILT_IN_NETWORKS;

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
		<!-- Two links. "How it works", "Pricing", GitHub and "Sign in" came out on
		     2026-09-15: the first screen's job is one question, and a six-item bar
		     above it was answering five others. Every one of them is still on the
		     page — the sections are a scroll away, the code is the hero's second
		     button, and the wallet is behind "Getting started". -->
		<div class="nav-links">
			<a href="#why">{m.chrome.nav.whyVela}</a>
			<a href="#faq">{m.chrome.nav.faq}</a>
		</div>
		<LanguageSwitcher locale={data.locale} />
		<ThemeToggle />
	</div>
</nav>

<!-- Hero -->
<section class="hero">
	<!-- Left: the claim and the choice. Right: the globe and the one number on
	     this page nobody can fake. The four hooks used to sit on the right and
	     made the first screen a page to read rather than a thing to answer; they
	     now have a screen of their own, one scroll down. -->
	<div class="container hero-grid">
		<div class="hero-text">
			<h1>{m.home.hero.headline}</h1>
			<p class="subtitle">{m.home.hero.subtitle}</p>
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
			<!-- The path the buyer takes (founder, 2026-09-22): a quiet link, not a
			     third button, so the choice above stays a choice between two. -->
			<a
				href={L('/docs/self-hosting')}
				class="hero-selfhost"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="hero-selfhost">{m.home.hero.ctaSelfHost}</a
			>
		</div>

		<!-- The wallet count, read live from the registry contract and set over a
		     slowly turning globe: a running total rather than a stamp. The globe
		     is abstract on purpose — we do not know where these wallets were
		     created, and a dotted map would be inventing that. The number links
		     to the registry so the claim can be checked, and the whole block is
		     dropped when every RPC fails: a counter with no number is worse than
		     no counter.

		     The number is grouped by the PAGE's locale, not the browser's: a German
		     reader on /de gets 1.234 even if their browser is set to en-US, which
		     is the whole point of serving them /de (SC-009). -->
		<div class="hero-globe">
			<Globe />
			{#if !countFailed}
				<a
					class="counter"
					href={resolve('/registry')}
					title={m.home.seal.verify}
					data-rybbit-event="registry_open"
				>
					{#if countReady}
						<span class="counter-count">{displayCount.toLocaleString(data.locale)}</span>
					{:else}
						<span class="counter-skeleton" aria-label={m.home.seal.loading}></span>
					{/if}
					<span class="counter-label">
						<span class="counter-dot" aria-hidden="true"></span>
						{m.home.seal.label}
					</span>
				</a>
			{/if}
		</div>
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

<!-- The four facts a sceptic checks before trusting a wallet with money. They
     get the second screen to themselves, so the reader meets them one row at a
     time instead of all at once next to the headline. Every line is verifiable
     from the page below it or from the repo — nothing here is a claim the docs
     don't already make.

     Set as a numbered list of full-width rows rather than the 2x2 grid this
     started as: four claims of very different lengths in two columns left the
     rules ending in four different places and the block reading as debris. A
     row per claim — index, claim, proof — puts every rule and every link on the
     same line as the one above it, and the numbers tell the reader at a glance
     that there are exactly four things to check. The whole row is the link:
     the proof is the point, so the whole row should take you to it. -->
<section id="facts" class="facts">
	<div class="container">
		<p class="facts-lede">{m.home.facts.lede}</p>
		<ol class="fact-list">
			{#each m.home.hero.facts as fact, i (fact.term)}
				{@const proof = FACT_LINKS[i]}
				<li class="fact">
					<a
						class="fact-row"
						href={proof.external ? proof.href : L(proof.href)}
						target={proof.external ? '_blank' : undefined}
						rel={proof.external ? 'noopener' : undefined}
					>
						<!-- Decorative: a screen reader gets the four claims as a numbered
						     list from <ol> already, and would otherwise hear each one twice. -->
						<span class="fact-index" aria-hidden="true">{String(i + 1).padStart(2, '0')}</span>
						<span class="fact-term">{fact.term}</span>
						<span class="fact-proof">{fact.link}</span>
					</a>
				</li>
			{/each}
		</ol>
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
			<!-- Numbered, because the close refers to "one of those three" and a
			     reader who has to count them is already being asked for too much.
			     The digit is markup, not catalog text: a translator should never be
			     able to renumber the list by translating it. -->
			<ol class="tradeoff-list">
				{#each m.home.tradeoffs.items as item, i (item.title)}
					<li class="tradeoff">
						<h3><span class="tradeoff-index">{i + 1}</span>{item.title}</h3>
						<!-- Paragraphs are blank-line separated in the catalog rather than
						     <p> tags of their own: a translator writes prose, not markup,
						     and the markup-parity test has nothing extra to keep in step. -->
						{#each item.body.split('\n\n') as para (para)}
							<!-- eslint-disable-next-line svelte/no-at-html-tags -->
							<p>{@html para}</p>
						{/each}
					</li>
				{/each}
			</ol>
			<p class="tradeoffs-close">{m.home.tradeoffs.close}</p>
		</div>
	</div>
</section>

<!-- Compare -->
<section id="compare" class="compare">
	<div class="container">
		<h2>{m.home.compare.heading}</h2>
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
					</div>
				{/each}
			</div>
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
		<div class="faq-list">
			{#each m.home.faq.items as item (item.q)}
				<details>
					<summary>{item.q}</summary>
					<!-- Two beats per answer, blank-line separated in the catalog like
					     the trade-offs above: the claim, then the consequence. Split
					     here rather than carrying <p> tags a translator would have to
					     keep in step. -->
					{#each item.a.split('\n\n') as para (para)}
						<!-- eslint-disable-next-line svelte/no-at-html-tags -->
						<p>{@html para}</p>
					{/each}
				</details>
			{/each}
		</div>
	</div>
</section>

<!-- Footer -->
<SiteFooter locale={data.locale} />

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
	/* Text left, globe right. The fact list that used to hold the right-hand
	   column is a section of its own now, so this screen carries one sentence,
	   one choice and one number — and a lot of air. */
	.hero-grid {
		display: grid;
		grid-template-columns: 1fr 0.85fr;
		gap: 72px;
		/* Both columns run the full height of the spread so the two pairs of edges
		   line up: the headline starts where the globe starts, and the buttons end
		   where the counter ends. Nothing floats. */
		align-items: stretch;
	}
	.hero-text {
		text-align: left;
		/* A readable measure even on a wide window; the column can be wider than
		   the words. */
		max-width: 620px;
		display: flex;
		flex-direction: column;
	}
	.hero-globe {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-self: center;
	}
	/* The two bottom edges. Whichever column is taller sets the height; the
	   other's slack is taken up above these, never below them. */
	.hero-buttons,
	.counter {
		margin-top: auto;
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
	/* Two short sentences, so it can carry the weight of a standfirst rather
	   than reading as body text under a headline. Sized for the longest locale
	   (vi, 84 characters) at three lines on a phone. */
	.subtitle {
		color: var(--text-secondary);
		font-size: 1.2rem;
		line-height: 1.6;
		max-width: 460px;
		margin: 0 0 40px;
	}
	.hero-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
	}
	.hero-selfhost {
		display: inline-block;
		margin-top: 20px;
		font-size: 0.95rem;
		color: var(--text-secondary);
		text-decoration: underline;
		text-decoration-color: var(--border);
		text-underline-offset: 4px;
	}
	.hero-selfhost::after {
		content: ' →';
	}
	.hero-selfhost:hover {
		color: var(--text);
		text-decoration-color: currentColor;
	}
	.btn.btn-hero {
		padding: 20px 44px;
		min-width: 230px;
		text-align: center;
		font-size: 1.02rem;
		border-radius: 12px;
	}

	/* ── The four claims ── */
	/* Their own screen, vertically centred, so the reader arrives at four lines
	   and nothing else. */
	.facts {
		min-height: 70vh;
		display: flex;
		align-items: center;
	}
	.facts .container {
		width: 100%;
	}
	/* One quiet line to say WHEN the four rows are worth reading — before the
	   deposit. Without it the block reads as text that fell out of the hero;
	   with it, it is a section. Small and muted on purpose, and a fragment
	   rather than a sentence — the claims are the loud part. */
	.facts-lede {
		max-width: 1000px;
		margin: 0 auto 22px;
		color: var(--text-tertiary);
		font-size: 0.85rem;
		letter-spacing: 0.01em;
	}
	/* A numbered list of full-width rows, the way a well-set contents page does
	   it. This was a 2x2 grid and the four claims are of very different lengths,
	   so the rules ended in four different places and the whole block read as
	   debris. One row per claim puts every rule, every claim and every proof on
	   the same two vertical lines, and the numbers say at a glance that there
	   are exactly four things to check. */
	.fact-list {
		/* Capped and centred: the page is 1400px wide and these are short lines —
		   left to fill it, each claim would sit alone at the end of a rule twice
		   the length of its own sentence. */
		max-width: 1000px;
		margin: 0 auto;
		padding: 0;
		list-style: none;
		border-top: 1px solid var(--border);
	}
	.fact {
		border-bottom: 1px solid var(--border);
	}
	/* The whole row is the link. The proof is the point of the row, so the row
	   should take you to it — a 12px link at the end was the only target. */
	.fact-row {
		display: grid;
		grid-template-columns: 34px minmax(0, 1fr) minmax(0, 1fr);
		align-items: baseline;
		column-gap: 28px;
		padding: 26px 16px;
		/* Bled out past the text so the hover tint reads as a row, not a box
		   drawn around the words. */
		margin: 0 -16px;
		border-radius: 10px;
		color: inherit;
		text-decoration: none;
		transition: background-color 0.18s;
	}
	.fact-row:hover {
		background: light-dark(rgba(35, 33, 28, 0.03), rgba(237, 234, 226, 0.04));
	}
	.fact-row:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	/* Tabular so 01–04 sit in one column, and mono so they read as an index
	   rather than as part of the sentence next to them. */
	.fact-index {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		font-variant-numeric: tabular-nums;
		letter-spacing: 0.06em;
		color: var(--text-tertiary);
		transition: color 0.18s;
	}
	.fact-row:hover .fact-index {
		color: var(--accent);
	}
	.fact-term {
		font-weight: 600;
		font-size: 1.16rem;
		line-height: 1.45;
		letter-spacing: -0.01em;
		color: var(--text);
		text-wrap: balance;
	}
	/* The proof, not the explanation: every claim is one link away from the page
	   that can check it. */
	.fact-proof {
		font-size: 0.86rem;
		line-height: 1.55;
		color: var(--text-tertiary);
		transition: color 0.18s;
	}
	.fact-row:hover .fact-proof {
		color: var(--text-secondary);
	}
	/* The arrow is a pseudo-element glued to the last word with a non-breaking
	   space: as a separate span it could — and on the English row 02 did — wrap
	   onto a line of its own, which reads as a bullet with nothing after it. */
	.fact-proof::after {
		content: '\00a0→';
		transition: margin-left 0.18s;
	}
	.fact-row:hover .fact-proof::after {
		margin-left: 3px;
	}
	/* Three columns need about 900px to hold a sentence each; below that the
	   proof goes under its claim and keeps the number's column. This is the one
	   rule on the page with its own breakpoint, because 768px is already too
	   narrow for the three-column row. */
	@media (max-width: 900px) {
		.fact-row {
			grid-template-columns: 34px minmax(0, 1fr);
			column-gap: 14px;
			row-gap: 6px;
		}
		.fact-index {
			grid-column: 1;
			grid-row: 1;
		}
		.fact-term,
		.fact-proof {
			grid-column: 2;
		}
	}
	/* CJK sets tighter than Latin at the same size and the claims are the only
	   place on this screen where that shows; a hair smaller keeps a four-line
	   claim off a third line. */
	:global(html[lang^='zh']) .fact-term,
	:global(html[lang='ja']) .fact-term,
	:global(html[lang='ko']) .fact-term {
		font-size: 1.1rem;
		letter-spacing: 0;
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
	.network-note :global(a),
	.compare-table td :global(a),
	details :global(a) {
		color: inherit;
		text-decoration: underline;
		text-underline-offset: 4px;
		transition: color 0.15s;
	}
	.why-content :global(a:not(.more-link):hover),
	.tradeoff :global(a:hover),
	.network-note :global(a:hover),
	.compare-table td :global(a:hover),
	details :global(a:hover) {
		color: var(--accent);
	}
	/* External links get a trailing lucide external-link glyph, masked in
	   currentColor so it follows the link color. */
	.why-content :global(a[target='_blank']::after),
	.tradeoff :global(a[target='_blank']::after),
	.network-note :global(a[target='_blank']::after),
	.compare-table td :global(a[target='_blank']::after),
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

	/* ── The on-chain counter ──
	   The number sits under the globe as a running total: the brand serif for the
	   figure, a live dot and a quiet label under it. It is a link, because the
	   claim it makes is one the reader can go and check. */
	.counter {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 6px 14px;
		border-radius: 12px;
		text-decoration: none;
		color: var(--text-secondary);
		transition:
			color 0.2s,
			background 0.2s;
	}
	.counter:hover {
		background: var(--accent-soft);
	}
	.counter-count {
		font-family: var(--font-serif);
		font-size: 3rem;
		font-weight: 700;
		line-height: 1;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
		color: var(--accent);
	}
	.counter-label {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.09em;
		text-transform: uppercase;
		color: var(--text-muted);
		/* One line: "ON-CHAIN" breaking at its own hyphen was the first thing the
		   eye caught. */
		white-space: nowrap;
	}
	/* CJK has no case, so the uppercase transform does nothing and the wide
	   tracking just pulls the characters apart. */
	:global(html[lang^='zh']) .counter-label,
	:global(html[lang='ja']) .counter-label,
	:global(html[lang='ko']) .counter-label {
		text-transform: none;
		letter-spacing: 0.02em;
		font-size: 0.76rem;
	}
	.counter-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: var(--green);
		box-shadow: 0 0 6px color-mix(in srgb, var(--green) 50%, transparent);
		animation: pulse-dot 2s ease-in-out infinite;
	}
	.counter-skeleton {
		display: block;
		width: 96px;
		height: 3rem;
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
		/* Room for the hanging index; the titles stay flush with the lede. */
		padding: 0 0 0 22px;
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
	/* Hanging in the left margin so the titles keep one edge and the numbers
	   read as an index rather than as the first word of the sentence. */
	.tradeoff-index {
		display: inline-block;
		width: 22px;
		margin-left: -22px;
		font-family: var(--font-mono);
		font-size: 0.8rem;
		font-variant-numeric: tabular-nums;
		color: var(--text-tertiary);
	}
	.tradeoff p {
		color: var(--text-secondary);
		font-size: 0.92rem;
		line-height: 1.75;
		margin: 0;
	}
	.tradeoff p + p {
		margin-top: 13px;
	}
	.tradeoffs-close {
		margin: 26px 0 0;
		padding-top: 22px;
		border-top: 1px solid var(--border);
		color: var(--text);
		font-size: 0.95rem;
		line-height: 1.7;
	}

	/* ── Compare ── */
	/* No standfirst under this heading any more, so the heading carries the gap
	   the paragraph used to leave above the table. */
	.compare h2 {
		text-align: center;
		margin-bottom: 48px;
	}
	.compare-table-wrap {
		max-width: 960px;
		margin: 0 auto;
		overflow-x: auto;
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
	}

	/* ── FAQ ── */
	/* No standfirst under this heading any more, so the heading carries the gap
	   the paragraph used to leave above the questions. */
	.faq h2 {
		text-align: center;
		margin-bottom: 48px;
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
		margin: 0;
		color: var(--text-secondary);
		line-height: 1.7;
		font-size: 0.88rem;
	}
	/* The second beat is a new paragraph, not a new answer: closer together
	   than two answers are, and only the last one pays the closing padding. */
	details p + p {
		margin-top: 10px;
	}
	details p:last-of-type {
		padding-bottom: 16px;
	}
	/* ── Responsive ── */
	@media (max-width: 768px) {
		.hero {
			padding-top: 80px;
			padding-bottom: 24px;
			min-height: 100vh;
		}
		/* The globe goes under the words on a phone, and everything centres. */
		.hero-grid {
			grid-template-columns: 1fr;
			gap: 28px;
		}
		.hero-text {
			text-align: center;
			max-width: none;
		}
		.hero-buttons {
			justify-content: center;
		}
		.subtitle {
			margin-bottom: 32px;
		}
		/* A phone screen has to hold the headline, the choice, the globe AND the
		   number: the globe gives up the room. */
		.hero-globe {
			--globe-size: 220px;
		}
		.counter-count {
			font-size: 2.5rem;
		}
		/* Stacked on a phone, so there is no second column to line up with: the
		   counter sits straight under the globe. */
		.hero-buttons,
		.counter {
			margin-top: 0;
		}
		.facts {
			/* A phone screen fits about two of these, so the section stops
			   pretending to be exactly one screen and is simply a section. */
			min-height: 0;
			padding: 72px 0;
		}
		.fact-row {
			padding: 20px 12px;
			margin: 0 -12px;
		}
		.fact-term {
			font-size: 1.05rem;
		}
		.fact-proof {
			font-size: 0.8rem;
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
