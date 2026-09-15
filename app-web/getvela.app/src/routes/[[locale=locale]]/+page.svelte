<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { seoConfig } from '$lib/seo';

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
	const structuredData = [
		{
			'@context': 'https://schema.org',
			'@type': 'Organization',
			'@id': `${seoConfig.domain}/#organization`,
			name: seoConfig.siteName,
			legalName: 'MONDAY LABS LTD',
			url: seoConfig.domain,
			logo: `${seoConfig.domain}/vela-logo.png`,
			description:
				'An open-source, self-hostable Ethereum wallet for ETH and ERC-20 tokens. Sign with a passkey — no seed phrase, no hardware key, no lock-in.',
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
			publisher: { '@id': `${seoConfig.domain}/#organization` }
		}
	];
	// Serialize the structured data into a JSON-LD script block for the document
	// head. The closing tag is split across two string literals ("</scr" + "ipt>")
	// so the complete closing-script token never appears literally anywhere in this
	// module's source — if it did, the Svelte parser would read it as the end of the
	// component's own script block and orphan everything after it. Every less-than
	// char in the JSON payload is escaped so the data can never break out of the tag.
	const structuredDataHtml =
		`<script type="application/ld+json">${JSON.stringify(structuredData).replace(/</g, '\\u003c')}</scr` +
		`ipt>`;
</script>

<svelte:head>
	<title>Vela Wallet — An Ethereum wallet you actually own</title>
	<meta
		name="description"
		content="An open-source, self-hostable Ethereum wallet for ETH & ERC-20s. Sign with a passkey — no seed phrase, no hardware key, no lock-in. You pay for convenience, not access."
	/>
	<meta property="og:title" content="Vela Wallet — An Ethereum wallet you actually own" />
	<meta
		property="og:description"
		content="Open-source, self-hostable wallet for ETH & ERC-20s. Passkey signing, no seed phrase, no lock-in. Compile it yourself if you want to."
	/>
	<meta property="og:image" content="https://getvela.app/getvela-app-preview.png" />
	<meta property="og:url" content="https://getvela.app" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:image" content="https://getvela.app/getvela-app-preview.png" />
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html structuredDataHtml}
</svelte:head>

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
				Alpha
			</a>
		</div>
		<div class="nav-links">
			<a href="#why">Why Vela</a>
			<a href="#how-it-works">How it works</a>
			<a href="#pricing">Pricing</a>
			<a href="#faq">FAQ</a>

			<a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
				>GitHub</a
			>
			<a
				href="https://wallet.getvela.app/"
				target="_blank"
				rel="noopener"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="nav-signin">Sign in</a
			>
		</div>
		<ThemeToggle />
	</div>
</nav>

<!-- Hero -->
<section class="hero">
	<div class="container hero-grid">
		<div class="hero-text">
			<h1>An Ethereum wallet you actually own</h1>
			<p class="subtitle">
				Sign with a passkey Vela never sees. Open source and self-hostable on most EVM chains — so
				it keeps working even if we disappear.
			</p>
			<div class="hero-cta">
				<div class="hero-buttons">
					<a
						href="https://wallet.getvela.app/"
						target="_blank"
						rel="noopener"
						class="btn btn-primary btn-hero"
						data-rybbit-event="cta_click"
						data-rybbit-prop-location="hero">Create a wallet — no seed phrase</a
					>
					<a
						href="https://github.com/mondaylabsltd/vela-wallet"
						target="_blank"
						rel="noopener"
						class="btn btn-outline btn-hero"
						data-rybbit-event="cta_click"
						data-rybbit-prop-location="hero-code">Read the code</a
					>
				</div>
				<a
					href="https://wallet.getvela.app/"
					target="_blank"
					rel="noopener"
					class="hero-signin"
					data-rybbit-event="cta_click"
					data-rybbit-prop-location="hero-signin">Already have a wallet? Sign in</a
				>
			</div>
		</div>
		<!-- The three facts a sceptic checks before trusting a wallet with money.
		     Every line is verifiable from the page below it or from the repo —
		     nothing here is a claim the docs don't already make. ERC-4337 and
		     "100% open source" used to be rows of their own; both are still stated
		     in the trust strip and the technical-details table below. -->
		<dl class="hero-facts">
			<div class="fact">
				<dt>Safe v1.4.1, unmodified</dt>
				<dd>Third-party audited contracts, deployed exactly as published.</dd>
			</div>
			<div class="fact">
				<dt>12 chains built in, plus your own</dt>
				<dd>
					Any EVM chain with the RIP-7212 precompile and Safe v1.4.1's contracts deployed. One
					address on every chain.
				</dd>
			</div>
			<div class="fact">
				<dt>1-of-n signers, security keys included</dt>
				<dd>
					Passkeys, a nearby device, or a USB/NFC key — up to seven, any one of which signs.
					Chosen when you create the wallet.
				</dd>
			</div>
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

