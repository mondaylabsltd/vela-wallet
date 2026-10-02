<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Delete Your Data — Vela Wallet</title>
	<meta
		name="description"
		content="How to delete what Vela Wallet keeps on your device and on MONDAY LABS LTD's servers, how long anything is kept, and what is permanent on-chain."
	/>
</svelte:head>

<SiteHeader />

<main class="container">
	<h1>Delete your data</h1>
	<p class="updated">Last updated: 1 October 2026</p>

	<section>
		<p>
			This page is for <strong>Vela Wallet</strong>, made by <strong>MONDAY LABS LTD</strong>, a
			company registered in England and Wales (company number 16988118), 61 Bridge Street, Kington,
			HR5 3DJ, United Kingdom. It covers every Vela app — iPhone, Android, desktop and the web
			wallet — and says how to delete what Vela keeps, what is kept and for how long, and what is
			public on-chain for good.
		</p>
		<p>
			There is no Vela account to close: Vela has no sign-up and never asks for your name, email
			address or phone number. Your wallet is controlled by its keys alone. Deleting data never
			moves your funds — they stay at your wallet's address, ready for any of its keys.
		</p>
	</section>

	<section>
		<h2>On your device</h2>
		<p><strong>Erase This Device</strong> deletes everything Vela stored on the device:</p>
		<ol>
			<li>Open <strong>Settings</strong> and scroll to the bottom.</li>
			<li>Choose <strong>Erase This Device</strong>.</li>
			<li>Confirm with <strong>Erase Everything</strong>.</li>
		</ol>
		<p>
			It deletes your transaction history, contacts and groups, browsing history, dApp permissions,
			custom tokens and networks, RPC and endpoint settings, and every preference, and signs every
			wallet on the device out. In the iPhone, Android and desktop apps it also clears the in-app
			browser's cookies and site data. The app then checks that the data is gone; if anything is
			left, it says so and keeps you signed in so you can try again.
		</p>
		<ul>
			<li>
				One record waits until its job is done: if a wallet's public keys have not yet reached our
				public-key index, the app keeps them and retries on the next launch, so the wallet can still
				be found when you sign in elsewhere. The record goes once the upload succeeds.
			</li>
			<li>
				On the desktop app for Mac, the in-app browser's site data is cleared when the browser has
				been opened since the app started — open it once before erasing.
			</li>
			<li>
				On iPhone and Android, uninstalling the app also deletes what it stored. Erasing does not
				reach a backup made earlier, such as an iCloud backup of your iPhone.
			</li>
		</ul>
		<p>
			<strong>One wallet only.</strong> To take one wallet off a device and keep the others, tap
			your wallet at the top of Settings and choose <strong>Remove from this device</strong> beside
			it. Like <strong>Sign Out</strong>, this removes the wallet from the app's list and keeps its
			history, contacts and settings, so signing in again brings it back. Erase This Device deletes
			those too.
		</p>
	</section>

	<section>
		<h2>Your passkeys</h2>
		<p>
			Vela never holds your passkeys. They are kept by your passkey provider — iCloud Keychain,
			Google Password Manager, another password manager, or a security key — and erasing a device
			leaves them where they are. To delete a passkey, delete it there: in your password manager's
			settings, or with your security key's own management tool.
		</p>
		<p>
			A wallet's funds move only with one of its keys. Before you delete a passkey, move the funds
			to a wallet you keep, or make sure you still have another of that wallet's keys.
		</p>
	</section>

	<section>
		<h2>On our servers</h2>
		<p>
			Our services keep only what they need to register your wallet and send your transactions, and
			delete it on their own:
		</p>
		<ul>
			<li>
				<strong>Public-key index</strong> (<code>p256-index-v2.getvela.app</code>) — your wallet's
				registration: its address, the wallet name and key labels you chose, each key's public key
				and credential ID, and the signed registration data. Kept for <strong>7 days</strong> after
				the registration lands on-chain, or <strong>30 days</strong> if it fails, then deleted.
			</li>
			<li>
				<strong>Relay</strong> (<code>vela-relay-cf.getvela.app</code>) — the transactions you send,
				with your wallet address. Kept for between <strong>one hour and 14 days</strong> so they can be
				retried, then deleted. The relay's logs record your address and each transaction's hash and are
				kept as long as our hosting provider, Cloudflare, keeps them.
			</li>
			<li>
				<strong>Bug reports</strong> you send from the app (Settings → Send feedback) — filed as
				<strong>public</strong> issues in our GitHub repository, with any screenshots stored on Cloudflare
				R2. Kept until we delete them; ask and we will.
			</li>
		</ul>
		<p>
			If you point the apps at your own relay and index (see the
			<a href={resolve('/docs/self-hosting')}>self-hosting guide</a>), our servers receive none of
			this.
		</p>
	</section>

	<section>
		<h2>What stays public</h2>
		<p>
			Two things live on public blockchains, by design. They are permanent, and nobody — MONDAY LABS
			LTD included — can edit or delete them:
		</p>
		<ul>
			<li>
				<strong>Your wallet's registry record on Gnosis Chain.</strong> Creating a wallet publishes
				each key's public key, credential ID, authenticator model and flags, your wallet name and
				key labels, the wallet address, and the signed registration data. This record is what lets
				any of your keys find the wallet on a new device. It holds no private key and cannot move
				your funds. You can view it on the <a href={resolve('/registry')}>registry page</a>.
			</li>
			<li>
				<strong>Your transactions.</strong> Every transaction your wallet makes is recorded on that network's
				public blockchain, as with any wallet.
			</li>
		</ul>
		<p>
			When the index deletes its own copy after 7 or 30 days, the on-chain record stays, and erasing
			a device does not change it.
		</p>
	</section>

	<section>
		<h2>Ask us to delete it sooner</h2>
		<p>
			To have our servers delete what they still hold before it expires — or anything you find kept
			longer than this page says — email
			<a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a> with:
		</p>
		<ul>
			<li>your <strong>wallet address</strong>, for index and relay records;</li>
			<li>
				the <strong>issue link</strong>, for a bug report and its screenshots (wallet addresses are
				removed from a report's device details, so we can't find one by address).
			</li>
		</ul>
		<p>
			Nothing else is needed, and we will delete it. The <a href={resolve('/privacy')}
				>Privacy Policy</a
			>
			covers everything Vela handles and your rights; for anything else, see
			<a href={resolve('/support')}>Support</a>.
		</p>
	</section>
</main>

<SiteFooter />

<style>
	/* Same reading page as /privacy and /terms. */
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
		font-size: max(0.85rem, var(--floor-meta));
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
		font-size: max(0.95rem, var(--floor-read));
		margin-bottom: 12px;
	}

	/* The links on this page are the way to act on it, so they look like links. */
	main a {
		color: var(--link);
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--link) 35%, transparent);
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
	}
	main a:hover {
		text-decoration-color: var(--link);
	}

	ul,
	ol {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 16px;
	}
	li {
		color: var(--text-secondary);
		font-size: max(0.95rem, var(--floor-read));
		line-height: 1.7;
		padding-left: 20px;
		position: relative;
	}
	ul li::before {
		content: '';
		position: absolute;
		left: 0;
		top: 10px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent);
	}
	/* Numbered steps: the count is the instruction, so it is printed. */
	ol {
		counter-reset: step;
	}
	ol li {
		counter-increment: step;
		padding-left: 28px;
	}
	ol li::before {
		content: counter(step) '.';
		position: absolute;
		left: 0;
		color: var(--accent);
		font-weight: 600;
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
