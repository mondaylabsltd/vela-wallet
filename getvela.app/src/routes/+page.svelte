<script lang="ts">
	import { resolve } from '$app/paths';

	let email = $state('');
	let subscribeStatus: 'idle' | 'loading' | 'success' | 'error' = $state('idle');
	let subscribeMessage = $state('');

	// Mockup animation steps: wallet → send → faceid → done
	type MockupStep = 'wallet' | 'send' | 'faceid' | 'done';
	const stepTimings: [MockupStep, number][] = [
		['wallet', 2500],
		['send', 2200],
		['faceid', 2000],
		['done', 2000],
	];
	let mockupStep: MockupStep = $state('wallet');
	let stepIndex = 0;

	$effect(() => {
		function nextStep() {
			stepIndex = (stepIndex + 1) % stepTimings.length;
			mockupStep = stepTimings[stepIndex][0];
			setTimeout(nextStep, stepTimings[stepIndex][1]);
		}
		const firstTimeout = setTimeout(nextStep, stepTimings[0][1]);
		return () => clearTimeout(firstTimeout);
	});

	async function handleSubscribe(e: Event) {
		e.preventDefault();
		if (!email.trim()) return;

		subscribeStatus = 'loading';
		try {
			const res = await fetch('https://newsletter.appsdata.xyz/v1/subscribe', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({
					email: email.trim(),
					source: 'getvela.app',
					referrer: document.referrer || undefined,
					timestamp: new Date().toISOString(),
					locale: {
						language: navigator.language || 'unknown',
						timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'unknown',
						utcOffset: -new Date().getTimezoneOffset(),
					},
					device: {
						userAgent: navigator.userAgent,
						deviceType: /Mobi|Android/i.test(navigator.userAgent) ? 'mobile' : 'desktop',
						platform: navigator.userAgent.includes('Mac') ? 'macOS' : navigator.userAgent.includes('Win') ? 'Windows' : navigator.userAgent.includes('Linux') ? 'Linux' : navigator.userAgent.includes('iPhone') ? 'iOS' : navigator.userAgent.includes('Android') ? 'Android' : 'unknown',
						browser: (() => {
							const ua = navigator.userAgent;
							if (ua.includes('Firefox')) return 'Firefox';
							if (ua.includes('Edg')) return 'Edge';
							if (ua.includes('Chrome')) return 'Chrome';
							if (ua.includes('Safari')) return 'Safari';
							return 'Other';
						})(),
						screenWidth: screen.width,
						screenHeight: screen.height,
						touchSupport: 'ontouchstart' in window || navigator.maxTouchPoints > 0,
					},
				}),
			});
			if (res.ok) {
				subscribeStatus = 'success';
				subscribeMessage = "You're on the list. We'll email you when mobile apps launch.";
				email = '';
			} else {
				const data = await res.json().catch(() => null);
				subscribeStatus = 'error';
				subscribeMessage = data?.message || 'Something went wrong. Please try again.';
			}
		} catch {
			subscribeStatus = 'error';
			subscribeMessage = 'Network error. Please try again.';
		}
	}

	const FALLBACK_RPCS = [
		'https://rpc.gnosischain.com',
		'https://rpc.gnosis.gateway.fm',
		'https://gnosis-rpc.publicnode.com',
		'https://rpc.ankr.com/gnosis',
		'https://gnosis-mainnet.public.blastapi.io',
		'https://gnosis.blockpi.network/v1/rpc/public',
		'https://gnosis.drpc.org',
		'https://1rpc.io/gnosis',
		'https://gnosis.oat.farm',
	];

	const RPC_SOURCE = 'https://ethereum-data.awesometools.dev/chains/eip155-100.json';
	const CONTRACT = '0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3';
	const CALLDATA =
		'0x3ebcb2150000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000b67657476656c612e617070000000000000000000000000000000000000000000';

	let rpcs = [...FALLBACK_RPCS];
	let displayCount = $state(0);
	let rpcIndex = 0;

	async function refreshRpcs() {
		try {
			const res = await fetch(RPC_SOURCE);
			const data = await res.json();
			const urls: string[] = (data?.rpc ?? [])
				.filter((r: { url: string }) => r.url.startsWith('https://') && !r.url.includes('${'))
				.map((r: { url: string }) => r.url);
			if (urls.length > 0) {
				rpcs = urls;
				rpcIndex = 0;
			}
		} catch {
			// keep using current rpcs
		}
	}

	async function fetchCount(): Promise<number | null> {
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
						params: [{ to: CONTRACT, data: CALLDATA }, 'latest'],
					}),
				});
				const json = await res.json();
				if (json.result) {
					return parseInt(json.result, 16);
				}
			} catch {
				// failover to next RPC
			}
			rpcIndex++;
		}
		return null;
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
		if (count !== null) animateCount(count);
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
</script>

<svelte:head>
	<title>Vela Wallet — No seed phrases. No recovery keys. No vendor lock-in.</title>
	<meta name="description" content="Self-custodial, self-hostable passkey wallet for ETH & EVM. Built on audited Safe smart contracts. 100% open source." />
	<meta property="og:title" content="Vela Wallet — No seed phrases. No recovery keys. No vendor lock-in." />
	<meta property="og:description" content="Self-custodial, self-hostable passkey wallet for ETH & EVM. Built on Safe. Open source." />
	<meta property="og:image" content="https://getvela.app/getvela-app-preview.png" />
	<meta property="og:url" content="https://getvela.app" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:image" content="https://getvela.app/getvela-app-preview.png" />
</svelte:head>

