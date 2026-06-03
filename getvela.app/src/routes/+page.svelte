<script lang="ts">
	import { resolve } from '$app/paths';

	let email = $state('');
	let subscribeStatus: 'idle' | 'loading' | 'success' | 'error' = $state('idle');
	let subscribeMessage = $state('');

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
			<a href="#faq">FAQ</a>
			<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">GitHub</a>
		</div>
	</div>
</nav>

<!-- Hero -->
<section class="hero">
	<div class="container hero-grid">
		<div class="hero-text">
			<h1>No seed phrases.<br />No recovery keys.</h1>
			<p class="subtitle">
				Self-custodial, self-hostable wallet for ETH & EVM. Signed with passkeys. No vendor lock-in.
			</p>
			<div class="hero-cta">
				<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" class="btn btn-primary">Create wallet</a>
			</div>
		</div>
		<div class="hero-visual">
			<div class="mockup-phone">
				<div class="mockup-screen">
					<!-- Status bar + notch -->
					<div class="mockup-statusbar">
						<span class="mockup-time">9:41</span>
						<div class="mockup-notch"></div>
						<div class="mockup-statusbar-icons">
							<!-- Signal bars -->
							<svg width="12" height="10" viewBox="0 0 16 12" fill="currentColor"><rect x="0" y="8" width="3" height="4" rx="0.5" opacity="0.4"/><rect x="4.5" y="5" width="3" height="7" rx="0.5" opacity="0.6"/><rect x="9" y="2" width="3" height="10" rx="0.5" opacity="0.8"/><rect x="13.5" y="0" width="2.5" height="12" rx="0.5" opacity="1"/></svg>
							<!-- Battery -->
							<svg width="18" height="10" viewBox="0 0 25 10" fill="currentColor"><rect x="0" y="0" width="21" height="10" rx="2" stroke="currentColor" stroke-width="1" fill="none" opacity="0.4"/><rect x="22" y="2.5" width="2" height="5" rx="1" opacity="0.25"/><rect x="1.5" y="1.5" width="14" height="7" rx="1" opacity="0.6"/></svg>
						</div>
					</div>
					<!-- Account header -->
					<div class="mockup-account">
						<!-- <div class="mockup-avatar"></div> -->
						<div class="mockup-account-info">
							<span class="mockup-account-name">My Wallet</span>
							<span class="mockup-account-addr">0x14fB…D1eA5c</span>
						</div>
					</div>
					<!-- Balance -->
					<div class="mockup-balance">$1,969<span class="mockup-cents">.53</span></div>
					<!-- Action buttons -->
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
					<!-- Assets header -->
					<div class="mockup-assets-header">
						<span class="mockup-assets-title">Assets</span>
						<span class="mockup-assets-add">+ Add</span>
					</div>
					<!-- Token list -->
					<div class="mockup-tokens">
						<div class="mockup-token">
							<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_binance.jpg" alt="BNB" width="36" height="36" />
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
							<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_arbitrum.jpg" alt="ETH" width="36" height="36" />
							<div class="mockup-token-info">
								<span class="mockup-token-name">ETH</span>
								<span class="mockup-token-chain">Arbitrum</span>
							</div>
							<div class="mockup-token-value">
								<span class="mockup-token-qty">0.1997</span>
								<span class="mockup-token-usd">$457.69</span>
							</div>
						</div>
						<div class="mockup-token">
							<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_ethereum.jpg" alt="ETH" width="36" height="36" />
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
							<img class="mockup-token-icon" src="https://assets.coingecko.com/coins/images/325/small/Tether.png" alt="USDT" width="36" height="36" />
							<div class="mockup-token-info">
								<span class="mockup-token-name">USDT</span>
								<span class="mockup-token-chain">Polygon</span>
							</div>
							<div class="mockup-token-value">
								<span class="mockup-token-qty">178.5160</span>
								<span class="mockup-token-usd">$178.52</span>
							</div>
						</div>
						<div class="mockup-token last">
							<img class="mockup-token-icon" src="https://icons.llamao.fi/icons/chains/rsz_base.jpg" alt="ETH" width="36" height="36" />
							<div class="mockup-token-info">
								<span class="mockup-token-name">ETH</span>
								<span class="mockup-token-chain">Base</span>
							</div>
							<div class="mockup-token-value">
								<span class="mockup-token-qty">0.02406</span>
								<span class="mockup-token-usd">$54.86</span>
							</div>
						</div>
					</div>
					<!-- Tab bar -->
					<div class="mockup-tabbar">
						<div class="mockup-tab active">
							<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M21 12a2.25 2.25 0 00-2.25-2.25H15a3 3 0 11-6 0H5.25A2.25 2.25 0 003 12m18 0v6a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 18v-6m18 0V9M3 12V9m18 0a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 013 9m18 0V6a2.25 2.25 0 00-2.25-2.25H5.25A2.25 2.25 0 013 6v3" stroke-linecap="round" stroke-linejoin="round"/></svg>
							<span>Wallet</span>
						</div>
						<div class="mockup-tab">
							<svg width="18" height="18" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" stroke-linecap="round" stroke-linejoin="round"/><path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" stroke-linecap="round" stroke-linejoin="round"/></svg>
							<span>Settings</span>
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
		<p class="section-desc">Three things make Vela different from traditional wallets.</p>

		<div class="pillar">
			<div class="pillar-number">01</div>
			<div class="pillar-content">
				<h3>Passkeys replace seed phrases</h3>
				<p>
					When you create a Vela wallet, your device generates a passkey — a cryptographic credential
					stored in your OS secure enclave (the same chip that protects Face ID data).
					You sign transactions with your fingerprint or face. The private key never leaves your device.
				</p>
				<p>
					Lost your phone? Your passkey syncs automatically through iCloud Keychain or Google Password Manager.
					Sign in on a new device in seconds.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">02</div>
			<div class="pillar-content">
				<h3>Safe smart contracts, not custom code</h3>
				<p>
					Every Vela wallet is a <a href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" target="_blank" rel="noopener">Safe</a> smart contract account —
					the same audited v1.4.1 contracts that secure over $100B in assets across the ecosystem.
					We didn't write our own contract. We use the official, unmodified Safe with a WebAuthn signer module.
				</p>
				<p>
					This means your wallet exists on-chain, independent of Vela. If our servers go down, your funds are still yours.
					All three backend services are open source and self-hostable.
				</p>
			</div>
		</div>

		<div class="pillar">
			<div class="pillar-number">03</div>
			<div class="pillar-content">
				<h3>One wallet, 8+ chains</h3>
				<p>
					Same address across Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain, Avalanche, and Gnosis.
					Add any custom EVM network. Manage all your assets from a single portfolio view.
				</p>
				<div class="chain-row">
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-1.png" alt="Ethereum" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-8453.png" alt="Base" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-42161.png" alt="Arbitrum" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-10.png" alt="Optimism" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-137.png" alt="Polygon" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-56.png" alt="BNB Chain" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-43114.png" alt="Avalanche" width="24" height="24" />
					<img src="https://ethereum-data.awesometools.dev/chainlogos/eip155-100.png" alt="Gnosis" width="24" height="24" />
				</div>
			</div>
		</div>
	</div>