<!-- Trust Strip -->
<section class="trust-strip">
	<div class="container">
		<p class="trust-tagline">
			Don't trust us — verify. Every line is on <a
				href="https://github.com/mondaylabsltd/vela-wallet"
				target="_blank"
				rel="noopener">GitHub</a
			>, every wallet is on-chain, and
			<a
				href={resolve('/docs/security-audits')}
				data-rybbit-event="audits_open"
				data-rybbit-prop-location="trust-tagline">every audit and known issue is documented</a
			>.
		</p>
		<div class="trust-row">
			<div class="trust-chip">
				<svg
					width="14"
					height="14"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M17.25 6.75L22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3l-4.5 16.5"
						stroke-linecap="round"
						stroke-linejoin="round"
					/></svg
				>
				<a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener"
					>100% open source</a
				> — app + all our services
			</div>
			<div class="trust-chip">
				<svg
					width="14"
					height="14"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"
						stroke-linecap="round"
						stroke-linejoin="round"
					/></svg
				>
				<a
					href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
					target="_blank"
					rel="noopener">Safe v1.4.1</a
				>
				—
				<a
					href={resolve('/docs/security-audits')}
					data-rybbit-event="audits_open"
					data-rybbit-prop-location="trust-chip">audited, unmodified</a
				>
			</div>
			{#if !countFailed}
				<div class="trust-chip">
					<span class="live-dot"></span>
					{#if countReady}
						<a href={resolve('/registry')} data-rybbit-event="registry_open">
							<span class="stat-number">{displayCount.toLocaleString()}</span>
						</a>
					{:else}
						<span class="stat-skeleton" aria-label="Loading wallet count"></span>
					{/if}

					wallets created on-chain
				</div>
			{/if}
		</div>
	</div>
</section>

<!-- Does Less -->
<section id="minimal" class="does-less">
	<div class="container">
		<div class="does-less-content">
			<h2>A wallet that does less — on purpose.</h2>
			<p>
				No NFT gallery. No built-in swaps. No DeFi dashboard. Nothing engineered to pull you toward
				the next thing to click.
			</p>
			<p>
				Vela holds ETH and ERC-20s. When you want to use a dApp, you connect to the one you choose
				through
				<a href="https://walletpair.org/" target="_blank" rel="noopener">WalletPair</a>.
			</p>
			<p>That's the whole product.</p>
			<p>
				Because every extra feature inside a wallet is more code to trust and more UI standing
				between you and your money. Vela stays small on purpose: fewer paths to attack, fewer moving
				parts to audit, and fewer chances to make a bad click.
			</p>
		</div>
	</div>
</section>

<!-- Why -->
<section id="why" class="why">
	<div class="container">
		<div class="why-content">
			<h2>Why we built Vela</h2>
			<p>
				We didn't set out to build another wallet. We started with a question we could never answer
				cleanly:
			</p>
			<p class="why-beat">Where are you supposed to keep twelve words?</p>
			<p>
				Put them in Notes and you're one stolen phone away from trouble. Write them on paper, and
				now you're thinking about fire, water, moving apartments, roommates, trash bags, and whether
				future-you will remember where "the safe place" was. The honest answer, for a lot of people,
				is a screenshot in the camera roll. Everyone knows it's wrong. They do it anyway — because
				the "right" answer is too hard to live with.
			</p>
			<p>
				Then passkeys changed what a wallet could feel like. We used <a
					href="https://account.base.app"
					target="_blank"
					rel="noopener">Base Account</a
				> every day, and signing with Face ID felt obvious in a way seed phrases never did — less like
				handling hazardous material, more like using the rest of the internet. But the more we used it,
				the more we hit edges we couldn't ignore: a recovery key generated in a browser that you just
				had to trust, no custom networks, no way to host it ourselves. And the quiet problem was the biggest
				one — if the service disappeared, the wallet disappeared with it.
			</p>
			<p>So we built the version we wanted to depend on.</p>
			<p>
				Vela is <strong>a passkey wallet you can fully own.</strong> Your passkey stays where your
				device already protects it — iCloud Keychain or Google Password Manager. When you sign a
				transaction, Vela sends a challenge to your device; your device signs it and sends back just
				the signature. Vela never sees the key itself. Most wallets still have a dangerous moment,
				even if it's brief: words on a screen, a seed phrase in memory, a recovery key sitting in a
				browser tab. Vela is designed so that moment never exists.
				<strong
					>We can't access your keys. Not "we promise not to" — we architecturally can't.</strong
				>
			</p>
			<p>
				We made Vela open source so you can check that for yourself, and self-hostable so your
				wallet never depends on our company staying online. And we built on unmodified <a
					href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
					target="_blank"
					rel="noopener">Safe contracts</a
				> because the boring, battle-tested path is the right one when people's money is involved — the
				same contracts already securing billions on-chain.
			</p>
			<p>
				There's still a trade-off. With Vela, your Apple or Google account matters, because that's
				where your passkey lives. Lose that account, or delete the passkey, and there's no seed
				phrase, no support reset, no back door. But every self-custodial wallet asks you to choose
				which risk you'd rather live with. A seed phrase can be copied, screenshotted, phished, or
				typed into the wrong site at 1 a.m. A passkey is different: there are no words to reveal, no
				secret to paste, and no fake site that can trick you into handing it over. Your device signs
				for the real domain, or it does not sign.
			</p>
			<p>
				That's why we built Vela — a wallet with no seed phrase to hide, no recovery key to trust,
				and no company you have to hope will stay around forever.
			</p>
		</div>
	</div>
</section>

<!-- Compare -->
<section id="compare" class="compare">
	<div class="container">
		<h2>How Vela compares</h2>
		<p class="section-desc">The differences that matter once you actually own your keys.</p>
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
					<tr>
						<td>Where your signing key lives</td>
						<td class="yes">Apple / Google Password Manager</td>
						<td class="warn">In the app</td>
						<td class="yes">Apple / Google Password Manager</td>
					</tr>
					<tr>
						<td>Key ever exposed to the app?</td>
						<td class="yes">No</td>
						<td class="warn">Yes</td>
						<td class="yes">No</td>
					</tr>
					<tr>
						<td>Open source</td>
						<td class="yes"
							><a
								href="https://github.com/orgs/mondaylabsltd/repositories"
								target="_blank"
								rel="noopener">All of it</a
							></td
						>
						<td class="warn">Partial</td>
						<td class="warn">Partial</td>
					</tr>
					<tr>
						<td>Account contract</td>
						<td class="yes"><a href="/docs/security-audits">Safe v1.4.1 — ecosystem standard</a></td
						>
						<td class="warn">EOA</td>
						<td class="warn">Custom (audited)</td>
					</tr>
					<tr>
						<td>Networks supported</td>
						<td>12 built-in + custom</td>
						<td class="yes">Any EVM</td>
						<td class="warn">Base-first, few</td>
					</tr>
					<tr>
						<td>Keeps working if the vendor disappears</td>
						<td class="yes"
							>Yes —
							<a
								href="https://github.com/mondaylabsltd/vela-wallet#self-deploy-service-endpoints"
								target="_blank"
								rel="noopener">self-hostable</a
							></td
						>
						<td class="yes">Yes</td>
						<td class="warn">Signing depends on vendor infra</td>
					</tr>
					<tr>
						<td>What you must back up</td>
						<td class="yes">Nothing — your passkey syncs automatically</td>
						<td class="no">Seed phrase</td>
						<td class="no">Recovery key</td>
					</tr>
				</tbody>
			</table>
		</div>
		<p class="compare-note">
			You can self-host everything Vela builds. A few data sources (some chains' history, long-tail
			prices, threat scanning) come from third-party providers — swap in your own node or key.
		</p>
	</div>
</section>

<!-- How It Works -->
<section id="how-it-works" class="how-it-works">
	<div class="container">
		<h2>How Vela works</h2>
		<p class="section-desc">What happens at each step.</p>

		<div class="pillar">
			<div class="pillar-number">01</div>
			<div class="pillar-content">
				<h3>Create a wallet</h3>
				<p>
					Authenticate with Face ID or fingerprint. Your device creates a passkey and derives a <a
						href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
						target="_blank"
						rel="noopener">Safe</a
					> smart account address from it — one address across all supported chains. No gas cost upfront.
					The contract deploys on-chain with your first transaction.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">02</div>
			<div class="pillar-content">
				<h3>Sign a transaction</h3>
				<p>
					The app builds a transaction and sends a signing challenge to your device. Your device
					signs it with the passkey and sends back just the signature — Vela never sees the key. The
					signed transaction goes on-chain through an <a
						href="https://eips.ethereum.org/EIPS/eip-4337"
						target="_blank"
						rel="noopener">ERC-4337</a
					> bundler.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">03</div>
			<div class="pillar-content">
				<h3>Sign in on a new device</h3>
				<p>
					Get a new phone, sign in with the same Apple or Google account. Your passkey syncs
					automatically through iCloud Keychain or Google Password Manager. Same address, same
					assets, same chains — no seed phrase to import, no recovery key to enter.
				</p>
			</div>
		</div>

		<div class="tech-details">
			<h3>Technical details</h3>
			<table>
				<tbody>
					<tr>
						<td>Wallet</td>
						<td
							><a
								href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
								target="_blank"
								rel="noopener">Safe v1.4.1</a
							></td
						>
					</tr>
					<tr>
						<td>Authentication</td>
						<td
							><a href="https://www.w3.org/TR/webauthn-2/" target="_blank" rel="noopener"
								>WebAuthn</a
							> / P-256</td
						>
					</tr>
					<tr>
						<td>Account type</td>
						<td
							><a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener"
								>ERC-4337</a
							> (Smart Account)</td
						>
					</tr>
					<tr>
						<td>Signer module</td>
						<td
							><a
								href="https://github.com/safe-global/safe-modules/tree/main/modules/passkey/contracts/4337"
								target="_blank"
								rel="noopener">SafeWebAuthnSharedSigner</a
							></td
						>
					</tr>
					<tr>
						<td>Networks</td>
						<td>12 EVM chains (+ custom)</td>
					</tr>
					<tr>
						<td>Source code</td>
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
				<p class="network-note">
					Custom networks need more than EVM compatibility — the chain must have the RIP-7212 P256
					precompile and Vela's Safe + ERC-4337 contracts deployed. Vela checks this when you add
					one, and the
					<a href="https://biubiu.tools/apps/vela-wallet-chain-setup" target="_blank" rel="noopener"
						>Chain Setup tool</a
					> can deploy them on chains that don't — including your own local testnet.
				</p>
			</div>
		</div>
	</div>
</section>

<!-- Business Model -->
<section id="pricing" class="business-model">
	<div class="container">
		<div class="bm-content">
			<h2>Free and open. Pay only if you want to.</h2>
			<p class="bm-intro">
				You're paying for convenience, not access. Everything is open source and self-hostable —
				nothing locks you in.
			</p>

			<div class="bm-grid">
				<div class="bm-card">
					<h4>Web wallet</h4>
					<div class="bm-price">Free</div>
					<p>
						The web wallet is free, open source, and self-hostable. No install, no seed phrase —
						just authenticate and go.
					</p>
				</div>
				<div class="bm-card">
					<h4>Mobile app</h4>
					<div class="bm-price">Funds the project</div>
					<p>
						The mobile app, when it ships, will be a paid download, priced by region — it's how a
						small, independent team funds building Vela in the open. It's open source too, so you
						can always build it from source and install it on your own phone for free.
					</p>
				</div>
				<div class="bm-card">
					<h4>Bundler gas fee</h4>
					<div class="bm-price">Network gas + service fee</div>
					<p>
						Transactions go through an ERC-4337 bundler. You pay network gas plus a service fee —
						the exact amount is quoted and shown to you before you sign, so you always know the
						total up front. You can skip the fee entirely by running a compatible
						<a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener"
							>self-hosted bundler</a
						>.
					</p>
				</div>
			</div>

			<p class="bm-note">
				Funded by the people who use it. Don't want to pay? Use the web wallet free, self-host the
				services, run your own bundler — and owe us nothing.
			</p>
		</div>
	</div>
</section>

<!-- FAQ -->
<section id="faq" class="faq">
	<div class="container">
		<h2>FAQ</h2>
		<p class="section-desc">What you'd want to know before putting real money in.</p>
		<div class="faq-list">
			<!-- Product -->
			<details>
				<summary>What chains does Vela support?</summary>
				<p>
					Twelve chains are built in: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
					Avalanche, Gnosis, Unichain, Monad, World Chain, and Tempo. You can add other EVM networks
					yourself if the chain has the RIP-7212 P256 precompile and Vela's Safe + ERC-4337
					contracts deployed — Vela checks when you add one. Same wallet address across all chains.
				</p>
			</details>
			<details>
				<summary>How is Vela different from Coinbase Smart Wallet or other passkey wallets?</summary
				>
				<p>
					Most passkey wallets are closed-source and run on infrastructure you can't control. If the
					company pivots or shuts down, you're stuck. Vela is fully open source and self-hostable —
					the app, the bundler, and all backend services. You can add custom networks, run your own
					bundler to skip fees, and keep using your wallet even if getvela.app disappears. No
					recovery keys generated in a browser. No vendor lock-in.
				</p>
			</details>
			<details>
				<summary>Can I use Vela with dApps?</summary>
				<p>
					Yes. Pair your phone with the <a
						href="https://walletpair.org/"
						target="_blank"
						rel="noopener">WalletPair extension</a
					> and sign transactions on desktop dApps using your phone's passkey.
				</p>
			</details>
			<details>
				<summary>Do I pay more gas than a regular wallet?</summary>
				<p>
					Yes. Smart account transactions have extra overhead from on-chain signature verification
					and the ERC-4337 EntryPoint. Expect roughly 1.5–3x the gas of a standard wallet transfer,
					depending on the chain — plus the bundler service fee described under pricing, unless you
					run your own bundler. Either way, the exact total is shown before you sign. That's the
					cost of passkey signing, no seed phrase, and one address across all chains.
				</p>
			</details>
			<!-- Security & recovery -->
			<details>
				<summary>What if I lose my phone?</summary>
				<p>
					Your passkey is backed up through iCloud Keychain (iOS) or Google Password Manager
					(Android) — as long as that sync is turned on. With it on, get a new phone, sign in with
					the same Apple/Google account, and your wallet is right there. If you've turned that sync
					off, your passkey stays on your old phone only, and losing the device means losing access.
				</p>
			</details>
			<details>
				<summary>What if I accidentally delete my passkey?</summary>
				<p>
					It's gone — and so is access to your wallet. There's no recovery mechanism. This is
					irreversible. If you ever clean up your password manager, know what each passkey is for
					before you remove it.
				</p>
			</details>
			<details>
				<summary>What if my Apple or Google account is compromised?</summary>
				<p>
					Anyone who can access your Apple/Google account and use your passkey could access your
					wallet. Enable two-factor authentication and use a strong, unique password — your
					Apple/Google account is part of your wallet security.
				</p>
			</details>
			<details>
				<summary>Can I add a second passkey as backup?</summary>
				<p>
					Yes — up to seven signers per wallet, and any one of them can sign on its own. They can
					be passkeys on different devices, a nearby phone you scan, or a USB/NFC security key.
					The catch: you choose them <strong>when you create the wallet</strong>, because your
					address is derived from the full set of keys — adding one later would be a different
					address. On top of that, each passkey is replicated by its own sync (iCloud Keychain,
					Google Password Manager) across your trusted devices.
				</p>
			</details>
			<!-- Trust & transparency -->
			<details>
				<summary>What happens if Vela shuts down?</summary>
				<p>
					Your wallet is a Safe smart contract on-chain — it doesn't depend on Vela's servers. The
					app and all backend services (chain data, passkey index, bundler) are open source, so you
					can deploy your own Vela interface and run your own services. Because your passkey signer
					is Vela-specific and bound to the getvela.app domain, you keep signing through Vela's own
					open-source code — your self-hosted instance plus the
					<a
						href="https://github.com/mondaylabsltd/vela-wallet#webauthn-proxy-extension-domain-recovery--dev-passkeys"
						target="_blank"
						rel="noopener">recovery extension</a
					> — not a generic Safe app.
				</p>
			</details>
			<details>
				<summary>What if the getvela.app domain goes offline?</summary>
				<p>
					Your funds stay on-chain regardless. Since passkeys are tied to a domain, Vela provides an
					open-source
					<a
						href="https://github.com/mondaylabsltd/vela-wallet#webauthn-proxy-extension-domain-recovery--dev-passkeys"
						target="_blank"
						rel="noopener">recovery extension</a
					>
					that lets you use your existing passkey from another domain or localhost.
				</p>
			</details>
			<details>
				<summary>Has the code been audited?</summary>
				<p>
					The Safe contracts and Safe WebAuthn signer module that Vela uses have been audited.
					Vela's own app code hasn't been independently audited yet — all source code is <a
						href="https://github.com/mondaylabsltd/vela-wallet"
						target="_blank"
						rel="noopener">public</a
					>
					for review. Every contract we depend on, its audit report, and the known issues we track are
					documented in
					<a
						href={resolve('/docs/security-audits')}
						data-rybbit-event="audits_open"
						data-rybbit-prop-location="faq">Audits &amp; known issues</a
					>.
				</p>
			</details>
		</div>
	</div>
</section>

<!-- CTA -->
<section id="notify" class="notify">
	<div class="container">
		<h2>Ready to try it?</h2>
		<p class="notify-sub">
			The web wallet is live and free. No install, no seed phrase — just authenticate and go.
		</p>
		<a
			href="https://wallet.getvela.app/"
			target="_blank"
			rel="noopener"
			class="btn btn-primary btn-cta-main"
			data-rybbit-event="cta_click"
			data-rybbit-prop-location="bottom">Create a wallet</a
		>

		<ul class="notify-cards">
			<li class="notify-card">
				<svg
					width="22"
					height="22"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M8 6l-5 6 5 6M16 6l5 6-5 6"
						stroke-linecap="round"
						stroke-linejoin="round"
					/></svg
				>
				<h4>Open source</h4>
				<p>Every line is on GitHub. Verify, don't trust.</p>
			</li>
			<li class="notify-card">
				<svg
					width="22"
					height="22"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><rect x="3" y="4" width="18" height="7" rx="1.5" /><rect
						x="3"
						y="13"
						width="18"
						height="7"
						rx="1.5"
					/><path d="M7 7.5h.01M7 16.5h.01" stroke-linecap="round" /></svg
				>
				<h4>Self-hostable</h4>
				<p>Run your own bundler and services.</p>
			</li>
			<li class="notify-card">
				<svg
					width="22"
					height="22"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"
						stroke-linecap="round"
						stroke-linejoin="round"
					/></svg
				>
				<h4>Battle-tested</h4>
				<p>
					Your account is an <a href={resolve('/docs/security-audits')}>audited</a>, unmodified Safe
					v1.4.1.
				</p>
			</li>
			<li class="notify-card">
				<svg
					width="22"
					height="22"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					stroke-width="2"
					><path
						d="M7 3H5a2 2 0 00-2 2v2M17 3h2a2 2 0 012 2v2M7 21H5a2 2 0 01-2-2v-2M17 21h2a2 2 0 002-2v-2M9 10v1M15 10v1M9.5 15a3.5 3.5 0 005 0"
						stroke-linecap="round"
						stroke-linejoin="round"
					/></svg
				>
				<h4>No seed phrase</h4>
				<p>Sign with a passkey. Nothing to write down.</p>
			</li>
		</ul>

		<div class="notify-divider"><span>mobile apps coming soon</span></div>

		<p class="notify-email-desc">
			Follow <a href="https://x.com/realvelawallet" target="_blank" rel="noopener"
				>@realvelawallet</a
			> and we'll post the moment iOS &amp; Android go live.
		</p>

		<div class="notify-social">
			<a
				href="https://x.com/realvelawallet"
				target="_blank"
				rel="noopener"
				class="btn btn-outline btn-social"
				data-rybbit-event="social_click"
				data-rybbit-prop-network="x">Follow on X</a
			>
			<a
				href="https://t.me/velawallet"
				target="_blank"
				rel="noopener"
				class="btn btn-outline btn-social"
				data-rybbit-event="social_click"
				data-rybbit-prop-network="telegram">Join Telegram</a
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
	.hero-signin {
		font-size: 0.85rem;
		color: var(--text-secondary);
		text-decoration: underline;
		text-underline-offset: 3px;
		transition: color 0.15s;
	}
	.hero-signin:hover {
		color: var(--text);
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
	.why-content a,
	.does-less-content a,
	.pillar-content a,
	.tech-details a,
	.compare-table td a,
	.bm-card a,
	details a {
		color: inherit;
		text-decoration: underline;
		text-underline-offset: 4px;
		transition: color 0.15s;
	}
	.why-content a:hover,
	.does-less-content a:hover,
	.pillar-content a:hover,
	.tech-details a:hover,
	.compare-table td a:hover,
	.bm-card a:hover,
	details a:hover {
		color: var(--accent);
	}
	/* External links get a trailing lucide external-link glyph, masked in
	   currentColor so it follows the link color. */
	.why-content a[target='_blank']::after,
	.does-less-content a[target='_blank']::after,
	.pillar-content a[target='_blank']::after,
	.tech-details a[target='_blank']::after,
	.compare-table td a[target='_blank']::after,
	.bm-card a[target='_blank']::after,
	details a[target='_blank']::after {
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

	/* ── Trust Strip ── */
	.trust-strip {
		padding: 32px 0 0;
	}
	.trust-tagline {
		text-align: center;
		color: var(--text-secondary);
		font-size: 0.9rem;
		line-height: 1.6;
		max-width: 720px;
		margin: 0 auto 20px;
	}
	.trust-tagline a {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 4px;
	}
	.trust-tagline a:hover {
		color: var(--accent);
	}
	.trust-row {
		display: flex;
		justify-content: center;
		flex-wrap: wrap;
		gap: 12px;
	}
	.trust-chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 7px 14px;
		border-radius: 999px;
		font-size: 0.78rem;
		color: var(--text-secondary);
		background: var(--bg-raised);
		border: 1px solid var(--border);
	}
	.trust-chip svg {
		color: var(--text-tertiary);
		flex-shrink: 0;
	}
	.trust-chip a {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 4px;
	}
	.trust-chip a:hover {
		color: var(--accent);
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
	.why-content strong {
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
