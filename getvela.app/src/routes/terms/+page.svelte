<script lang="ts">
	import { resolve } from '$app/paths';
</script>

<svelte:head>
	<title>Terms of Service — Vela Wallet</title>
	<meta name="description" content="Vela Wallet Terms of Service" />
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
	<h1>Terms of Service</h1>
	<p class="updated">Last updated: May 12, 2026</p>

	<section>
		<h2>1. Acceptance</h2>
		<p>
			Vela Wallet is operated by MONDAY LABS LTD, 61 Bridge Street, Kington, United Kingdom, HR5
			3DJ. By using Vela Wallet, you agree to these terms. If you do not agree, do not use the app.
		</p>
	</section>

	<section>
		<h2>2. Self-Custody</h2>
		<p>
			Vela Wallet is a self-custodial wallet. You are solely responsible for your wallet and its
			contents.
		</p>
		<ul>
			<li>
				Your passkey private key is managed by your device's operating system (iCloud Keychain or
				Google Password Manager). Vela Wallet cannot access, recover, or reset it.
			</li>
			<li>
				If you lose access to your iCloud or Google account, you may permanently lose access to your
				wallet. We cannot help you recover it.
			</li>
			<li>
				If your iCloud or Google account is compromised, your wallet control may also be
				compromised. Use a strong password and enable two-factor authentication.
			</li>
			<li>We cannot reverse, cancel, or modify any blockchain transaction once submitted.</li>
		</ul>
	</section>

	<section>
		<h2>3. No Financial Advice</h2>
		<p>
			Vela Wallet is a tool for managing cryptocurrency. We do not provide investment, financial,
			tax, or legal advice. You are responsible for your own financial decisions.
		</p>
	</section>

	<section>
		<h2>4. Service Availability</h2>
		<p>
			We provide backend services (Passkey Index, Chain Data Index, Bundler) on a best-effort basis.
			These services may experience downtime, rate limiting, or discontinuation.
		</p>
		<p>
			All backend services are open source. You can self-host them to eliminate dependency on our
			infrastructure. See our <a
				href="https://github.com/atshelchin/vela-wallet#self-deploy-service-endpoints"
				target="_blank"
				rel="noopener">self-hosting documentation</a
			>.
		</p>
	</section>

	<section>
		<h2>5. Blockchain Risks</h2>
		<p>
			You acknowledge the inherent risks of blockchain technology, including but not limited to:
		</p>
		<ul>
			<li>Token price volatility and potential total loss of value</li>
			<li>Smart contract vulnerabilities (including the Safe contracts that power your wallet)</li>
			<li>Network congestion, high gas fees, and failed transactions</li>
			<li>Regulatory changes that may affect your ability to use cryptocurrency</li>
		</ul>
	</section>

	<section>
		<h2>6. Open Source</h2>
		<p>
			Vela Wallet is distributed under the MIT License. The software is provided "as is", without
			warranty of any kind, express or implied. See the
			<a
				href="https://github.com/atshelchin/vela-wallet/blob/main/LICENSE"
				target="_blank"
				rel="noopener">full license</a
			>.
		</p>
	</section>

	<section>
		<h2>7. Limitation of Liability</h2>
		<p>
			To the maximum extent permitted by law, Vela Wallet and its contributors shall not be liable
			for any loss of funds, data, or profits arising from the use of this software. This includes
			losses due to software bugs, smart contract exploits, passkey compromise, or service
			unavailability.
		</p>
	</section>

	<section>
		<h2>8. Changes</h2>
		<p>
			We may update these terms from time to time. Continued use of Vela Wallet after changes
			constitutes acceptance of the updated terms.
		</p>
	</section>

	<section>
		<h2>9. Contact</h2>
		<p>
			Questions about these terms? Open an issue on
			<a href="https://github.com/atshelchin/vela-wallet/issues" target="_blank" rel="noopener"
				>GitHub</a
			>.
		</p>
	</section>
</main>

<footer>
	<div class="container footer-inner">
		<div class="footer-brand">
			<img src="/vela-logo.png" alt="Vela" width="32" height="32" />
			<span>Vela Wallet</span>
		</div>
		<div class="footer-links">
			<a href={resolve('/privacy')}>Privacy</a>
			<a href={resolve('/terms')}>Terms</a>
		</div>
		<p class="footer-copy">&copy; {new Date().getFullYear()} Vela Wallet. All rights reserved.</p>
	</div>
</footer>

<style>
	:root {
		--bg: #0f0e0c;
		--bg-card: #1e1d1b;
		--border: #2a2926;
		--text: #e8e6e1;
		--text-secondary: #9a9790;
		--accent: #e8572a;
		--max-w: 720px;
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
		max-width: 1080px;
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
		color: var(--accent);
		text-decoration: underline;
	}

	footer {
		padding: 40px 0;
		border-top: 1px solid var(--border);
	}
	.footer-inner {
		max-width: 1080px;
		margin: 0 auto;
		padding: 0 24px;
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.footer-brand {
		display: flex;
		align-items: center;
		gap: 10px;
		font-weight: 600;
	}
	.footer-brand img {
		border-radius: 6px;
	}
	.footer-links {
		display: flex;
		gap: 24px;
	}
	.footer-links a {
		color: var(--text-secondary);
		font-size: 0.85rem;
		text-decoration: none;
	}
	.footer-links a:hover {
		color: var(--text);
	}
	.footer-copy {
		color: var(--text-secondary);
		font-size: 0.85rem;
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
		.footer-inner {
			flex-direction: column;
			gap: 16px;
			text-align: center;
		}
	}
</style>