</section>

<!-- Security Model -->
<section class="security">
	<div class="container">
		<h2>What Vela can and cannot do</h2>
		<p class="section-desc">Transparency about our access model.</p>
		<div class="security-grid">
			<div class="security-col">
				<h4 class="security-heading-never">Vela never has access to</h4>
				<ul>
					<li>Your private key</li>
					<li>Your biometric data</li>
					<li>Your transaction contents</li>
					<li>Ability to freeze or move your funds</li>
				</ul>
			</div>
			<div class="security-col">
				<h4 class="security-heading-does">Vela stores</h4>
				<ul>
					<li>Your passkey's public key (on Gnosis blockchain)</li>
					<li>Your account display name</li>
					<li>Signed transactions you submit (relayed to the blockchain)</li>
				</ul>
			</div>
		</div>
		<p class="security-note">
			All data handling is described in our <a href={resolve('/privacy')}>privacy policy</a>.
			All source code is <a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">publicly auditable</a>.
		</p>
	</div>
</section>

<!-- FAQ -->
<section id="faq" class="faq">
	<div class="container">
		<h2>FAQ</h2>
		<p class="section-desc">Common questions about security, recovery, and fees.</p>
		<div class="faq-list">
			<details>
				<summary>How is Vela different from MetaMask?</summary>
				<p>
					MetaMask uses seed phrases (12-24 words) that you must write down and protect.
					If someone finds your seed phrase, they control your wallet. Vela uses passkeys —
					your private key lives in your device's secure enclave, protected by biometric authentication.
					Nothing to write down, nothing to leak.
				</p>
			</details>
			<details>
				<summary>What if I lose my phone?</summary>
				<p>
					Your passkey syncs through iCloud Keychain (iOS) or Google Password Manager (Android).
					Get a new phone, sign in with the same Apple/Google account, and your wallet is restored.
				</p>
			</details>
			<details>
				<summary>What if my iCloud or Google account is compromised?</summary>
				<p>
					An attacker with full access to your iCloud/Google account could potentially use your passkey.
					We strongly recommend enabling two-factor authentication and using a strong, unique password
					for your Apple/Google account.
				</p>
			</details>
			<details>
				<summary>What happens if Vela shuts down?</summary>
				<p>
					Your wallet is a smart contract on the blockchain — it doesn't depend on Vela's servers.
					All three backend services (chain data, passkey index, bundler) are open source and self-hostable.
					Your passkey still works regardless.
				</p>
			</details>
			<details>
				<summary>How do gas fees work?</summary>
				<p>
					Vela uses an ERC-4337 bundler to submit transactions. Each account requires a one-time gas deposit
					per network to activate the bundler. After that, each transaction is independently charged.
					Fee estimates are shown before you confirm. You can switch to any standard ERC-4337 bundler
					or <a href="https://github.com/atshelchin/vela-bundler" target="_blank" rel="noopener">self-host the Vela Bundler</a> to eliminate the service fee.
				</p>
			</details>
			<details>
				<summary>What if the getvela.app domain goes offline?</summary>
				<p>
					Your funds are safe — they live on-chain in your Smart Account, not on our servers.
					Passkeys are tied to the domain (rpId), but we provide an open-source Chrome extension
					that lets you use your existing passkeys from any other domain or localhost.
					See the <a href="https://github.com/atshelchin/vela-wallet#webauthn-proxy-extension-domain-recovery--dev-passkeys" target="_blank" rel="noopener">Domain Recovery guide</a> for setup instructions.
				</p>
			</details>
			<details>
				<summary>Is it free?</summary>
				<p>
					The web version is free. iOS and Android apps will have a one-time purchase price —
					no subscriptions, no in-app purchases. Since Vela is fully open source,
					you can also build and install it yourself for free.
				</p>
			</details>
		</div>
	</div>