<!-- Nav -->
<nav>
	<div class="nav-inner">
		<a href={resolve('/')} class="logo">
			<img src="/vela-logo.png" alt="Vela Wallet" width="28" height="28" />
			<span>vela</span>
		</a>
		<div class="nav-links">
			<a href="https://wallet.getvela.app/" target="_blank" rel="noopener">Create wallet</a>
			<a href="#why">Why Vela</a>
			<a href="#how-it-works">How it works</a>
			<a href="#pricing">Pricing</a>
			<a href="#faq">FAQ</a>
			<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">GitHub</a>
		</div>
	</div>
</nav>

<!-- Hero -->
<section class="hero">
	<div class="container hero-grid">
		<div class="hero-text">
			<h1>Your keys.<br />Your face.</h1>
			<p class="subtitle">
				Sign transactions with passkeys — no seed phrases to lose, no hardware wallets to carry.
			</p>
			<div class="hero-cta">
				<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" class="btn btn-primary">Try it - no seed phrase needed</a>
				<p class="hero-note">Self-custodial, self-hostable wallet for ETH & EVM.</p>
			</div>
		</div>
		<div class="hero-visual">
			<div class="mockup-phone">
				<div class="mockup-screen">
					<!-- Status bar + notch (shared) -->
					<div class="mockup-statusbar">
						<span class="mockup-time">9:41</span>
						<div class="mockup-notch"></div>
						<div class="mockup-statusbar-icons">
							<svg width="12" height="10" viewBox="0 0 16 12" fill="currentColor"><rect x="0" y="8" width="3" height="4" rx="0.5" opacity="0.4"/><rect x="4.5" y="5" width="3" height="7" rx="0.5" opacity="0.6"/><rect x="9" y="2" width="3" height="10" rx="0.5" opacity="0.8"/><rect x="13.5" y="0" width="2.5" height="12" rx="0.5" opacity="1"/></svg>
							<svg width="18" height="10" viewBox="0 0 25 10" fill="currentColor"><rect x="0" y="0" width="21" height="10" rx="2" stroke="currentColor" stroke-width="1" fill="none" opacity="0.4"/><rect x="22" y="2.5" width="2" height="5" rx="1" opacity="0.25"/><rect x="1.5" y="1.5" width="14" height="7" rx="1" opacity="0.6"/></svg>
						</div>
					</div>

					<!-- Step 1: Wallet home -->
					<div class="mockup-step" class:active={mockupStep === 'wallet'}>
						<div class="mockup-account">
							<span class="mockup-account-name">My Wallet</span>
							<span class="mockup-account-addr">0x14fB…D1eA5c</span>
						</div>
						<div class="mockup-balance">$1,969<span class="mockup-cents">.53</span></div>
						<div class="mockup-actions">
							<div class="mockup-action">
								<div class="mockup-action-circle active">
									<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M4.5 10.5L12 3m0 0l7.5 7.5M12 3v18" stroke-linecap="round" stroke-linejoin="round"/></svg>
								</div>
								<span>Send</span>
							</div>
							<div class="mockup-action">
								<div class="mockup-action-circle">
									<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M19.5 13.5L12 21m0 0l-7.5-7.5M12 21V3" stroke-linecap="round" stroke-linejoin="round"/></svg>
								</div>
								<span>Receive</span>
							</div>
							<div class="mockup-action">
								<div class="mockup-action-circle">
									<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/></svg>
								</div>
								<span>History</span>
							</div>
						</div>
						<div class="mockup-tokens">
							<div class="mockup-token">
								<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_binance.jpg" alt="BNB" width="32" height="32" />
								<div class="mockup-token-info">
									<span class="mockup-token-name">BNB</span>
									<span class="mockup-token-chain">BNB Chain</span>
								</div>
								<div class="mockup-token-value">
									<span class="mockup-token-qty">1.1655</span>
									<span class="mockup-token-usd">$768.43</span>
								</div>
							</div>
							<div class="mockup-token">
								<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_ethereum.jpg" alt="ETH" width="32" height="32" />
								<div class="mockup-token-info">
									<span class="mockup-token-name">ETH</span>
									<span class="mockup-token-chain">Ethereum</span>
								</div>
								<div class="mockup-token-value">
									<span class="mockup-token-qty">0.1844</span>
									<span class="mockup-token-usd">$422.68</span>
								</div>
							</div>
							<div class="mockup-token">
								<img class="mockup-token-icon" src="https://assets.coingecko.com/coins/images/325/small/Tether.png" alt="USDT" width="32" height="32" />
								<div class="mockup-token-info">
									<span class="mockup-token-name">USDT</span>
									<span class="mockup-token-chain">Polygon</span>
								</div>
								<div class="mockup-token-value">
									<span class="mockup-token-qty">178.5160</span>
									<span class="mockup-token-usd">$178.52</span>
								</div>
							</div>
						</div>
					</div>

					<!-- Step 2: Confirm transaction -->
					<div class="mockup-step" class:active={mockupStep === 'send'}>
						<div class="mockup-step-header">Confirm Transaction</div>
						<div class="mockup-tx-card">
							<div class="mockup-tx-row">
								<span class="mockup-tx-label">Send</span>
								<span class="mockup-tx-value">0.05 ETH</span>
							</div>
							<div class="mockup-tx-row">
								<span class="mockup-tx-label">To</span>
								<span class="mockup-tx-value mockup-tx-addr">0x7a3B…9f2E</span>
							</div>
							<div class="mockup-tx-row">
								<span class="mockup-tx-label">Network</span>
								<span class="mockup-tx-value">Ethereum</span>
							</div>
							<div class="mockup-tx-row last">
								<span class="mockup-tx-label">Gas fee</span>
								<span class="mockup-tx-value">~$0.42</span>
							</div>
						</div>
						<div class="mockup-slide-track">
							<div class="mockup-slide-thumb">
								<svg width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" stroke-linecap="round" stroke-linejoin="round"/></svg>
							</div>
							<span class="mockup-slide-text">Slide to confirm</span>
						</div>
					</div>

					<!-- Step 3: Face ID signing -->
					<div class="mockup-step" class:active={mockupStep === 'faceid'}>
						<div class="mockup-faceid-screen">
							<div class="mockup-faceid-icon">
								<svg width="64" height="64" viewBox="0 0 96 96" fill="none">
									<path d="M28 8h-12a8 8 0 00-8 8v12" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M68 8h12a8 8 0 018 8v12" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M28 88h-12a8 8 0 01-8-8v-12" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M68 88h12a8 8 0 008-8v-12" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M36 36v10" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M60 36v10" stroke="currentColor" stroke-width="4" stroke-linecap="round"/>
									<path d="M48 44v10h-4" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
									<path d="M36 64c2 6 10 10 16 10s12-4 16-10" stroke="currentColor" stroke-width="3" stroke-linecap="round" fill="none"/>
								</svg>
							</div>
							<span class="mockup-faceid-label">Sign with Face ID</span>
							<span class="mockup-faceid-sub">Confirm with passkey to send 0.05 ETH</span>
						</div>
					</div>

					<!-- Step 4: Success -->
					<div class="mockup-step" class:active={mockupStep === 'done'}>
						<div class="mockup-done-screen">
							<div class="mockup-done-check">
								<svg width="48" height="48" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
									<path d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
							</div>
							<span class="mockup-done-title">Transaction Sent</span>
							<span class="mockup-done-detail">0.05 ETH → 0x7a3B…9f2E</span>
							<span class="mockup-done-time">Confirmed in 3s</span>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
	<div class="scroll-hint">
		<svg width="20" height="20" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19.5 13.5L12 21m0 0l-7.5-7.5M12 21V3" stroke-linecap="round" stroke-linejoin="round"/></svg>
	</div>
