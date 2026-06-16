<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
</script>

<svelte:head>
	<title>Vela is in alpha — what that means</title>
	<meta
		name="description"
		content="Vela is early software. Here's the honest version: what alpha means, why to start with small amounts, how self-custody and recovery work, and how to verify everything yourself."
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
	<div class="status-head">
		<span class="status-dot"></span>
		<span class="status-label">Alpha</span>
	</div>
	<h1>Vela is in alpha</h1>
	<p class="updated">What that means, and how to use it well · Last updated June 16, 2026</p>

	<p class="intro">
		Vela is early software. It works, it's used with real funds, and it improves almost every day —
		but it hasn't had the years of production hardening a mature wallet has. This is the honest
		version of where things stand. We'd rather you know exactly what you're stepping into than find
		out later.
	</p>

	<section>
		<h2>Start with small amounts</h2>
		<p>
			While Vela is in alpha, use amounts you'd be comfortable putting into any new piece of
			software. As the wallet matures and gets independently audited, that calculus changes — but
			for now, start small and scale up as your own confidence grows.
		</p>
	</section>

	<section>
		<h2>You hold your funds — and the responsibility</h2>
		<p>
			Vela is fully self-custodial. We never hold your keys and <strong
				>cannot move, freeze, or recover your funds</strong
			>. Your wallet is secured by a passkey on your device, backed up by iCloud Keychain or Google
			Password Manager.
		</p>
		<p>
			Keep that backup enabled. Today there is <strong>no social recovery</strong>: if you lose both
			your device and your cloud-synced passkey, and you have no other copy, the account cannot be
			recovered. More recovery options are on the <a href={resolve('/roadmap')}>roadmap</a>.
		</p>
	</section>

	<section>
		<h2>Not independently audited yet</h2>
		<p>
			Vela is built on <a
				href="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1"
				target="_blank"
				rel="noopener">Safe</a
			> smart accounts, which are independently audited and secure billions in value. Vela's own integration
			around them has not yet had a third-party audit — one is planned. Until then, treat Vela as alpha.
		</p>
	</section>

	<section>
		<h2>Don't trust us — verify</h2>
		<p>
			Vela is fully open source under the MIT license — the app and all three backend services. The
			best thing you can do with early software is check it yourself: read the code, run the
			services on your own infrastructure, and tell us what's wrong. Bug reports and pull requests
			genuinely shape what ships next.
		</p>
		<div class="cta-row">
			<a class="btn" href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener"
				>Read the code</a
			>
			<a
				class="btn btn-ghost"
				href="https://github.com/atshelchin/vela-wallet/issues"
				target="_blank"
				rel="noopener">Report an issue</a
			>
		</div>
	</section>

	<section>
		<h2>A few practical notes</h2>
		<ul>
			<li>
				<strong>No token, nothing to buy.</strong> Vela has no token. Gas is paid from your own balance
				in each network's native asset.
			</li>
			<li>
				<strong>One-time relayer deposit.</strong> Your first transaction on each network requires a small,
				one-time, non-refundable deposit to fund the gas relayer.
			</li>
			<li>
				<strong>Things will change.</strong> Interfaces, the relayer model, and recovery mechanisms are
				still evolving during alpha.
			</li>
		</ul>
		<p class="more">
			Want the full technical picture, including the security and recovery model in depth? Read the
			<a href={resolve('/docs/whitepaper')}>whitepaper</a>.
		</p>
	</section>
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
		--max-w: 1200px;
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
		padding: 120px 24px 80px;
	}

	.status-head {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 5px 13px;
		border-radius: 999px;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		margin-bottom: 20px;
	}
	.status-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--accent);
		box-shadow: 0 0 7px rgba(232, 87, 42, 0.6);
		animation: pulse 2s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.35;
		}
	}
	.status-label {
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text);
	}

	h1 {
		font-size: 2rem;
		font-weight: 700;
		margin-bottom: 8px;
		letter-spacing: -0.02em;
	}
	.updated {
		color: var(--text-secondary);
		font-size: 0.85rem;
		margin-bottom: 28px;
	}
	.intro {
		color: var(--text-secondary);
		line-height: 1.75;
		font-size: 0.98rem;
		margin-bottom: 48px;
		padding-bottom: 32px;
		border-bottom: 1px solid var(--border);
	}

	section {
		margin-bottom: 40px;
	}
	h2 {
		font-size: 1.2rem;
		font-weight: 600;
		margin-bottom: 12px;
		letter-spacing: -0.01em;
	}
	p {
		color: var(--text-secondary);
		line-height: 1.75;
		font-size: 0.95rem;
		margin-bottom: 12px;
	}
	strong {
		color: var(--text);
		font-weight: 600;
	}
	a {
		color: var(--accent);
		text-decoration: underline;
		text-underline-offset: 2px;
	}

	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 16px;
	}
	li {
		color: var(--text-secondary);
		font-size: 0.95rem;
		line-height: 1.7;
		padding-left: 22px;
		position: relative;
	}
	li::before {
		content: '';
		position: absolute;
		left: 0;
		top: 9px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent);
	}
	.more {
		font-size: 0.9rem;
	}

	.cta-row {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
		margin-top: 20px;
	}
	.btn {
		display: inline-block;
		padding: 11px 22px;
		border-radius: 10px;
		font-size: 0.88rem;
		font-weight: 600;
		background: var(--accent);
		color: #fff;
		text-decoration: none;
		transition: all 0.15s;
	}
	.btn:hover {
		transform: translateY(-1px);
		box-shadow: 0 4px 16px rgba(232, 87, 42, 0.3);
	}
	.btn-ghost {
		background: transparent;
		color: var(--text);
		border: 1px solid var(--border);
	}
	.btn-ghost:hover {
		border-color: var(--text-tertiary);
		box-shadow: none;
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
