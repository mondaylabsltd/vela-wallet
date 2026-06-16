<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
</script>

<svelte:head>
	<title>Privacy Policy — Vela Wallet</title>
	<meta name="description" content="Vela Wallet Privacy Policy" />
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
	<h1>Privacy Policy</h1>
	<p class="updated">Last updated: June 11, 2026</p>

	<section>
		<h2>Overview</h2>
		<p>
			Vela Wallet is a self-custodial cryptocurrency wallet. We are committed to protecting your
			privacy. This policy explains what data we collect, why, and how it is handled.
		</p>
	</section>

	<section>
		<h2>What We Do NOT Collect</h2>
		<ul>
			<li>
				<strong>Private keys</strong> — Your passkey private key is managed entirely by your operating
				system (iCloud Keychain or Google Password Manager). Vela Wallet never has access to it.
			</li>
			<li>
				<strong>Seed phrases</strong> — Vela Wallet does not use seed phrases. There are none to collect.
			</li>
			<li>
				<strong>Transaction contents</strong> — We do not monitor, log, or analyze your on-chain transactions.
			</li>
			<li>
				<strong>Personal identity</strong> — We do not require your name, email, phone number, or any
				government ID to use the wallet.
			</li>
			<li>
				<strong>Email or contact details</strong> — The website has no signup, no newsletter, and no email
				capture. We have no way to email you, because we never ask for your address.
			</li>
		</ul>
	</section>

	<section>
		<h2>What We Store</h2>

		<h3>Passkey Public Key</h3>
		<p>
			When you create a wallet, your passkey's <strong>public key</strong> (not private key) is uploaded
			to our Passkey Index server. This enables cross-device sign-in — if you lose your device, you can
			recover your wallet on a new one by signing in with the same iCloud or Google account.
		</p>
		<p>
			The public key is stored on the Gnosis blockchain via a smart contract and is publicly
			readable. It cannot be used to sign transactions or move your funds.
		</p>

		<h3>Account Name</h3>
		<p>
			The account name you choose during wallet creation is stored alongside your public key on the
			Passkey Index server. This helps identify your wallet during cross-device sign-in. Choose a
			name that does not reveal your real identity if you prefer to remain pseudonymous.
		</p>

		<h3>On-Device Data</h3>
		<p>
			Wallet configuration, token balances, transaction history, and RPC endpoint preferences are
			stored locally on your device using AsyncStorage. This data is not transmitted to our servers.
		</p>
	</section>

	<section>
		<h2>The Website (getvela.app)</h2>
		<p>
			This policy also covers the getvela.app website, not just the wallet app. The website has no
			accounts, no newsletter, and no email capture.
		</p>
		<p>
			We use privacy-friendly, cookieless analytics to count page views and basic events (such as
			which sections are viewed or which links are clicked). It runs on our own self-hosted instance
			— your data is never sent to Google or any ad network. No cookies, no cross-site tracking, and
			no device fingerprinting. We do not build advertising profiles, and we do not sell or share
			this data.
		</p>
	</section>

	<section>
		<h2>Third-Party Services</h2>
		<p>Vela Wallet interacts with the following external services:</p>
		<ul>
			<li>
				<strong>Blockchain RPC nodes</strong> — To query balances and submit transactions. Your wallet
				address is visible to these providers.
			</li>
			<li>
				<strong>Vela Bundler</strong> — To relay ERC-4337 UserOperations to the blockchain. The bundler
				sees your signed transaction data but cannot modify it.
			</li>
			<li>
				<strong>Chain Data Index</strong> — To fetch token metadata and chain information. No wallet-specific
				data is sent.
			</li>
			<li>
				<strong>iCloud Keychain / Google Password Manager</strong> — Your passkey is synced by your operating
				system. Refer to Apple's or Google's privacy policies for details.
			</li>
		</ul>
	</section>

	<section>
		<h2>Data Retention</h2>
		<p>
			Public key records on the Passkey Index are stored indefinitely on the Gnosis blockchain.
			On-device data is deleted when you log out or uninstall the app. We do not maintain
			server-side backups of your local wallet data.
		</p>
	</section>

	<section>
		<h2>Open Source</h2>
		<p>
			Vela Wallet is fully open source. You can audit the code, verify our claims, and self-host all
			backend services. See <a
				href="https://github.com/atshelchin/vela-wallet"
				target="_blank"
				rel="noopener">our GitHub repository</a
			>.
		</p>
	</section>

	<section>
		<h2>Contact</h2>
		<p>
			If you have questions about this privacy policy, open an issue on
			<a href="https://github.com/atshelchin/vela-wallet/issues" target="_blank" rel="noopener"
				>GitHub</a
			>.
		</p>
		<p>
			Vela Wallet is operated by MONDAY LABS LTD, 61 Bridge Street, Kington, United Kingdom, HR5 3DJ
			— the data controller for this website.
		</p>
	</section>
</main>

<SiteFooter />

<style>
	:root {
		--bg: #0f0e0c;
		--bg-card: #1e1d1b;
		--border: #2a2926;
		--text: #e8e6e1;
		--text-secondary: #9a9790;
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
		/* max-width: 1080px; */
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
		max-width: var(--max-w);
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
		margin-bottom: 48px;
	}

	section {
		margin-bottom: 40px;
	}
	h2 {
		font-size: 1.25rem;
		font-weight: 600;
		margin-bottom: 12px;
		text-align: left;
	}
	h3 {
		font-size: 1rem;
		font-weight: 600;
		margin: 20px 0 8px;
		color: var(--accent);
	}

	p {
		color: var(--text-secondary);
		line-height: 1.75;
		font-size: 0.95rem;
		margin-bottom: 12px;
	}

	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	li {
		color: var(--text-secondary);
		font-size: 0.95rem;
		line-height: 1.7;
		padding-left: 20px;
		position: relative;
	}
	li::before {
		content: '';
		position: absolute;
		left: 0;
		top: 10px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent);
	}

	a {
		/* color: var(--accent); */
		/* text-decoration: underline; */
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