</section>

<!-- Trust Strip -->
<section class="trust-strip">
	<div class="container">
		<div class="trust-row">
			{#if displayCount > 0}
				<div class="trust-chip">
					<span class="live-dot"></span>
						<a href="https://gnosisscan.io/address/0xdd93420bd49baabdff4a363ddd300622ae87e9c3#readContract#F14" target="_blank" rel="noopener">					<span class="stat-number">{displayCount.toLocaleString()}</span> </a>
		

					wallets created on-chain
				</div>
			{/if}
			<div class="trust-chip">
				<svg width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z" stroke-linecap="round" stroke-linejoin="round"/></svg>
				<a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe v1.4.1</a> — audited, unmodified
			</div>
			<div class="trust-chip">
				<svg width="14" height="14" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17.25 6.75L22.5 12l-5.25 5.25m-10.5 0L1.5 12l5.25-5.25m7.5-3l-4.5 16.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
				<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">100% open source</a> — app + all services
			</div>
		</div>
	</div>
</section>

<!-- Why -->
<section id="why" class="why">
	<div class="container">
		<div class="why-content">
			<h2>Why we built Vela</h2>
			<p>
				Managing a seed phrase is hard. Not "I got hacked" hard — quietly, constantly hard.
				Where do you store 12 words so they survive a house fire, a stolen phone, and your own forgetfulness?
				Most people end up with a screenshot in their photo library. That's not security. That's a liability.
			</p>
			<p>
				Passkey wallets make this much easier. We used <a href="https://account.base.app" target="_blank" rel="noopener">Base Account</a> daily — great UX, loved signing with Face ID.
				But we kept hitting walls: a recovery key generated in the browser that we had to trust blindly.
				No way to add custom networks. No way to self-host.
				And no way to keep using the wallet if the service goes away.
			</p>
			<p>
				We built Vela because <strong>we wanted a passkey wallet we could fully own.</strong>
				Your passkey lives in iCloud Keychain or Google Password Manager.
				When you sign a transaction, the app sends a challenge — your credential manager signs it internally and returns only the result.
				Your passkey's private key never leaves the credential manager. The app never sees it.
				With most software seed phrase wallets, there's a moment where your secret exists in app memory. With recovery key wallets, there's a moment where the recovery key is generated in a browser.
				In Vela, that moment doesn't exist. <strong>We can't access your keys. Not "we promise not to" — we architecturally can't.</strong>
			</p>
			<p>
				Open source, so you can verify every line.
				Self-hostable, so you're never dependent on us.
				Built on unmodified Safe contracts — you're not trusting a custom wallet contract, you're trusting the same contracts that secure the broader ecosystem.
			</p>
			<p>
				<strong>The trade-off:</strong> you're trusting iCloud Keychain or Google Password Manager to safeguard your passkey.
				If you lose access to your Apple or Google account, or accidentally delete the passkey from your credential manager, you lose access to your wallet.
				There is no recovery mechanism — no backup seed, no support ticket, no reset.
				This is true of any self-custodial wallet — the question is what you're protecting.
				With a seed phrase wallet, you're protecting 12 words that can be copied, screenshotted, exported, and phished.
				With Vela, you're protecting access to the credential manager that holds your passkey: your Apple or Google account, your trusted devices, and the passkey itself.
			</p>
		</div>
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
					Tap create, authenticate with Face ID or fingerprint. Your device generates a passkey
					and derives a <a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe</a> smart account address from it —
					the same address works across all supported chains.
					No deployment cost upfront. The contract activates on-chain with your first transaction.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">02</div>
			<div class="pillar-content">
				<h3>Sign a transaction</h3>
				<p>
					The app builds a transaction and sends a signing challenge to your credential manager.
					Your credential manager signs it internally with your passkey and returns only the signature.
					The signed transaction is submitted on-chain through an <a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener">ERC-4337</a> bundler.
					The app never touches your passkey's private key at any point.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">03</div>
			<div class="pillar-content">
				<h3>Sign in on a new device</h3>
				<p>
					Get a new phone, sign in with the same Apple or Google account.
					Your passkey syncs automatically through iCloud Keychain or Google Password Manager.
					Same address, same assets, same chains — no seed phrase to import, no recovery key to enter.
				</p>
			</div>
		</div>

		<div class="tech-details">
			<h3>Technical details</h3>
			<table>
				<tbody>
					<tr>
						<td>Wallet</td>
						<td><a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe v1.4.1</a></td>
					</tr>
					<tr>
						<td>Authentication</td>
						<td><a href="https://www.w3.org/TR/webauthn-2/" target="_blank" rel="noopener">WebAuthn</a> / P-256</td>
					</tr>
					<tr>
						<td>Account type</td>
						<td><a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener">ERC-4337</a> (Smart Account)</td>
					</tr>
					<tr>
						<td>Signer module</td>
						<td><a href="https://github.com/safe-global/safe-modules/tree/main/modules/passkey/contracts/4337" target="_blank" rel="noopener">SafeWebAuthnSharedSigner</a></td>
					</tr>
					<tr>
						<td>Networks</td>
						<td>8 EVM chains (+ custom)</td>
					</tr>
					<tr>
						<td>Source code</td>
						<td><a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">GitHub</a></td>
					</tr>
				</tbody>
			</table>
		</div>
	</div>
</section>


<!-- Business Model -->
<section id="pricing" class="business-model">
	<div class="container">
		<div class="bm-content">
			<h2>How Vela sustains itself</h2>
			<p class="bm-intro">
				No VC funding, no token, no ads, no data sales. Vela is funded by the people who use it.
				We're transparent about how we make money because you deserve to know.
			</p>

			<div class="bm-grid">
				<div class="bm-card">
					<h4>Web wallet</h4>
					<div class="bm-price">Free</div>
					<p>Full-featured web wallet. No time limit, no feature restrictions.</p>
				</div>
				<div class="bm-card">
					<h4>Mobile app</h4>
					<div class="bm-price">Paid download</div>
					<p>
						iOS & Android via App Store and Google Play.
						Pricing is adjusted by region so the app is accessible worldwide.
						No subscriptions. No in-app purchases.
					</p>
				</div>
				<div class="bm-card">
					<h4>Bundler gas fee</h4>
					<div class="bm-price">Network gas + service fee</div>
					<p>
						Transactions go through an ERC-4337 bundler. Your total cost includes network gas plus a bundler service fee.
						You can avoid the bundler fee by using a compatible
						<a href="https://github.com/atshelchin/vela-bundler" target="_blank" rel="noopener">self-hosted bundler</a>.
					</p>
				</div>
			</div>

			<p class="bm-note">
				This revenue keeps the bundler running, the app maintained, and the project independent.
				Nothing is locked in — you can swap to any ERC-4337 bundler, build the mobile app from source, and self-host all services.
				You're paying for convenience, not for access — and for keeping an independent, open-source wallet alive in the ecosystem.
			</p>
		</div>
	</div>
</section>

<!-- FAQ -->
<section id="faq" class="faq">
	<div class="container">
		<h2>FAQ</h2>
		<p class="section-desc">Common questions about security, recovery, and trust.</p>
		<div class="faq-list">
			<details>
				<summary>What if I lose my phone?</summary>
				<p>
					Your passkey syncs through iCloud Keychain (iOS) or Google Password Manager (Android).
					Get a new phone, sign in with the same Apple/Google account, and you can access your wallet again.
				</p>
			</details>
			<details>
				<summary>What if my iCloud or Google account is compromised?</summary>
				<p>
					If someone gains control of your Apple/Google account and can sync or use your passkey, they may be able to access your wallet.
					We strongly recommend enabling two-factor authentication and using a strong, unique password.
					Treat your Apple/Google account as part of your wallet security.
				</p>
			</details>
			<details>
				<summary>What happens if Vela shuts down?</summary>
				<p>
					Your wallet is a Safe smart contract on the blockchain — it doesn't depend on Vela's servers.
					All three backend services (chain data, passkey index, bundler) are open source and self-hostable.
					Your passkey still works, and you can interact with your wallet through any Safe-compatible interface.
				</p>
			</details>
			<details>
				<summary>What if the getvela.app domain goes offline?</summary>
				<p>
					If getvela.app goes offline, your funds remain safe on-chain.
					Because passkeys are normally tied to a domain, Vela provides an open-source
					<a href="https://github.com/atshelchin/vela-wallet#webauthn-proxy-extension-domain-recovery--dev-passkeys" target="_blank" rel="noopener">recovery extension</a>
					that lets you use your existing passkey from another domain or localhost.
				</p>
			</details>
			<details>
				<summary>Can I use Vela with dApps?</summary>
				<p>
					Yes. Vela supports DApp Connect — pair your phone with the Vela browser extension via Bluetooth
					or relay, and sign transactions on desktop dApps using your phone's passkey.
					Supports eth_sendTransaction, personal_sign, and eth_signTypedData_v4.
				</p>
			</details>
			<details>
				<summary>Can I add a second passkey as backup?</summary>
				<p>
					No. Each wallet is bound to a single passkey — this is a fundamental design constraint of the WebAuthn signer architecture.
					Your backup is iCloud Keychain or Google Password Manager's built-in cross-device sync, which replicates
					the passkey across all your trusted devices automatically.
				</p>
			</details>
			<details>
				<summary>Has the code been audited?</summary>
				<p>
					The Safe contracts and Safe WebAuthn signer module Vela relies on have been audited.
					Vela's own app code has not yet received a third-party audit.
					All source code is <a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">public</a> for review.
				</p>
			</details>
		</div>
	</div>
</section>

<!-- CTA -->
<section id="notify" class="notify">
	<div class="container">
		<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" class="btn btn-primary btn-cta-main">Create wallet</a>

		<div class="notify-divider"><span>stay in the loop</span></div>

		<h2>Get notified when mobile apps launch</h2>

		{#if subscribeStatus === 'success'}
			<div class="subscribe-success">
				<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
					<path d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				<span>{subscribeMessage}</span>
			</div>
		{:else}
			<form class="subscribe-form" onsubmit={handleSubscribe}>
				<input type="email" bind:value={email} placeholder="you@example.com" required class="subscribe-input" />
				<button type="submit" class="btn btn-primary subscribe-btn" disabled={subscribeStatus === 'loading'}>
					{subscribeStatus === 'loading' ? 'Subscribing...' : 'Notify Me'}
				</button>
			</form>
			{#if subscribeStatus === 'error'}
				<p class="subscribe-error">{subscribeMessage}</p>
			{/if}
		{/if}

		<div class="notify-divider"><span>or</span></div>

		<a href="https://x.com/realvelawallet" target="_blank" rel="noopener" class="btn btn-x">
			<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>
			Follow @realvelawallet
		</a>
	</div>
</section>

<!-- Footer -->
<footer>
	<div class="container footer-inner">
		<div class="footer-left">
			<div class="footer-brand">
				<img src="/vela-logo.png" alt="Vela" width="24" height="24" />
				<span>Vela Wallet</span>
			</div>
			<p class="footer-copy">&copy; {new Date().getFullYear()} Vela Wallet</p>
		</div>
		<div class="footer-links">
			<a href={resolve('/privacy')}>Privacy</a>
			<a href={resolve('/terms')}>Terms</a>
			<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">GitHub</a>
			<a href="https://x.com/realvelawallet" target="_blank" rel="noopener">X</a>
			<a href="https://t.me/velawallet" target="_blank" rel="noopener">Telegram</a>
		</div>
	</div>
</footer>

<style>
	/* ── Palette ── */
	:root {
		--bg: #0F0E0C;
		--bg-raised: #1A1918;
		--bg-card: #1E1D1B;
		--border: #2A2926;
		--border-accent: rgba(232, 87, 42, 0.2);
		--text: #E8E6E1;
		--text-secondary: #9A9790;
		--text-tertiary: #6B6963;
		--accent: #E8572A;
		--accent-soft: rgba(232, 87, 42, 0.1);
		--green: #2D8E5F;
		--green-soft: rgba(45, 142, 95, 0.1);
		--radius: 14px;
		--max-w: 960px;
	}

	/* ── Base ── */
	.container { max-width: var(--max-w); margin: 0 auto; padding: 0 24px; }
	section { padding: 120px 0; }

	h2 {
		font-size: 2rem;
		font-weight: 700;
		letter-spacing: -0.025em;
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
		position: fixed; top: 0; left: 0; right: 0; z-index: 100;
		background: rgba(15, 14, 12, 0.92);
		backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--border);
	}
	.nav-inner {
		max-width: var(--max-w); margin: 0 auto; padding: 0 24px;
		height: 52px; display: flex; align-items: center; justify-content: space-between;
	}
	.logo { display: flex; align-items: center; gap: 9px; font-weight: 700; font-size: 1.1rem; letter-spacing: 0.5px; }
	.logo img { border-radius: 7px; }
	.nav-links { display: flex; gap: 24px; }
	.nav-links a { color: var(--text-secondary); font-size: 0.82rem; font-weight: 500; transition: color 0.15s; }
	.nav-links a:hover { color: var(--text); }

	/* ── Hero ── */
	.hero { padding: 80px 0 80px; min-height: 100vh; display: flex; align-items: center; position: relative; }
	.hero-grid {
		display: grid;
		grid-template-columns: 1fr 380px;
		gap: 48px;
		align-items: center;
	}
	.hero-text { text-align: left; }
	h1 {
		font-size: clamp(2.5rem, 5vw, 3.2rem); font-weight: 700; line-height: 1.1;
		letter-spacing: -0.03em; margin-bottom: 24px;
	}
	.subtitle {
		color: var(--text-secondary); font-size: 1.05rem; line-height: 1.75;
		max-width: 520px; margin-bottom: 28px;
	}
	.hero-cta { display: flex; flex-direction: column; align-items: flex-start; gap: 14px; }
	.hero-note { color: var(--text-tertiary); font-size: 0.78rem; margin: 0; }

	/* ── Scroll Hint ── */
	.scroll-hint {
		position: absolute; bottom: 32px; left: 50%; transform: translateX(-50%);
		color: var(--text-tertiary); opacity: 0.5;
		animation: breathe 2.5s ease-in-out infinite;
	}
	@keyframes breathe {
		0%, 100% { opacity: 0.2; transform: translateX(-50%) translateY(0); }
		50% { opacity: 0.6; transform: translateX(-50%) translateY(6px); }
	}

	/* ── Live Stat (trust strip) ── */
	.live-dot {
		width: 6px; height: 6px; border-radius: 50%; background: var(--green);
		box-shadow: 0 0 6px rgba(45, 142, 95, 0.5);
		animation: pulse-dot 2s ease-in-out infinite;
	}
	.stat-number { font-size: 0.92rem; font-weight: 700; font-variant-numeric: tabular-nums; color: var(--accent); }
	@keyframes pulse-dot { 0%, 100% { opacity: 1; } 50% { opacity: 0.35; } }

	/* ── Buttons ── */
	.btn {
		display: inline-block; padding: 11px 22px; border-radius: 10px;
		font-size: 0.88rem; font-weight: 600; cursor: pointer;
		transition: all 0.15s; border: none; font-family: inherit;
	}
	.btn-primary { background: var(--accent); color: #fff; }
	.btn-primary:hover { transform: translateY(-1px); box-shadow: 0 4px 16px rgba(232, 87, 42, 0.3); }
	.btn-cta-main { padding: 14px 36px; font-size: 1rem; margin-bottom: 32px; }

	/* ── Phone Mockup ── */
	.hero-visual {
		max-height: 520px;
		overflow: hidden;
		-webkit-mask-image: linear-gradient(to bottom, #000 60%, transparent 100%);
		mask-image: linear-gradient(to bottom, #000 60%, transparent 100%);
	}
	.mockup-phone {
		width: 320px; margin: 0 auto;
		background: #1C1B19;
		border-radius: 36px;
		border: 1px solid rgba(255, 255, 255, 0.06);
		padding: 10px;
		box-shadow:
			0 0 0 1px rgba(255, 255, 255, 0.04) inset,
			0 8px 24px rgba(0, 0, 0, 0.3),
			0 32px 80px rgba(0, 0, 0, 0.5),
			0 0 120px rgba(232, 87, 42, 0.06);
	}
	.mockup-screen {
		background: var(--bg);
		border-radius: 26px;
		padding: 0 16px 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	/* Status bar + Dynamic Island */
	.mockup-statusbar {
		display: flex; align-items: center; justify-content: space-between;
		padding: 8px 8px 6px; font-size: 0.65rem; font-weight: 600;
		color: var(--text-tertiary); position: relative;
	}
	.mockup-time { width: 36px; font-variant-numeric: tabular-nums; }
	.mockup-notch {
		width: 48px; height: 14px; border-radius: 10px;
		background: #000; position: absolute; top: 6px; left: 50%;
		transform: translateX(-50%);
	}
	.mockup-statusbar-icons {
		display: flex; align-items: center; gap: 4px; width: 36px;
		justify-content: flex-end; color: var(--text-tertiary);
	}
	/* ── Mockup Steps (animated flow) ── */
	.mockup-step {
		display: none; flex-direction: column; padding: 0 4px 16px;
		opacity: 0; transition: opacity 0.4s ease;
	}
	.mockup-step.active { display: flex; opacity: 1; }

	/* Step 1: Wallet home */
	.mockup-account { display: flex; flex-direction: column; align-items: center; gap: 1px; margin-bottom: 16px; }
	.mockup-account-name { font-size: 0.88rem; font-weight: 600; color: var(--text); }
	.mockup-account-addr { font-size: 0.65rem; color: var(--text-tertiary); font-family: monospace; }
	.mockup-balance {
		text-align: center; font-size: 2.2rem; font-weight: 700;
		color: var(--text); letter-spacing: -0.02em; margin-bottom: 16px;
		font-variant-numeric: tabular-nums;
	}
	.mockup-cents { font-size: 1.3rem; color: var(--text-secondary); }
	.mockup-actions { display: flex; justify-content: center; gap: 28px; margin-bottom: 18px; }
	.mockup-action { display: flex; flex-direction: column; align-items: center; gap: 5px; }
	.mockup-action span { font-size: 0.65rem; color: var(--text-secondary); }
	.mockup-action-circle {
		width: 42px; height: 42px; border-radius: 50%;
		border: 1.5px solid var(--border); background: transparent;
		display: flex; align-items: center; justify-content: center;
		color: var(--text-secondary);
	}
	.mockup-action-circle.active { background: var(--accent); border-color: var(--accent); color: #fff; }
	.mockup-tokens { display: flex; flex-direction: column; }
	.mockup-token {
		display: flex; align-items: center; gap: 12px;
		padding: 10px 0; border-bottom: 1px solid var(--border);
	}
	.mockup-token:last-child { border-bottom: none; }
	.mockup-token-icon { width: 32px; height: 32px; border-radius: 50%; flex-shrink: 0; }
	.mockup-token-info { flex: 1; display: flex; flex-direction: column; gap: 2px; }
	.mockup-token-name { font-size: 0.82rem; font-weight: 600; color: var(--text); }
	.mockup-token-chain { font-size: 0.65rem; color: var(--text-tertiary); }
	.mockup-token-value { text-align: right; display: flex; flex-direction: column; gap: 2px; }
	.mockup-token-qty { font-size: 0.82rem; font-weight: 600; color: var(--text); font-variant-numeric: tabular-nums; }
	.mockup-token-usd { font-size: 0.65rem; color: var(--text-tertiary); font-variant-numeric: tabular-nums; }

	/* Step 2: Confirm tx */
	.mockup-step-header {
		font-size: 0.92rem; font-weight: 600; color: var(--text);
		text-align: center; margin-bottom: 20px; margin-top: 8px;
	}
	.mockup-tx-card {
		width: 100%; background: var(--bg-raised);
		border: 1px solid var(--border); border-radius: 12px;
		padding: 14px 16px; margin-bottom: 24px;
	}
	.mockup-tx-row {
		display: flex; justify-content: space-between; align-items: center;
		padding: 9px 0; border-bottom: 1px solid var(--border);
	}
	.mockup-tx-row.last { border-bottom: none; }
	.mockup-tx-label { font-size: 0.75rem; color: var(--text-tertiary); }
	.mockup-tx-value { font-size: 0.78rem; font-weight: 600; color: var(--text); }
	.mockup-tx-addr { font-family: monospace; font-size: 0.72rem; }
	.mockup-slide-track {
		position: relative; width: 100%; height: 44px;
		background: var(--bg-raised); border: 1px solid var(--border);
		border-radius: 22px; display: flex; align-items: center;
		justify-content: center; overflow: hidden;
	}
	.mockup-slide-thumb {
		position: absolute; left: 4px; top: 4px;
		width: 36px; height: 36px; border-radius: 50%;
		background: var(--accent); color: #fff;
		display: flex; align-items: center; justify-content: center;
		animation: slide-hint 2s ease-in-out infinite;
	}
	@keyframes slide-hint { 0%, 100% { left: 4px; } 50% { left: 28px; } }
	.mockup-slide-text { font-size: 0.72rem; color: var(--text-tertiary); font-weight: 500; }

	/* Step 3: Face ID */
	.mockup-faceid-screen {
		display: flex; flex-direction: column; align-items: center;
		justify-content: center; gap: 16px;
		padding: 60px 0 40px;
	}
	.mockup-faceid-icon {
		color: var(--accent);
		animation: faceid-pulse 1.8s ease-in-out infinite;
	}
	@keyframes faceid-pulse {
		0%, 100% { opacity: 0.7; transform: scale(1); }
		50% { opacity: 1; transform: scale(1.08); }
	}
	.mockup-faceid-label { font-size: 0.92rem; font-weight: 600; color: var(--text); }
	.mockup-faceid-sub { font-size: 0.72rem; color: var(--text-tertiary); text-align: center; }

	/* Step 4: Success */
	.mockup-done-screen {
		display: flex; flex-direction: column; align-items: center;
		justify-content: center; gap: 12px;
		padding: 50px 0 40px;
	}
	.mockup-done-check { color: var(--green); animation: done-pop 0.5s ease-out; }
	@keyframes done-pop {
		0% { transform: scale(0.5); opacity: 0; }
		70% { transform: scale(1.15); }
		100% { transform: scale(1); opacity: 1; }
	}
	.mockup-done-title { font-size: 1rem; font-weight: 700; color: var(--text); }
	.mockup-done-detail { font-size: 0.78rem; color: var(--text-secondary); font-family: monospace; }
	.mockup-done-time { font-size: 0.72rem; color: var(--green); font-weight: 600; }

	/* ── Trust Strip ── */
	.trust-strip { padding: 32px 0 0; }
	.trust-row { display: flex; justify-content: center; flex-wrap: wrap; gap: 12px; }
	.trust-chip {
		display: inline-flex; align-items: center; gap: 6px;
		padding: 7px 14px; border-radius: 999px; font-size: 0.78rem;
		color: var(--text-secondary); background: var(--bg-raised);
		border: 1px solid var(--border);
	}
	.trust-chip svg { color: var(--text-tertiary); flex-shrink: 0; }
	.trust-chip a { color: var(--text); text-decoration: underline; text-underline-offset: 2px; }
	.trust-chip a:hover { color: var(--accent); }

	/* ── Why ── */
	.why { padding: 120px 0 60px; }
	.why-content {
		max-width: 640px; margin: 0 auto;
	}
	.why-content h2 {
		text-align: left; margin-bottom: 20px;
	}
	.why-content p {
		color: var(--text-secondary); font-size: 1rem; line-height: 1.8;
		margin-bottom: 16px;
	}
	.why-content strong {
		color: var(--text);
	}

	/* ── How It Works (Pillars) ── */
	.how-it-works h2, .how-it-works .section-desc { text-align: center; }
	.pillar {
		display: flex; gap: 32px; align-items: flex-start;
		max-width: 640px; margin: 0 auto 56px;
		padding: 28px 28px 28px 32px;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-left: 3px solid var(--accent);
		border-radius: 0 var(--radius) var(--radius) 0;
	}
	.pillar:last-child { margin-bottom: 0; }
	.pillar-number {
		font-size: 0.78rem; font-weight: 700; color: var(--accent);
		background: var(--accent-soft); border: 1px solid var(--border-accent);
		padding: 6px 12px; border-radius: 8px; flex-shrink: 0;
		font-variant-numeric: tabular-nums; letter-spacing: 0.02em;
	}
	.pillar-content h3 { font-size: 1.1rem; font-weight: 600; margin-bottom: 10px; line-height: 1.3; }
	.pillar-content p { color: var(--text-secondary); font-size: 0.92rem; line-height: 1.75; margin-bottom: 12px; }
	.pillar-content p:last-child, .pillar-content p:last-of-type { margin-bottom: 0; }
	.pillar-content a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }
	/* ── Tech Details ── */
	.tech-details {
		max-width: 640px; margin: 56px auto 0;
		background: var(--bg-raised); border: 1px solid var(--border);
		border-radius: var(--radius); padding: 28px 28px;
	}
	.tech-details h3 {
		font-size: 0.82rem; font-weight: 600; text-transform: uppercase;
		letter-spacing: 0.06em; color: var(--text-secondary); margin-bottom: 16px;
	}
	.tech-details table { width: 100%; border-collapse: collapse; }
	.tech-details td {
		padding: 8px 0; font-size: 0.88rem; border-bottom: 1px solid var(--border);
	}
	.tech-details tr:last-child td { border-bottom: none; }
	.tech-details td:first-child { color: var(--text-secondary); width: 40%; }
	.tech-details td:last-child { color: var(--text); font-weight: 500; }
	.tech-details a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }


	/* ── Business Model ── */
	.business-model h2 { text-align: center; }
	.bm-content { max-width: 720px; margin: 0 auto; }
	.bm-intro {
		text-align: center; color: var(--text-secondary);
		font-size: 1rem; line-height: 1.7; margin-bottom: 40px; max-width: 560px; margin-left: auto; margin-right: auto;
	}
	.bm-grid {
		display: flex; flex-direction: column; gap: 16px;
		max-width: 560px; margin: 0 auto 32px;
	}
	.bm-card {
		background: var(--bg-raised); border: 1px solid var(--border);
		border-radius: var(--radius); padding: 24px 20px;
	}
	.bm-card h4 { font-size: 0.88rem; font-weight: 600; margin-bottom: 8px; }
	.bm-price {
		font-size: 1.1rem; font-weight: 700; color: var(--accent); margin-bottom: 12px;
	}
	.bm-card p { color: var(--text-secondary); font-size: 0.82rem; line-height: 1.6; margin-bottom: 8px; }
	.bm-card p:last-child { margin-bottom: 0; }
.bm-card a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }
	.bm-note {
		text-align: center; color: var(--text-tertiary); font-size: 0.82rem;
		line-height: 1.6; max-width: 560px; margin: 0 auto;
	}

	/* ── FAQ ── */
	.faq h2 { text-align: center; }
	.faq .section-desc { text-align: center; }
	.faq-list { max-width: 600px; margin: 0 auto; }
	details { border-bottom: 1px solid var(--border); }
	summary {
		padding: 16px 0; cursor: pointer; font-weight: 600;
		font-size: 0.92rem; list-style: none;
		display: flex; justify-content: space-between; align-items: center;
		color: var(--text);
	}
	summary::after { content: '+'; font-size: 1.2rem; color: var(--text-tertiary); }
	details[open] summary::after { content: '\2212'; }
	details p { padding-bottom: 16px; color: var(--text-secondary); line-height: 1.7; font-size: 0.88rem; }
	details a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }

	/* ── Notify ── */
	.notify { text-align: center; padding: 64px 0 80px; }
	.notify h2 { margin-bottom: 28px; }
	.subscribe-form { display: flex; align-items: center; justify-content: center; gap: 8px; max-width: 400px; margin: 0 auto; }
	.subscribe-input {
		flex: 1; padding: 11px 16px; border-radius: 10px;
		border: 1px solid var(--border); background: var(--bg-raised);
		color: var(--text); font-size: 0.88rem; font-family: inherit;
		outline: none; transition: border-color 0.15s;
	}
	.subscribe-input::placeholder { color: var(--text-tertiary); }
	.subscribe-input:focus { border-color: var(--accent); }
	.subscribe-btn { white-space: nowrap; }
	.subscribe-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.subscribe-success {
		display: inline-flex; align-items: center; gap: 8px;
		padding: 11px 20px; border-radius: 10px;
		background: var(--green-soft); border: 1px solid rgba(45, 142, 95, 0.25);
		color: var(--green); font-weight: 500; font-size: 0.88rem;
	}
	.subscribe-error { color: #EF6B6B; font-size: 0.8rem; margin-top: 10px; }
	.notify-divider { display: flex; align-items: center; gap: 16px; margin: 24px auto; max-width: 400px; }
	.notify-divider::before, .notify-divider::after { content: ''; flex: 1; height: 1px; background: var(--border); }
	.notify-divider span { color: var(--text-tertiary); font-size: 0.78rem; }
	.btn-x {
		display: inline-flex; align-items: center; gap: 7px;
		padding: 10px 22px; border-radius: 10px; font-size: 0.88rem;
		font-weight: 600; background: #fff; color: #000;
		transition: all 0.15s; border: none; font-family: inherit;
	}
	.btn-x:hover { background: #e5e5e5; transform: translateY(-1px); }

	/* ── Footer ── */
	footer { padding: 28px 0; border-top: 1px solid var(--border); }
	.footer-inner { display: flex; align-items: center; justify-content: space-between; }
	.footer-left { display: flex; align-items: center; gap: 16px; }
	.footer-brand { display: flex; align-items: center; gap: 7px; font-weight: 600; font-size: 0.85rem; }
	.footer-brand img { border-radius: 5px; }
	.footer-copy { color: var(--text-tertiary); font-size: 0.75rem; }
	.footer-links { display: flex; gap: 18px; }
	.footer-links a { color: var(--text-tertiary); font-size: 0.78rem; transition: color 0.15s; }
	.footer-links a:hover { color: var(--text); }

	/* ── Responsive ── */
	@media (max-width: 768px) {
		.hero { padding-top: 80px; padding-bottom: 24px; min-height: 100vh; }
		.hero-grid { grid-template-columns: 1fr; gap: 40px; }
		.hero-text { text-align: center; }
		.hero-cta { align-items: center; }
		.hero-visual { order: -1; }
		.scroll-hint { display: none; }
		h1 { font-size: 2rem; }
		h2 { font-size: 1.5rem; }
		.subtitle { font-size: 0.95rem; margin-left: auto; margin-right: auto; }
		.hero-visual { max-height: 400px; }
		.mockup-phone { width: min(400px, 85vw); }
		.trust-row { justify-content: center; }
		.trust-chip { font-size: 0.72rem; }
		.why-content { text-align: left; }
		.why-content h2 { font-size: 1.4rem; }
		.pillar { flex-direction: column; gap: 12px; }
		.pillar-number { align-self: flex-start; }
		.nav-links { display: none; }
		.subscribe-form { flex-direction: column; padding: 0 16px; }
		.subscribe-input, .subscribe-btn { width: 100%; }
		.footer-inner { flex-direction: column; gap: 14px; text-align: center; }
		.footer-left { flex-direction: column; gap: 6px; }
	}
</style>
