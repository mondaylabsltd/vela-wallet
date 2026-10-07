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
	<p class="updated">Last updated: 7 October 2026</p>

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
				its operators. With every request, the apps also send it the RPC address they use for that
				network — the one you set for it, the one built from a provider key you added, or the
				built-in one — <strong>including any API key that address contains</strong>. The relay reads
				the network through it, which is what lets it serve a network you added yourself; it may also
				submit your transactions through it on a network its own directory cannot reach. The apps
				say this where you set an RPC address or a provider key. To keep your key from the relay,
				use an address without one, or point the apps at your own relay.
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
			<li>
				<strong>Trusted Signer</strong> (<code>sign.getvela.app</code>), if you chose it as the way
				you sign — a single static page. Our server receives only the request for the page itself:
				the signing request travels in the part of the link after "#", which browsers never send to
				a server, and the page's own security policy stops it from making any network request. Its
				answer goes straight back to the Vela app on your device.
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
			<li>
				<strong>Usage statistics, in the web wallet and the browser extension</strong> (not the
				iPhone, Android or desktop apps) — sent to Rybbit, the cookieless analytics tool the website
				uses, at
				<code>tj.appsdata.org</code>, by the wallet's own code: no outside script runs in it. What
				is sent: which screen is open, as its path with anything that could identify you masked;
				named moments such as "wallet created", "send confirmed" or "network added", with only
				coarse details — the network's chain ID, which kind of passkey, whether it worked or why
				not; the campaign tags of the link you arrived by (<code>utm_source</code> and the like,
				never any other part of the address — a payment link's recipient and amount stay out); your
				screen size, your browser's language and the domain of the site that linked you there. Never
				your addresses, amounts, transaction hashes, contact names or ENS names. Like any web
				request it reaches Rybbit with your IP address and browser type, which it uses to count
				visits and tell countries and devices apart. It sets no cookies and stores no identifier on
				your device, and we don't use it for advertising or share it.
				<strong>Settings → About → Share anonymous usage statistics</strong> turns it off on that browser,
				and from then on nothing is sent.
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
			contacts and settings. <strong>Settings → Erase This Device</strong> deletes them in every
			Vela app — iPhone, Android, desktop and the web wallet — and the iPhone, Android and desktop
			apps also clear their in-app browser's cookies and site data. It keeps one record until its
			job is done: a new wallet's public keys that have not yet reached the public-key index,
			retried on the next launch. On the desktop app for Mac, the in-app browser's site data is
			cleared when the browser has been opened since the app started.
			<a href="/delete">Delete your data</a> gives the steps, what our services keep and for how long,
			and how to ask us to delete it sooner.
		</p>
	</section>

	<section>
		<h2>Bug reports</h2>
		<p>
			When you send feedback from the apps, the report goes to our bug-report service (<code
				>getvela.app/api/bug-report</code
			>), which files it as a <strong>public</strong> issue in our GitHub repository. A report contains
			what you type, the device details the form shows you before you send — app version, system, language,
			the names of networks the app couldn't reach and, where the app records them, recent error messages
			— and any screenshots you attach. Wallet addresses and web addresses in those device details are
			replaced before the issue is filed, and a report never includes your keys.
		</p>
		<p>
			<strong>Screenshots are public.</strong> The app re-encodes each one on your device first, which
			removes location and other photo metadata; our service then stores it on Cloudflare R2 and shows
			it in the issue, where anyone can see it — crop out anything you'd rather keep private. We keep
			reports and screenshots until we delete them; email us and we will.
		</p>
		<p>
			The service uses your IP address, held only in memory, to limit how many reports can be sent;
			it is not stored or passed to GitHub. If the service can't file a report, the app opens a
			pre-filled GitHub form in your browser instead, and nothing is sent until you submit it there
			yourself.
		</p>
	</section>

	<section>
		<h2>The website (getvela.app)</h2>
		<ul>
			<li>
				<strong>Analytics:</strong> we use Rybbit, a cookieless analytics tool, served from
				<code>tj.appsdata.org</code>, to count page views and clicks on some buttons and which
				sections of the home page are viewed. It sets no cookies and we don't use it for advertising
				or share it. Links from the website to the web wallet and to the app stores carry campaign
				tags that name the website and the button, so a visit can be counted as coming from here.
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
			The extension is the Vela wallet packaged for Chrome, so what this page says about the apps
			applies to it too, including bug reports. To let dApps find the wallet, it adds a wallet
			provider (EIP-1193 and EIP-6963) to every web page you open, in the page's main frame only.
			The provider does not read the page's text, forms or anything else on it; it only receives the
			requests a site sends to the wallet.
		</p>
		<ul>
			<li>
				<strong>Requests that need no signature</strong> — such as reading a balance, a contract or the
				latest block, or passing on a transaction that is already signed — are sent on to the RPC nodes
				and relay set in your wallet, for the network that site is on. This happens for any site that
				asks, whether or not you have connected it. The extension sends them, not the site, and the node
				or relay receives the request and your IP address.
			</li>
			<li>
				<strong>Connecting and signing</strong> — a site learns your address only after you approve a
				connection, and your wallet signs or sends nothing until you approve it in Vela's side panel or
				window with one of your keys.
			</li>
			<li>
				<strong>What it keeps in your browser</strong> — the sites you connected and the address each
				may see; the network each site switched to (a site can ask to switch before you connect it, so
				this can include sites you never connected); and, in your activity, each request you approved,
				with the site's address, the message or transaction, and its status. While the browser is open
				it also keeps the requests waiting for your answer and a short log of the sites that asked to
				connect or sign; these are cleared when the browser closes.
			</li>
			<li>
				<strong>Feedback</strong> sent from the extension works as described under Bug reports. Its device
				details also include counts of the extension's own errors, never site names.
			</li>
			<li>
				<strong>Usage statistics</strong> — the extension's wallet reports which of its screens are opened
				and how its steps end, as described under Third parties the apps contact directly, including whether
				you approved or declined a site's request (by kind and network, never the site, the message or
				the transaction). Settings → About turns it off.
			</li>
		</ul>
		<p>
			The extension does not keep a list of the pages you visit. To tell a site's open tabs about a
			change, such as a disconnect, it checks which of your open tabs belong to that site, without
			storing their addresses; when it is first installed it counts the pages already open, only to
			say whether they need a reload. It cannot be turned on in Incognito windows. Apart from the requests
			and the services described on this page, it sends nothing. Removing the extension, or Settings
			→ Erase This Device, deletes what it stored in your browser; your passkeys stay with your
			passkey provider.
		</p>
		<p>
			The use of information received from Google APIs will adhere to the Chrome Web Store User Data
			Policy, including the Limited Use requirements.
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
			<a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a>;
			<a href="/delete">Delete your data</a> says what to include. We keep very little, and we cannot
			delete data written on-chain. You can also complain to the UK Information Commissioner's Office.
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

	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	li {
		color: var(--text-secondary);
		font-size: max(0.95rem, var(--floor-read));
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