</section>

<!-- CTA -->
<section id="notify" class="notify">
	<div class="container">
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
	.hero-cta { display: flex; align-items: center; gap: 12px; }

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
	/* Account header */
	.mockup-account { display: flex; flex-direction: column; align-items: center; gap: 4px; margin-bottom: 20px; }
	.mockup-account-info { display: flex; flex-direction: column; align-items: center; gap: 1px; }
	.mockup-account-name { font-size: 0.88rem; font-weight: 600; color: var(--text); }
	.mockup-account-addr { font-size: 0.65rem; color: var(--text-tertiary); font-family: monospace; }
	/* Balance */
	.mockup-balance {
		text-align: center; font-size: 2.2rem; font-weight: 700;
		color: var(--text); letter-spacing: -0.02em; margin-bottom: 20px;
		font-variant-numeric: tabular-nums;
	}
	.mockup-cents { font-size: 1.3rem; color: var(--text-secondary); }
	/* Action buttons */
	.mockup-actions { display: flex; justify-content: center; gap: 28px; margin-bottom: 22px; }
	.mockup-action { display: flex; flex-direction: column; align-items: center; gap: 5px; }
	.mockup-action span { font-size: 0.65rem; color: var(--text-secondary); }
	.mockup-action-circle {
		width: 42px; height: 42px; border-radius: 50%;
		border: 1.5px solid var(--border); background: transparent;
		display: flex; align-items: center; justify-content: center;
		color: var(--text-secondary);
	}
	.mockup-action-circle.active {
		background: var(--accent); border-color: var(--accent);
		color: #fff;
	}
	/* Assets header */
	.mockup-assets-header {
		display: flex; justify-content: space-between; align-items: center;
		margin-bottom: 8px; padding-bottom: 10px;
	}
	.mockup-assets-title { font-size: 0.92rem; font-weight: 600; color: var(--text); }
	.mockup-assets-add { font-size: 0.75rem; font-weight: 600; color: var(--accent); }
	/* Token list */
	.mockup-tokens { display: flex; flex-direction: column; }
	.mockup-token {
		display: flex; align-items: center; gap: 12px;
		padding: 11px 0;
		border-bottom: 1px solid var(--border);
	}
	.mockup-token.last { border-bottom: none; }
	.mockup-token-icon { width: 36px; height: 36px; border-radius: 50%; flex-shrink: 0; }

	.mockup-token-info { flex: 1; display: flex; flex-direction: column; gap: 2px; }
	.mockup-token-name { font-size: 0.82rem; font-weight: 600; color: var(--text); }
	.mockup-token-chain { font-size: 0.65rem; color: var(--text-tertiary); }
	.mockup-token-value { text-align: right; display: flex; flex-direction: column; gap: 2px; }
	.mockup-token-qty { font-size: 0.82rem; font-weight: 600; color: var(--text); font-variant-numeric: tabular-nums; }
	.mockup-token-usd { font-size: 0.65rem; color: var(--text-tertiary); font-variant-numeric: tabular-nums; }
	/* Tab bar */
	.mockup-tabbar {
		display: flex; justify-content: space-around; align-items: center;
		padding: 10px 0 14px; margin-top: 8px;
		border-top: 1px solid var(--border);
	}
	.mockup-tab {
		display: flex; flex-direction: column; align-items: center; gap: 3px;
		color: var(--text-tertiary); font-size: 0.6rem;
	}
	.mockup-tab.active { color: var(--accent); }

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
	.chain-row {
		display: flex; gap: 8px; align-items: center; margin-top: 16px;
	}
	.chain-row img { border-radius: 50%; opacity: 0.85; transition: opacity 0.15s; }
	.chain-row img:hover { opacity: 1; }

	/* ── Security ── */
	.security h2, .security .section-desc { text-align: center; }
	.security-grid {
		display: grid; grid-template-columns: 1fr 1fr; gap: 24px;
		max-width: 640px; margin: 0 auto;
	}
	.security-col {
		background: var(--bg-raised); border: 1px solid var(--border);
		border-radius: var(--radius); padding: 28px 24px;
	}
	.security-heading-never {
		font-size: 0.78rem; font-weight: 600; text-transform: uppercase;
		letter-spacing: 0.06em; margin-bottom: 14px; color: #C97070;
	}
	.security-heading-does {
		font-size: 0.78rem; font-weight: 600; text-transform: uppercase;
		letter-spacing: 0.06em; margin-bottom: 14px; color: var(--text-secondary);
	}
	.security-col ul { list-style: none; padding: 0; }
	.security-col li {
		font-size: 0.88rem; color: var(--text-secondary); line-height: 1.6;
		padding: 5px 0; border-bottom: 1px solid var(--border);
	}
	.security-col li:last-child { border-bottom: none; }
	.security-note {
		text-align: center; color: var(--text-tertiary); font-size: 0.82rem;
		margin-top: 24px; max-width: 640px; margin-left: auto; margin-right: auto;
	}
	.security-note a { color: var(--text-secondary); text-decoration: underline; text-underline-offset: 2px; }
	.security-note a:hover { color: var(--accent); }

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
		.hero-cta { justify-content: center; }
		.hero-visual { order: -1; }
		h1 { font-size: 2rem; }
		h2 { font-size: 1.5rem; }
		.subtitle { font-size: 0.95rem; margin-left: auto; margin-right: auto; }
		.hero-visual { max-height: 400px; }
		.mockup-phone { width: min(400px, 85vw); }
		.mockup-screen { padding: 16px 14px 0; }
		.mockup-balance { font-size: 1.7rem; }
		.mockup-cents { font-size: 1rem; }
		.mockup-token-icon { width: 30px; height: 30px; }
		.mockup-action-circle { width: 36px; height: 36px; }
		.mockup-actions { gap: 20px; }
		.trust-row { justify-content: center; }
		.trust-chip { font-size: 0.72rem; }
		.why-content { text-align: left; }
		.why-content h2 { font-size: 1.4rem; }
		.pillar { flex-direction: column; gap: 12px; }
		.pillar-number { align-self: flex-start; }
		.chain-row { flex-wrap: wrap; }
		.security-grid { grid-template-columns: 1fr; }
		.nav-links { display: none; }
		.subscribe-form { flex-direction: column; padding: 0 16px; }
		.subscribe-input, .subscribe-btn { width: 100%; }
		.footer-inner { flex-direction: column; gap: 14px; text-align: center; }
		.footer-left { flex-direction: column; gap: 6px; }
	}
</style>
