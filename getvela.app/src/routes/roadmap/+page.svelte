<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';

	type Status = 'now' | 'next' | 'later';
	const upcoming: { status: Status; label: string; title: string; body: string }[] = [
		{
			status: 'now',
			label: 'In progress',
			title: 'See every coin you receive',
			body: 'A plain native-coin deposit — or coins that arrive through an internal call (an exchange withdrawal, a router, a multisig) — emits no on-chain log, so it can’t show in your activity on most networks today. We’re building a transfer service that traces blocks to surface these, so every deposit appears, on every chain.'
		},
		{
			status: 'now',
			label: 'In progress',
			title: 'Wider clear-signing coverage',
			body: 'More contracts and chains shown as human-readable intent, so fewer transactions fall back to blind signing.'
		},
		{
			status: 'next',
			label: 'Next',
			title: 'Native iOS & Android apps',
			body: 'Vela runs on the web today; the mobile builds share the same code and are in real-device testing ahead of an App Store and Google Play release.'
		},
		{
			status: 'next',
			label: 'Next',
			title: 'Sync across all your devices',
			body: 'Your accounts and networks already follow you. Next: your language, currency and formatting, plus one-tap restore of your whole setup on a new device — and a saved address book so you stop re-pasting addresses.'
		},
		{
			status: 'later',
			label: 'Exploring',
			title: 'Reach further',
			body: 'DApp Connect from the desktop without your phone, more EVM networks (including a signing path for chains without the P-256 precompile), and an independent security audit of Vela’s Safe + WebAuthn integration.'
		}
	];

	const shipped: { date: string; title: string; body: string }[] = [
		{
			date: 'Jun 2026',
			title: 'Localization & everyday polish',
			body: '13-language support with instant switching (Russian and Italian added), local currency and locale-aware formatting, a dynamic amount display, branded pull-to-refresh, pending-until-confirmed sends, and one-tap in-app feedback.'
		},
		{
			date: 'Jun 13, 2026',
			title: 'Payment-first home',
			body: 'The home screen rebuilt around your activity and balances.'
		},
		{
			date: 'Jun 9, 2026',
			title: 'Clear Signing (ERC-7730)',
			body: 'Transactions show what they actually do — amount, recipient, intent — in plain language instead of raw hex, with a preview harness and tests.'
		},
		{
			date: 'Jun 4, 2026',
			title: 'WalletPair dApp connect',
			body: 'End-to-end-encrypted pairing so you can sign for desktop dApps from your wallet.'
		},
		{
			date: 'May 28, 2026',
			title: 'dApp signing flow',
			body: 'Connection infrastructure and the signing-request experience.'
		},
		{
			date: 'May 2026',
			title: 'The core wallet experience',
			body: 'A real design system, gas-tier selection and a redesigned confirm screen, a fullscreen QR scanner, and rebuilt receive, token, add-token and deposit screens.'
		},
		{
			date: 'Apr 22, 2026',
			title: 'Vela is born',
			body: 'The wallet launches on iOS, Android and Web — Safe smart accounts (ERC-4337), passkey sign-in, and no seed phrase, from day one.'
		}
	];
</script>

<svelte:head>
	<title>Roadmap — Vela Wallet</title>
	<meta
		name="description"
		content="What Vela has shipped since April 2026 and what's coming next — built in the open. Directions, not deadlines."
	/>
</svelte:head>

<nav>
	<div class="nav-inner">
		<a href={resolve('/')} class="logo">
			<img src="/vela-logo.png" alt="Vela Wallet" width="36" height="36" />
			<span>Vela Wallet</span>
		</a>
	</div>
</nav>

