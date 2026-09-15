<script lang="ts">
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Privacy Policy — Vela Wallet</title>
	<meta name="description" content="Vela Wallet Privacy Policy" />
</svelte:head>

<SiteHeader />

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

		<h3>What Goes On-Chain (Public and Permanent)</h3>
		<p>
			When you create a wallet, it is written to a <strong>public smart contract</strong> on the
			Gnosis blockchain — the Passkey Registry — with a WebAuthn signature proving you hold the
			passkey. Everything below is <strong>readable by anyone, forever</strong>, and cannot be
			edited or deleted. You can browse exactly what is stored on the
			<a href="/registry">registry page</a>. None of it can move your funds or sign on your behalf.
		</p>
		<p>For each wallet, the registry stores:</p>
		<ul>
			<li>
				<strong>Your passkey's public key</strong> — the P-256 public key (never the private key).
			</li>
			<li>
				<strong>The wallet name you chose</strong> and the <strong>Safe wallet address</strong>, its
				Safe version, and its creation time. The name is public — choose one that does not reveal
				your real identity if you prefer to stay pseudonymous.
			</li>
			<li>
				<strong>Your authenticator's model</strong> — a 20-byte attestation carrying the
				authenticator's AAGUID (which identifies the passkey provider, e.g. Apple Passwords, Google
				Password Manager, a browser, a password manager, or a security key) and the WebAuthn flag
				bits that indicate whether user verification happened and whether the passkey is syncable /
				backed up.
			</li>
			<li>
				<strong>The WebAuthn credential id</strong> — a per-site handle for the passkey (not a
				secret; it cannot authenticate on its own).
			</li>
			<li>
				<strong>Browser-reported hints</strong> — the authenticatorAttachment ("platform" /
				"cross-platform") and the transport list ("internal", "hybrid", "usb", etc.).
			</li>
			<li>
				<strong>The relying-party id</strong> (<code>getvela.app</code>) and a one-time group key
				used only to close the record.
			</li>
		</ul>
		<p>
			The registry is how your wallet's founding passkeys are recorded on-chain, as one immutable
			group. Cross-device sign-in and recovery rely on it, and for a multi-passkey wallet it is
			required: the wallet's address is derived from its <strong>full set of founding passkeys</strong>,
			and a new device reconstructs that set — and therefore the correct address — from this public
			record. (A new device can re-derive a single passkey from two of its signatures on its own, but
			not the rest of a multi-key wallet's membership.)
		</p>

		<h3>What Never Leaves Your Device</h3>
		<p>
			Your passkey <strong>private key</strong>, your biometrics, and your operating-system account
			(Apple ID / Google account) are never uploaded, never transmitted, and never written on-chain.
			Wallet configuration, token balances, transaction history, and RPC endpoint preferences are
			stored locally on your device and are not sent to our servers.
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
		<p>
			The <a href="/registry">registry page</a> reads the smart contract directly from your browser
			via public Gnosis RPC nodes — nothing there comes from our servers. To label each authenticator
			it looks up the AAGUID (an already-public, non-personal identifier of the authenticator model)
			against our AAGUID Explorer service; it never sends your credential id, public key, or wallet
			address to that service.
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
				<strong>Vela Relay</strong> — To relay ERC-4337 UserOperations to the blockchain. The relay
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
			Your wallet's registry record is stored indefinitely and immutably on the Gnosis blockchain —
			it is append-only and cannot be edited or deleted by us or by anyone. On-device data is deleted
			when you log out or uninstall the app. We do not maintain server-side backups of your local
			wallet data.
		</p>
	</section>

	<section>
		<h2>Open Source</h2>
		<p>
			Vela Wallet is fully open source. You can audit the code, verify our claims, and self-host all
			backend services. See <a
				href="https://github.com/mondaylabsltd/vela-wallet"
				target="_blank"
				rel="noopener">our GitHub repository</a
			>.
		</p>
	</section>

	<section>
		<h2>Contact</h2>
		<p>
			If you have questions about this privacy policy, open an issue on
			<a href="https://github.com/mondaylabsltd/vela-wallet/issues" target="_blank" rel="noopener"
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

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
