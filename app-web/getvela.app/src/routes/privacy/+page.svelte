<script lang="ts">
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Privacy Policy — Vela Wallet</title>
	<meta
		name="description"
		content="What data Vela Wallet, its apps and its services handle, what becomes public on-chain, and how long anything is kept."
	/>
</svelte:head>

<SiteHeader />

<main class="container">
	<h1>Privacy Policy</h1>
	<p class="updated">Last updated: 22 September 2026</p>

	<section>
		<h2>Who we are</h2>
		<p>
			Vela Wallet is made by MONDAY LABS LTD, a company registered in England and Wales (company
			number 16988118), 61 Bridge Street, Kington, HR5 3DJ, United Kingdom. We are the data
			controller for this website, the Vela apps and browser extension, and the backend services we
			run for them. For privacy questions or requests, email
			<a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a>.
		</p>
		<p>
			Vela is a self-custodial wallet: we never hold your keys or your funds. This policy says what
			data exists anyway — on your device, on-chain, and on the servers involved — who can see it,
			and for how long.
		</p>
	</section>

	<section>
		<h2>What we never have</h2>
		<ul>
			<li>
				<strong>Your private keys.</strong> They stay in your passkey provider — your device's password
				manager (such as iCloud Keychain, Google Password Manager or 1Password), Windows Hello, another
				phone, or a hardware security key. Vela only ever receives signatures.
			</li>
			<li><strong>A seed phrase.</strong> Vela doesn't use one.</li>
			<li><strong>Your biometrics</strong> or your Apple, Google or password-manager account.</li>
			<li>
				<strong>Your identity.</strong> We don't ask for your name, email, phone number or any ID, and
				the website has no sign-up or newsletter.
			</li>
		</ul>
	</section>

	<section>
		<h2>What becomes public on-chain</h2>
		<p>
			Creating a wallet writes a record to a <strong>public registry contract</strong> on Gnosis
			Chain. It is readable by anyone, permanent, and cannot be edited or deleted by us or anyone
			else. You can browse the records on the <a href="/registry">registry page</a>. For each wallet
			it contains:
		</p>
		<ul>
			<li>
				each key's <strong>public key</strong> (never the private key) and its
				<strong>credential ID</strong>;
			</li>
			<li>
				each key's <strong>authenticator model</strong> (its AAGUID, which identifies the password manager
				or security key that made it) and flags showing whether you were verified and whether the key
				is synced;
			</li>
			<li>the transports the browser reported (for example "internal", "hybrid", "usb");</li>
			<li>
				the <strong>wallet name</strong> you chose and a <strong>label for each key</strong> — anyone
				can look these up from your address, and other Vela users see your wallet name when they send
				to you, so choose a name you're happy to make public;
			</li>
			<li>the <strong>wallet address</strong>, its Safe version and its creation time;</li>
			<li>
				the complete <strong>signed registration data</strong>, including each key's WebAuthn client
				data, which shows where the key was created (getvela.app, or our Android app's package and
				signing identity).
			</li>
		</ul>
		<p>
			The same record is written if you sign in on a new device to a wallet that has none yet. You
			(or anyone) can copy it to the same contract on Ethereum or Base. Older wallets may also
			appear in the earlier index contracts this registry replaced. None of this can move your
			funds.
		</p>
	</section>

	<section>
		<h2>What our services receive</h2>
		<p>
			The apps talk to the services below by default. You can point the apps at your own copies
			instead (see the <a href="/docs/self-hosting">self-hosting guide</a>); then these servers
			receive nothing from you. All of them run on Cloudflare, which processes your IP address and
			may do so outside the United Kingdom.
		</p>
		<ul>
			<li>
				<strong>Public-key index</strong> (<code>p256-index-v2.getvela.app</code>) — receives your
				registration record and your IP address before submitting the record on-chain (it pays the
				gas), and keeps the submission record for 7 days after it lands, or 30 days if it fails.
				When you sign in on a new device, it is asked which wallet your key belongs to. When you
				enter or save an address, it may be asked for that address's Vela wallet name. Your IP
				address is used, in a salted hashed form, to limit request rates.
			</li>
			<li>
				<strong>Relay</strong> (<code>vela-relay-cf.getvela.app</code>) — receives your wallet
				address and each transaction you send, both before you sign (to quote the fee) and after,
				along with your IP address. It logs your address and each operation's hash, keeps operations
				for between one hour and 14 days so it can retry them, forwards them to node providers
				(Alchemy and public RPC nodes) to submit them, and may include operation hashes in alerts to
				its operators. The apps also send it, in a request header, the RPC address they rank first —
				which can include an API key if you added one for a provider.
			</li>
			<li>
				<strong>Chain data</strong> (<code>ethereum-data.getvela.app</code>) — serves network
				details, token lists, logos and transaction descriptors. Requests for logos and descriptors
				include the token or contract address being displayed or signed (never your wallet address).
			</li>
			<li>
				<strong>Exchange rates</strong> (<code>vela-currency.getvela.app</code>) — receives no
				wallet data; its request logs include IP addresses.
			</li>
			<li>
				<strong>Authenticator directory</strong> (<code>aaguid-explorer.awesometools.dev</code>, run
				by Vela's founder) — receives only an authenticator model ID, to show the name of your
				security key or password manager.
			</li>
		</ul>
		<p>We do not sell or share this data, build profiles from it, or use it for advertising.</p>
	</section>

	<section>
		<h2>Third parties the apps contact directly</h2>
		<ul>
			<li>
				<strong>RPC node providers</strong> for each network — see your address, your IP address and the
				transactions the app simulates. Built-in public endpoints are used unless you set your own or
				add a provider key.
			</li>
			<li>
				<strong>Public function-selector databases</strong> (sourcify, openchain, 4byte) — receive a 4-byte
				function selector when a transaction can't be decoded otherwise.
			</li>
			<li>
				<strong>Apple's and Google's tunnel servers</strong> — carry the encrypted exchange when you sign
				with a phone by scanning a QR code; on Android, Google Play services may handle passkey requests.
			</li>
			<li>
				<strong>Your passkey provider</strong> (Apple, Google, a password manager) — stores and syncs
				your passkeys under its own privacy policy.
			</li>
			<li>
				<strong>dApp sites</strong> you connect to — see your wallet address once you connect; the signing
				screen also loads the site's icon from the site itself.
			</li>
		</ul>
	</section>

	<section>
		<h2>What stays on your device</h2>
		<p>
			Your account list, contacts, transaction history, settings, RPC provider keys, and the
			permissions you've given dApps are stored in the app's storage on your device (in the browser
			for the web wallet and extension). On iPhone, app storage can be included in your iCloud
			device backup. The apps' built-in browsers keep site cookies and your browsing history
			locally.
		</p>
		<p>
			<strong>Signing out</strong> removes your accounts from the app but keeps your history,
			contacts and settings. <strong>Settings → Erase this device</strong> deletes them in the web wallet
			and Android app; erasing is not yet complete on iPhone and desktop, where uninstalling (and, on
			desktop, deleting the app's folder in your user settings directory) removes the rest.
		</p>
	</section>

	<section>
		<h2>Bug reports</h2>
		<p>
			The feedback option in the apps opens a pre-filled <strong>public</strong> GitHub issue in your
			browser; on Android it includes the app version, your system and language, and recent error messages.
			Nothing is sent until you review it and submit it yourself on GitHub, where it is public. It never
			includes keys or balances.
		</p>
	</section>

	<section>
		<h2>The website (getvela.app)</h2>
		<ul>
			<li>
				<strong>Analytics:</strong> we use Rybbit, a cookieless analytics tool, served from
				<code>tj.appsdata.org</code>, to count page views and clicks on some buttons and which
				sections of the home page are viewed. It sets no cookies and we don't use it for advertising
				or share it.
			</li>
			<li>
				<strong>Fonts:</strong> pages load fonts from Google Fonts, so Google receives your IP address
				when you visit.
			</li>
			<li>
				<strong>Registry page:</strong> reads the registry contract from your browser through public Gnosis
				RPC nodes, and looks up authenticator names in the directory above using only the model ID.
			</li>
			<li>
				<strong>Chain setup page:</strong> if you create a deployer key there, it is generated and stored
				in your browser's local storage, and never sent to us.
			</li>
		</ul>
	</section>

	<section>
		<h2>The browser extension</h2>
		<p>
			The extension can read and act on the pages you visit so that it can offer the wallet to
			dApps. It stores which sites you've connected and which network each uses in your browser, and
			forwards a connected site's read requests to your RPC nodes and relay. It sends nothing else.
		</p>
	</section>

	<section>
		<h2>Retention</h2>
		<ul>
			<li>On-chain registry records: permanent, by design.</li>
			<li>Index submission records: 7 days after landing on-chain, 30 days if they fail.</li>
			<li>
				Relay operation records: between one hour and 14 days; logs as kept by our hosting provider.
			</li>
			<li>On your device: until you erase them or uninstall the app.</li>
		</ul>
	</section>

	<section>
		<h2>Your rights</h2>
		<p>
			You can ask what personal data our services hold about you, and ask us to delete it, at
			<a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a>. We keep very little, and we
			cannot delete data written on-chain. You can also complain to the UK Information
			Commissioner's Office.
		</p>
	</section>

	<section>
		<h2>Open source</h2>
		<p>
			The apps and services are public on
			<a href="https://github.com/orgs/mondaylabsltd/repositories" target="_blank" rel="noopener"
				>GitHub</a
			>, so you can check what they do. When this policy changes, the date at the top changes too.
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
