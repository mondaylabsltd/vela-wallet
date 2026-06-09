<script lang="ts">
	import { resolve } from '$app/paths';

	// Analytics helper
	interface RybbitWindow extends Window { rybbit?: { event: (name: string, props?: Record<string, string | number>) => void } }
	function track(event: string, props?: Record<string, string | number>) {
		try { (globalThis as unknown as RybbitWindow).rybbit?.event(event, props); } catch { /* noop */ }
	}

	let email = $state('');
	let subscribeStatus: 'idle' | 'loading' | 'success' | 'error' = $state('idle');
	let subscribeMessage = $state('');

	// Mockup animation: wallet → amount → confirm → faceid → done
	type MockupStep = 'wallet' | 'amount' | 'confirm' | 'faceid' | 'done';
	const stepTimings: [MockupStep, number][] = [
		['wallet', 4500],
		['amount', 3000],
		['confirm', 3000],
		['faceid', 2500],
		['done', 2500],
	];
	let mockupStep: MockupStep = $state('wallet');
	let sendTapped = $state(false);
	let stepIndex = 0;
	let amountText = $state('');

	$effect(() => {
		function typeAmount(full: string, i: number) {
			if (i <= full.length) {
				amountText = full.slice(0, i);
				setTimeout(() => typeAmount(full, i + 1), 180);
			}
		}

		function nextStep() {
			stepIndex = (stepIndex + 1) % stepTimings.length;
			const step = stepTimings[stepIndex][0];
			mockupStep = step;
			sendTapped = false;
			if (step === 'wallet') {
				setTimeout(() => { sendTapped = true; }, 3500);
			}
			if (step === 'amount') {
				amountText = '';
				setTimeout(() => typeAmount('0.05', 0), 400);
			}
			setTimeout(nextStep, stepTimings[stepIndex][1]);
		}
		const firstTimeout = setTimeout(nextStep, stepTimings[0][1]);

		// Section visibility tracking
		const seen: Record<string, boolean> = {};
		const observer = new IntersectionObserver((entries) => {
			for (const entry of entries) {
				const id = entry.target.id;
				if (entry.isIntersecting && id && !seen[id]) {
					seen[id] = true;
					track('section_viewed', { section: id });
				}
			}
		}, { threshold: 0.3 });
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
			clearTimeout(firstTimeout);
			observer.disconnect();
			faqList?.removeEventListener('toggle', onFaqToggle, true);
		};
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
				track('email_subscribe');
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
	<title>Vela Wallet — Your keys. Your face.</title>
	<meta name="description" content="Self-custodial passkey wallet for ETH & EVM. Sign transactions with Face ID — no seed phrases, no hardware wallets. Open source and self-hostable." />
	<meta property="og:title" content="Vela Wallet — Your keys. Your face." />
	<meta property="og:description" content="Self-custodial passkey wallet for ETH & EVM. Sign with Face ID — no seed phrases. Open source and self-hostable." />
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
			<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" data-rybbit-event="cta_click" data-rybbit-prop-location="nav">Create wallet</a>
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
				<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" class="btn btn-primary" data-rybbit-event="cta_click" data-rybbit-prop-location="hero">Try it - no seed phrase needed</a>
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
								<div class="mockup-action-circle active" class:tapping={sendTapped}>
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

					<!-- Step 3: Enter amount -->
					<div class="mockup-step" class:active={mockupStep === 'amount'}>
						<div class="mockup-step-header">Send ETH</div>
						<div class="mockup-send-form">
							<div class="mockup-token-select">
								<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_ethereum.jpg" alt="ETH" width="24" height="24" />
								<span class="mockup-token-select-name">ETH</span>
								<span class="mockup-token-select-chain">Ethereum</span>
								<svg class="mockup-chevron" width="12" height="12" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M19.5 8.25l-7.5 7.5-7.5-7.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
							</div>
							<div class="mockup-amount-input">
								<span class="mockup-amount-value">{amountText}<span class="mockup-cursor">&nbsp;</span></span>
								<span class="mockup-amount-unit">ETH</span>
							</div>
							<div class="mockup-amount-usd">≈ $114.50</div>
							<div class="mockup-to-field">
								<span class="mockup-to-label">To</span>
								<span class="mockup-to-addr">0x7a3B…9f2E</span>
							</div>
						</div>
						<div class="mockup-send-btn">Continue</div>
					</div>

					<!-- Step 4: Confirm transaction -->
					<div class="mockup-step" class:active={mockupStep === 'confirm'}>
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
						<div class="mockup-slide-track" class:sliding={mockupStep === 'confirm'}>
							<div class="mockup-slide-thumb">
								<svg width="16" height="16" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" stroke-linecap="round" stroke-linejoin="round"/></svg>
							</div>
							<span class="mockup-slide-text">Slide to confirm</span>
						</div>
					</div>

					<!-- Step 5: Face ID signing -->
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

					<!-- Step 6: Success -->
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

					<!-- Progress dots -->
					<div class="mockup-dots">
						<span class="mockup-dot" class:active={mockupStep === 'wallet'}></span>
						<span class="mockup-dot" class:active={mockupStep === 'amount'}></span>
						<span class="mockup-dot" class:active={mockupStep === 'confirm'}></span>
						<span class="mockup-dot" class:active={mockupStep === 'faceid'}></span>
						<span class="mockup-dot" class:active={mockupStep === 'done'}></span>
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
				Seed phrases are hard to live with. Not "I got hacked" hard — quietly, constantly hard.
				Where do you keep 12 words so they survive a house fire, a stolen phone, and your own forgetfulness?
				Most people end up with a screenshot in their camera roll. That's not security. That's a liability.
			</p>
			<p>
				Passkey wallets fix this. We used <a href="https://account.base.app" target="_blank" rel="noopener">Base Account</a> daily — loved the UX, loved signing with Face ID.
				But we kept hitting walls: a recovery key generated in the browser that you just had to trust.
				No custom networks. No self-hosting.
				And if the service shuts down, your wallet goes with it.
			</p>
			<p>
				So we built Vela — <strong>a passkey wallet you can fully own.</strong>
				Your passkey lives in iCloud Keychain or Google Password Manager.
				When you sign a transaction, the app sends a challenge to your device — your device signs it and sends back just the signature. The private key never leaves.
				Most wallets have a moment where your secret is exposed: a seed phrase in memory, a recovery key in a browser tab.
				Vela has no such moment. <strong>We can't access your keys. Not "we promise not to" — we architecturally can't.</strong>
			</p>
			<p>
				Open source, so you can verify every line.
				Self-hostable, so you're never dependent on us.
				Built on unmodified <a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe contracts</a> — the same ones that already secure billions across the ecosystem.
			</p>
			<p>
				<strong>The trade-off:</strong> your wallet security depends on your Apple or Google account, because that's where your passkey lives.
				Lose access to that account, or delete the passkey, and there's no way back — no backup seed, no support ticket, no reset.
				But every self-custodial wallet has this trade-off. The question is what you're guarding.
				A seed phrase can be copied, screenshotted, and phished.
				A passkey is bound to a domain — there's no secret to type, no words to reveal, and no fake site that can trick you into handing it over.
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
					Authenticate with Face ID or fingerprint. Your device creates a passkey
					and derives a <a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe</a> smart account address from it —
					one address across all supported chains.
					No gas cost upfront. The contract deploys on-chain with your first transaction.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">02</div>
			<div class="pillar-content">
				<h3>Sign a transaction</h3>
				<p>
					The app builds a transaction and sends a signing challenge to your device.
					Your device signs it with the passkey and sends back just the signature — the private key never leaves.
					The signed transaction goes on-chain through an <a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener">ERC-4337</a> bundler.
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
			<h2>How Vela makes money</h2>
			<p class="bm-intro">
				No VC funding, no token, no ads, no data sales. Vela is funded by the people who use it.
				Here's exactly how.
			</p>

			<div class="bm-grid">
				<div class="bm-card">
					<h4>Web wallet</h4>
					<div class="bm-price">Free</div>
					<p>Full-featured web wallet. No time limit, no feature gates.</p>
				</div>
				<div class="bm-card">
					<h4>Mobile app</h4>
					<div class="bm-price">Paid download</div>
					<p>
						iOS & Android via App Store and Google Play.
						Priced by region so it's accessible worldwide.
						No subscriptions. No in-app purchases.
					</p>
				</div>
				<div class="bm-card">
					<h4>Bundler gas fee</h4>
					<div class="bm-price">Network gas + service fee</div>
					<p>
						Transactions go through an ERC-4337 bundler. You pay network gas plus a small service fee.
						You can skip the fee entirely by running a compatible
						<a href="https://github.com/atshelchin/vela-bundler" target="_blank" rel="noopener">self-hosted bundler</a>.
					</p>
				</div>
			</div>

			<p class="bm-note">
				You're paying for convenience, not access. Everything is open source and self-hostable — nothing locks you in.
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
					Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, and Gnosis — plus any custom EVM network you add yourself.
					Same wallet address across all chains.
				</p>
			</details>
			<details>
				<summary>How is Vela different from Coinbase Smart Wallet or other passkey wallets?</summary>
				<p>
					Most passkey wallets are closed-source and run on infrastructure you can't control.
					If the company pivots or shuts down, you're stuck.
					Vela is fully open source and self-hostable — the app, the bundler, and all backend services.
					You can add custom networks, run your own bundler to skip fees, and keep using your wallet even if getvela.app disappears.
					No recovery keys generated in a browser. No vendor lock-in.
				</p>
			</details>
			<details>
				<summary>Can I use Vela with dApps?</summary>
				<p>
					Yes. Pair your phone with the <a href="https://walletpair.org/" target="_blank" rel="noopener">WalletPair extension</a> and sign transactions on desktop dApps using your phone's passkey.
				</p>
			</details>
			<details>
				<summary>Do I pay more gas than a regular wallet?</summary>
				<p>
					Yes. Smart account transactions have extra overhead from on-chain signature verification and the ERC-4337 EntryPoint.
					Expect roughly 1.5–3x the gas of a standard wallet transfer, depending on the chain.
					That's the cost of passkey signing, no seed phrase, and one address across all chains.
				</p>
			</details>
			<!-- Security & recovery -->
			<details>
				<summary>What if I lose my phone?</summary>
				<p>
					Your passkey syncs through iCloud Keychain (iOS) or Google Password Manager (Android).
					Get a new phone, sign in with the same Apple/Google account, and your wallet is right there.
				</p>
			</details>
			<details>
				<summary>What if I accidentally delete my passkey?</summary>
				<p>
					It's gone — and so is access to your wallet.
					There's no recovery mechanism. This is irreversible.
					If you ever clean up your password manager, know what each passkey is for before you remove it.
				</p>
			</details>
			<details>
				<summary>What if my Apple or Google account is compromised?</summary>
				<p>
					Anyone who can access your Apple/Google account and use your passkey could access your wallet.
					Enable two-factor authentication and use a strong, unique password — your Apple/Google account is part of your wallet security.
				</p>
			</details>
			<details>
				<summary>Can I add a second passkey as backup?</summary>
				<p>
					Not right now. Each wallet is bound to a single passkey — a design choice in the current signer module.
					Your backup is the built-in sync: iCloud Keychain or Google Password Manager replicates the passkey across all your trusted devices automatically.
				</p>
			</details>
			<!-- Trust & transparency -->
			<details>
				<summary>What happens if Vela shuts down?</summary>
				<p>
					Your wallet is a Safe smart contract on-chain — it doesn't depend on Vela's servers.
					All backend services (chain data, passkey index, bundler) are open source and self-hostable.
					Your passkey still works, and you can interact with your wallet through any Safe-compatible interface.
				</p>
			</details>
			<details>
				<summary>What if the getvela.app domain goes offline?</summary>
				<p>
					Your funds stay on-chain regardless.
					Since passkeys are tied to a domain, Vela provides an open-source
					<a href="https://github.com/atshelchin/vela-wallet#webauthn-proxy-extension-domain-recovery--dev-passkeys" target="_blank" rel="noopener">recovery extension</a>
					that lets you use your existing passkey from another domain or localhost.
				</p>
			</details>
			<details>
				<summary>Has the code been audited?</summary>
				<p>
					The Safe contracts and Safe WebAuthn signer module that Vela uses have been audited.
					Vela's own app code hasn't been independently audited yet — all source code is <a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">public</a> for review.
				</p>
			</details>
		</div>
	</div>
</section>

<!-- CTA -->
<section id="notify" class="notify">
	<div class="container">
		<h2>Ready to try it?</h2>
		<p class="notify-sub">The web wallet is live and free. No install, no seed phrase — just authenticate and go.</p>
		<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" class="btn btn-primary btn-cta-main" data-rybbit-event="cta_click" data-rybbit-prop-location="bottom">Create a wallet</a>

		<div class="notify-divider"><span>mobile apps coming soon</span></div>

		<p class="notify-email-desc">Leave your email and we'll let you know when iOS & Android launch.</p>

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
					{subscribeStatus === 'loading' ? 'Subscribing...' : 'Notify me'}
				</button>
			</form>
			{#if subscribeStatus === 'error'}
				<p class="subscribe-error">{subscribeMessage}</p>
			{/if}
		{/if}

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
		opacity: 0;
	}
	.mockup-step.active { display: flex; animation: step-fade 0.25s ease forwards; }
	@keyframes step-fade { from { opacity: 0; } to { opacity: 1; } }

	/* Progress dots */
	.mockup-dots {
		display: flex; justify-content: center; gap: 6px; padding: 8px 0 4px;
	}
	.mockup-dot {
		width: 5px; height: 5px; border-radius: 50%;
		background: var(--border); transition: all 0.3s ease;
	}
	.mockup-dot.active { background: var(--accent); width: 16px; border-radius: 3px; }

	/* Step 2: Tap Send — pulse ring */
	.mockup-action-circle.tapping {
		box-shadow: 0 0 0 6px rgba(232, 87, 42, 0.35);
		transform: scale(0.93);
		transition: all 0.2s ease;
	}

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

	/* Step 3: Enter amount */
	.mockup-send-form {
		display: flex; flex-direction: column; gap: 16px; margin-bottom: 20px;
	}
	.mockup-token-select {
		display: flex; align-items: center; gap: 8px;
		padding: 10px 14px; border-radius: 12px;
		background: var(--bg-raised); border: 1px solid var(--border-accent);
	}
	.mockup-token-select-name { font-size: 0.82rem; font-weight: 600; color: var(--text); }
	.mockup-token-select-chain { font-size: 0.65rem; color: var(--text-tertiary); flex: 1; }
	.mockup-chevron { color: var(--text-tertiary); }
	.mockup-amount-input {
		display: flex; align-items: baseline; justify-content: center; gap: 6px;
		padding: 12px 0;
	}
	.mockup-amount-value {
		font-size: 2rem; font-weight: 700; color: var(--text);
		font-variant-numeric: tabular-nums; min-height: 2.4rem;
	}
	.mockup-cursor {
		border-right: 2px solid var(--accent);
		margin-left: 1px;
		animation: cursor-breathe 1.2s ease-in-out infinite;
	}
	@keyframes cursor-breathe { 0%, 100% { opacity: 0.3; } 50% { opacity: 1; } }
	.mockup-amount-unit { font-size: 0.88rem; color: var(--text-tertiary); font-weight: 500; }
	.mockup-amount-usd { text-align: center; font-size: 0.75rem; color: var(--text-tertiary); margin-top: -10px; }
	.mockup-to-field {
		display: flex; justify-content: space-between; align-items: center;
		padding: 10px 14px; border-radius: 12px;
		background: var(--bg-raised); border: 1px solid var(--border);
	}
	.mockup-to-label { font-size: 0.75rem; color: var(--text-tertiary); }
	.mockup-to-addr { font-size: 0.75rem; color: var(--text); font-family: monospace; }
	.mockup-send-btn {
		text-align: center; padding: 12px; border-radius: 12px;
		background: var(--accent); color: #fff;
		font-size: 0.85rem; font-weight: 600;
	}

	/* Step 4: Confirm tx */
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
		transition: background 0.3s ease, border-color 0.3s ease;
	}
	.mockup-slide-thumb {
		position: absolute; left: 4px; top: 4px;
		width: 36px; height: 36px; border-radius: 50%;
		background: var(--accent); color: #fff;
		display: flex; align-items: center; justify-content: center;
	}
	/* Only animate when confirm step is active */
	.mockup-slide-track.sliding .mockup-slide-thumb {
		animation: slide-complete 2.2s ease-in-out forwards;
	}
	.mockup-slide-track.sliding {
		animation: slide-track-done 2.2s ease-in-out forwards;
	}
	@keyframes slide-complete {
		0% { left: 4px; }
		70% { left: calc(100% - 40px); }
		100% { left: calc(100% - 40px); }
	}
	@keyframes slide-track-done {
		0%, 69% { background: var(--bg-raised); border-color: var(--border); }
		70%, 100% { background: var(--green-soft); border-color: rgba(45, 142, 95, 0.3); }
	}
	.mockup-slide-text { font-size: 0.72rem; color: var(--text-tertiary); font-weight: 500; transition: opacity 0.3s; }
	.mockup-slide-track.sliding .mockup-slide-text {
		animation: slide-text-fade 2.2s ease forwards;
	}
	@keyframes slide-text-fade {
		0%, 60% { opacity: 1; }
		70%, 100% { opacity: 0; }
	}

	/* Step 5: Face ID */
	.mockup-faceid-screen {
		display: flex; flex-direction: column; align-items: center;
		justify-content: center; gap: 16px;
		padding: 60px 0 40px;
	}
	.mockup-faceid-icon {
		color: var(--accent);
		animation: faceid-scan 2s ease forwards;
	}
	@keyframes faceid-scan {
		0% { opacity: 0.5; transform: scale(0.9); }
		30% { opacity: 1; transform: scale(1); }
		60% { opacity: 1; transform: scale(1); filter: brightness(1.3); }
		100% { opacity: 1; transform: scale(1); filter: brightness(1); }
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
	/* Notify social links */
	.notify-sub { color: var(--text-secondary); font-size: 0.95rem; margin-bottom: 28px; }
	.notify-email-desc { color: var(--text-tertiary); font-size: 0.82rem; margin-bottom: 16px; }

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
