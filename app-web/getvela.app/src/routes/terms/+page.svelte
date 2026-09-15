<script lang="ts">
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Terms of Service — Vela Wallet</title>
	<meta name="description" content="Vela Wallet Terms of Service" />
</svelte:head>

<SiteHeader />

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
		<p>
			Creating a wallet publishes a <strong>public, permanent record</strong> to a smart contract on
			the Gnosis blockchain: the wallet name you choose, your passkey's public key, your
			authenticator's model (via its AAGUID) and WebAuthn flags, its credential id, and its
			transport hints. This record is readable by anyone, cannot be edited or deleted, and is
			required for cross-device sign-in and recovery — and, for a multi-passkey wallet, to
			reconstruct the wallet's address at all. See the <a href="/privacy">Privacy Policy</a> for the
			full list, and the <a href="/registry">registry</a> to browse exactly what is stored.
		</p>
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
			We provide backend services (Passkey Index, Chain Data Index, Relay) on a best-effort basis.
			These services may experience downtime, rate limiting, or discontinuation.
		</p>
		<p>
			All backend services are open source. You can self-host them to eliminate dependency on our
			infrastructure. See our <a
				href="https://github.com/mondaylabsltd/vela-wallet#self-deploy-service-endpoints"
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
				href="https://github.com/mondaylabsltd/vela-wallet/blob/main/LICENSE"
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
			<a href="https://github.com/mondaylabsltd/vela-wallet/issues" target="_blank" rel="noopener"
				>GitHub</a
			>.
		</p>
	</section>
</main>

<SiteFooter />

<style>
	/* Long-form reading page: cap the measure at prose width. The sticky
	   SiteHeader occupies its own flow space, so no fixed-nav offset needed. */
	main.container {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding: 48px 24px 80px;
	}

	h1 {
		font-size: 2rem;
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

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
