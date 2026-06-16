<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
</script>

<svelte:head>
	<title>Roadmap — Vela Wallet</title>
	<meta
		name="description"
		content="Where Vela Wallet is headed — recently shipped, in progress, and what's next. Directions, not deadlines."
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
	<p class="updated">Directions, not deadlines · Last updated June 16, 2026</p>

	<p class="intro">
		Vela is in alpha and shipping constantly. This is where it's headed. We build in the open —
		every change lands in the
		<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener"
			>public repo</a
		>
		first. Plans below are directional and may change. If something matters to you,
		<a href="https://github.com/atshelchin/vela-wallet/issues" target="_blank" rel="noopener"
			>open an issue</a
		>.
	</p>

	<section>
		<div class="phase-head">
			<span class="pill pill-shipped">Recently shipped</span>
		</div>
		<ul class="shipped">
			<li>
				<strong>Clear Signing</strong> — transactions show what they actually do (amount, recipient, intent)
				in plain language instead of raw hex, powered by ERC-7730 descriptors.
			</li>
			<li>
				<strong>Payment-first home</strong> — the home screen rebuilt around your activity and balances.
			</li>
			<li>
				<strong>13-language app</strong> — instant language switching, with Russian and Italian most recently
				added.
			</li>
			<li>
				<strong>Local currency &amp; formatting</strong> — choose your fiat currency and a configurable
				rates source; amounts, dates, and numbers follow your locale.
			</li>
			<li>
				<strong>Pull-to-refresh (VelaRefresh)</strong> — a branded, gesture-driven refresh across the
				app.
			</li>
			<li>
				<strong>Clearer sends</strong> — transactions show a pending state until they are confirmed on-chain.
			</li>
			<li>
				<strong>DApp Connect maturity</strong> — inline pairing, batched calls (EIP-5792), and silent
				reconnect over WalletPair.
			</li>
			<li>
				<strong>One-tap feedback</strong> — a Feedback entry in Settings opens a prefilled GitHub report.
			</li>
		</ul>
	</section>

	<section>
		<div class="phase-head">
			<span class="pill pill-now">Now</span>
			<span class="phase-sub">Actively building</span>
		</div>
		<ul>
			<li>
				<strong>Wider clear-signing coverage</strong> — more contracts and chains rendered in plain language,
				fewer blind-sign fallbacks.
			</li>
			<li>
				<strong>Home &amp; activity polish</strong> — continued refinement of the payment-first layout.
			</li>
		</ul>
	</section>

	<section>
		<div class="phase-head">
			<span class="pill pill-next">Next</span>
			<span class="phase-sub">Planned</span>
		</div>
		<ul>
			<li>
				<strong>More recovery options</strong> — additional ways to recover an account beyond cloud-synced
				passkeys.
			</li>
			<li>
				<strong>DApp Connect on web</strong> — desktop pairing currently relies on mobile Bluetooth; bringing
				it to the web build.
			</li>
		</ul>
	</section>

	<section>
		<div class="phase-head">
			<span class="pill pill-later">Later</span>
			<span class="phase-sub">Exploring</span>
		</div>
		<ul>
			<li>
				<strong>Independent security audit</strong> — a third-party review of Vela's Safe + WebAuthn integration.
				The Safe contracts themselves are already audited and battle-tested.
			</li>
			<li><strong>More networks</strong> — additional EVM chains based on demand.</li>
		</ul>
	</section>

	<section class="cta-block">
		<h2>Want something on this list?</h2>
		<p>Vela is built around what early users actually need. Tell us what's missing.</p>
		<div class="cta-row">
			<a
				class="btn"
				href="https://github.com/atshelchin/vela-wallet/issues"
				target="_blank"
				rel="noopener">Open an issue</a
			>
			<a class="btn btn-ghost" href={resolve('/docs/whitepaper')}>Read the whitepaper</a>
		</div>
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
		--green: #2d8e5f;
		--blue: #5b8def;
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
		max-width: 760px;
		margin: 0 auto;
		padding: 120px 24px 80px;
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
	.intro a {
		color: var(--text);
		text-decoration: underline;
		text-underline-offset: 2px;
	}
	.intro a:hover {
		color: var(--accent);
	}

	section {
		margin-bottom: 40px;
	}
	.phase-head {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 16px;
	}
	.pill {
		display: inline-flex;
		align-items: center;
		padding: 4px 12px;
		border-radius: 999px;
		font-size: 0.78rem;
		font-weight: 600;
		letter-spacing: 0.01em;
		border: 1px solid var(--border);
	}
	.pill-shipped {
		color: var(--green);
		border-color: rgba(45, 142, 95, 0.4);
		background: rgba(45, 142, 95, 0.08);
	}
	.pill-now {
		color: var(--accent);
		border-color: rgba(232, 87, 42, 0.4);
		background: rgba(232, 87, 42, 0.08);
	}
	.pill-next {
		color: var(--blue);
		border-color: rgba(91, 141, 239, 0.4);
		background: rgba(91, 141, 239, 0.08);
	}
	.pill-later {
		color: var(--text-tertiary);
		background: var(--bg-raised);
	}
	.phase-sub {
		color: var(--text-tertiary);
		font-size: 0.85rem;
	}

	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	li {
		color: var(--text-secondary);
		font-size: 0.95rem;
		line-height: 1.7;
		padding-left: 22px;
		position: relative;
	}
	li strong {
		color: var(--text);
		font-weight: 600;
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
	.shipped li::before {
		background: var(--green);
	}

	.cta-block {
		margin-top: 56px;
		padding: 32px;
		border: 1px solid var(--border);
		border-radius: 16px;
		background: var(--bg-card);
		text-align: center;
	}
	.cta-block h2 {
		font-size: 1.2rem;
		font-weight: 600;
		margin-bottom: 8px;
	}
	.cta-block p {
		color: var(--text-secondary);
		font-size: 0.95rem;
		margin-bottom: 20px;
	}
	.cta-row {
		display: flex;
		gap: 12px;
		justify-content: center;
		flex-wrap: wrap;
	}
	.btn {
		display: inline-block;
		padding: 11px 22px;
		border-radius: 10px;
		font-size: 0.88rem;
		font-weight: 600;
		background: var(--accent);
		color: #fff;
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
