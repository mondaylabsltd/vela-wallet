<script lang="ts">
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Terms of Service — Vela Wallet</title>
	<meta
		name="description"
		content="The terms for using Vela Wallet, its apps and the services we run: self-custody, fees, risks and liability."
	/>
</svelte:head>

<SiteHeader />

<main class="container">
	<h1>Terms of Service</h1>
	<p class="updated">Last updated: 22 September 2026</p>

	<section>
		<h2>1. Who we are, and acceptance</h2>
		<p>
			Vela Wallet ("Vela") is made by MONDAY LABS LTD ("we"), a company registered in England and
			Wales (company number 16988118), 61 Bridge Street, Kington, HR5 3DJ, United Kingdom. These
			terms cover the website, the Vela apps and browser extension, and the backend services we run.
			By using any of them you agree to these terms; if you do not agree, do not use them.
		</p>
	</section>

	<section>
		<h2>2. Self-custody and your keys</h2>
		<p>
			Vela is a self-custodial wallet. Your wallet is a smart-contract account controlled only by
			the keys you create it with — passkeys held by your devices or password manager, other phones,
			or hardware security keys. We never hold those keys and cannot move, freeze, recover or reset
			your wallet or its funds.
		</p>
		<ul>
			<li>
				A wallet has one to seven keys, chosen when you create it. They cannot be added, removed or
				replaced afterwards.
			</li>
			<li>
				<strong>Any one of your keys can move all of your funds on its own.</strong> Whoever obtains one
				of your keys — or control of the account a synced passkey is stored in (for example your Apple
				or Google account) — may be able to take your funds. Protect every key and every such account.
			</li>
			<li>
				If you lose every key you created the wallet with, the wallet cannot be recovered by anyone,
				including us.
			</li>
			<li>Blockchain transactions cannot be reversed, cancelled or modified once submitted.</li>
		</ul>
		<p>
			Creating a wallet publishes a permanent public record to a smart contract on the Gnosis
			blockchain, including your public keys, wallet name, key labels and wallet address. It cannot
			be edited or deleted. The <a href="/privacy">Privacy Policy</a> lists everything it contains.
		</p>
	</section>

	<section>
		<h2>3. Fees and purchases</h2>
		<ul>
			<li>
				<strong>Transaction fees.</strong> Every transaction pays a fee to the relay that submits
				it, taken from your wallet in the network's coin or a supported stablecoin. The amount is
				shown before you sign, is part of what you sign, and covers the network's gas plus the relay
				operator's margin; it is usually several times the transaction's on-chain cost. How it is
				calculated is described in
				<a href="/docs/networks-and-fees">networks &amp; fees</a>. When you use our relay, the fee
				is paid to us. Fees are not refundable.
			</li>
			<li>
				<strong>Relay top-ups.</strong> If a relay's own gas runs out on a network, the app may offer
				you the option to send it a contribution. Such a contribution is voluntary, is paid to the relay's
				operator, does not pay for your transaction, and is not refundable.
			</li>
			<li>
				<strong>Apps.</strong> The web wallet, the browser extension and the desktop apps are free. Apps
				bought from an app store are sold on that store's terms; anything you are entitled to under consumer
				law is unaffected by these terms. You can also build any Vela app from its source code at no charge.
			</li>
		</ul>
	</section>

	<section>
		<h2>4. No financial advice</h2>
		<p>
			Vela is software for managing crypto assets. We do not provide investment, financial, tax or
			legal advice, and nothing we publish is a recommendation to buy, sell or hold any asset.
		</p>
	</section>

	<section>
		<h2>5. Services we run</h2>
		<p>
			We run the relay, the public-key index, the chain-data service and the exchange-rate service,
			and we provide them on a best-effort basis. They may be slow, unavailable, rate-limited,
			changed or discontinued, and the relay may delay or decline to submit a transaction. You can
			run your own instead; the <a href="/docs/self-hosting">self-hosting guide</a> explains how, and
			what cannot be replaced.
		</p>
	</section>

	<section>
		<h2>6. Risks you accept</h2>
		<p>You acknowledge the risks of using blockchain software, including:</p>
		<ul>
			<li>the price of crypto assets can fall, including to zero;</li>
			<li>
				smart contracts can have undiscovered vulnerabilities — including the audited Safe, ERC-4337
				and passkey contracts your wallet uses, and Vela's own registry contract;
			</li>
			<li>
				Vela's own apps and services have not had an independent third-party security audit, and
				Vela is alpha software;
			</li>
			<li>
				token issuers can freeze or blocklist addresses, and networks can congest, fork or fail;
			</li>
			<li>laws and regulations can change and may affect your ability to use crypto assets.</li>
		</ul>
	</section>

	<section>
		<h2>7. Acceptable use</h2>
		<p>
			You must be at least 18 years old and allowed by the laws that apply to you to use Vela. Do
			not use Vela for anything unlawful, including sanctions evasion or fraud, and do not attack or
			overload the services we run.
		</p>
	</section>

	<section>
		<h2>8. Open-source software</h2>
		<p>
			Vela's source code is published under the licences in its repositories (for the wallet, the
			<a
				href="https://github.com/mondaylabsltd/vela-wallet/blob/main/LICENSE"
				target="_blank"
				rel="noopener">MIT License</a
			>). The software is provided "as is", without warranty of any kind, to the extent the law
			allows.
		</p>
	</section>

	<section>
		<h2>9. Limitation of liability</h2>
		<p>
			To the maximum extent permitted by law, MONDAY LABS LTD is not liable for any loss of funds,
			data or profits arising from your use of Vela, including losses caused by software defects,
			smart-contract exploits, compromise of your keys or the accounts that hold them, or
			unavailability of any service. Nothing in these terms limits or excludes liability that cannot
			be limited or excluded by law, including liability for death or personal injury caused by
			negligence, or for fraud.
		</p>
	</section>

	<section>
		<h2>10. Changes</h2>
		<p>
			We may update these terms. When we do, the date at the top of this page changes. If you keep
			using Vela after a change, the updated terms apply.
		</p>
	</section>

	<section>
		<h2>11. Governing law</h2>
		<p>
			These terms are governed by the laws of England and Wales, and the courts of England and Wales
			have jurisdiction, without affecting any mandatory consumer protections where you live.
		</p>
	</section>

	<section>
		<h2>12. Contact</h2>
		<p>
			Email <a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a>, or open an issue on
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