<main class="container">
	<h1>Roadmap</h1>
	<p class="lede">
		Vela has shipped continuously since April 2026, in the open. Here's the trail so far and where
		it's headed — directions, not deadlines. Want something on it?
		<a href="https://github.com/mondaylabsltd/vela-wallet/issues" target="_blank" rel="noopener"
			>Open an issue</a
		>.
	</p>

	<h2 class="phase-title">Up next</h2>
	<ol class="track">
		{#each upcoming as item (item.title)}
			<li class="node {item.status}">
				<span class="badge {item.status}">{item.label}</span>
				<h3>{item.title}</h3>
				<p>{item.body}</p>
			</li>
		{/each}
	</ol>

	<h2 class="phase-title shipped-title">Shipped</h2>
	<ol class="track">
		{#each shipped as item (item.title)}
			<li class="node ship">
				<span class="when">{item.date}</span>
				<h3>{item.title}</h3>
				<p>{item.body}</p>
			</li>
		{/each}
	</ol>
</main>

<SiteFooter />

<style>
	:root {
		--bg: #0f0e0c;
		--bg-raised: #1a1918;
		--bg-card: #1e1d1b;
		--border: #2a2926;
		--text: #e8e6e1;
		--text-secondary: #9a9790;
		--text-tertiary: #6b6862;
		--accent: #e8572a;
		--green: #2d8e5f;
		--max-w: 1400px;
	}

	nav {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 100;
		background: rgba(15, 14, 12, 0.92);
		backdrop-filter: blur(16px);
		border-bottom: 1px solid var(--border);
	}
	.nav-inner {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 0 24px;
		height: 64px;
		display: flex;
		align-items: center;
	}
	.logo {
		display: flex;
		align-items: center;
		gap: 10px;
		font-weight: 600;
		font-size: 1.1rem;
	}
	.logo img {
		border-radius: 8px;
	}

	main.container {
		max-width: 720px;
		margin: 0 auto;
		padding: 120px 24px 72px;
	}

	h1 {
		font-size: 2rem;
		font-weight: 700;
		margin-bottom: 12px;
		letter-spacing: -0.02em;
	}
	.lede {
		color: var(--text-secondary);
		line-height: 1.7;
		font-size: 0.98rem;
		margin-bottom: 44px;
	}
	.lede a {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 2px;
	}
	.lede a:hover {
		color: var(--accent);
	}

	.phase-title {
		font-size: 0.78rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
		margin-bottom: 24px;
	}
	.shipped-title {
		margin-top: 16px;
	}

	.track {
		list-style: none;
		position: relative;
		margin: 0 0 48px;
		padding: 0;
	}
	.track::before {
		content: '';
		position: absolute;
		left: 6px;
		top: 8px;
		bottom: 8px;
		width: 2px;
		background: var(--border);
	}
	.node {
		position: relative;
		padding: 0 0 30px 34px;
	}
	.node:last-child {
		padding-bottom: 0;
	}
	.node::before {
		content: '';
		position: absolute;
		left: 0;
		top: 3px;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--bg);
		border: 2px solid var(--border);
		box-sizing: border-box;
	}
	.node.ship::before {
		background: var(--green);
		border-color: var(--green);
	}
	.node.now::before {
		background: var(--accent);
		border-color: var(--accent);
		box-shadow: 0 0 0 4px rgba(232, 87, 42, 0.15);
		animation: pulse 2s ease-in-out infinite;
	}
	.node.next::before {
		background: var(--bg);
		border-color: var(--accent);
	}
	.node.later::before {
		background: var(--bg);
		border-color: var(--text-tertiary);
	}
	@keyframes pulse {
		0%,
		100% {
			box-shadow: 0 0 0 4px rgba(232, 87, 42, 0.15);
		}
		50% {
			box-shadow: 0 0 0 7px rgba(232, 87, 42, 0.05);
		}
	}

	.node h3 {
		font-size: 1.05rem;
		font-weight: 600;
		color: var(--text);
		margin: 6px 0 6px;
		letter-spacing: -0.01em;
	}
	.node p {
		color: var(--text-secondary);
		font-size: 0.93rem;
		line-height: 1.65;
	}

	.badge {
		display: inline-block;
		font-size: 0.68rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		padding: 2px 9px;
		border-radius: 999px;
		border: 1px solid var(--border);
	}
	.badge.now {
		color: var(--accent);
		border-color: rgba(232, 87, 42, 0.4);
		background: rgba(232, 87, 42, 0.08);
	}
	.badge.next {
		color: var(--text-secondary);
	}
	.badge.later {
		color: var(--text-tertiary);
	}
	.when {
		font-size: 0.78rem;
		font-weight: 600;
		color: var(--text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
