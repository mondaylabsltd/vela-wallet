/**
 * The English message source — and the TYPE every other locale is checked
 * against (spec 059, contracts/translation-store.md).
 *
 * Why TypeScript and not JSON: this file is the shape. A component reading
 * `m.chrome.nav.docs` is checked by `bun run check`, so a typo is a build
 * error rather than a blank label in production. The fourteen translations are
 * JSON (`./<tag>.json`) so a translator never edits code and can never break
 * the build.
 *
 * ## Inline markup is allowed, and is part of the string
 *
 * Marketing prose has links and emphasis inside sentences. Splitting a sentence
 * into three keys around an `<a>` gives a translator fragments they cannot
 * reorder — and every language reorders. So a value may contain `<a href>`,
 * `<strong>` and `<em>`, and the component renders it with `{@html}`.
 *
 * Two consequences, both enforced rather than trusted:
 *   - the href set and tag counts of a translation MUST match the English value
 *     (FR-031, checked by placeholders.test.ts). A translator translates the
 *     link TEXT, never the URL.
 *   - nothing here may ever come from user input. These are repo files.
 *
 * ## What does NOT belong here
 *
 * Proper nouns. "GitHub", "Telegram", "MetaMask", "Safe", "Vela
 * Relay", "RSS" are names, not words — they stay in the components where no
 * translator will be tempted to localize them.
 *
 * ## chrome and notice are special
 *
 * They must be COMPLETE in all fifteen locales at all times, because they are
 * what an untranslated page is wrapped in. The one sentence a reader of a
 * fallback page must be able to read is the one telling them the rest is
 * English — it cannot itself fall back.
 */

const GITHUB = 'https://github.com/mondaylabsltd/vela-wallet';
const SAFE_REPO = 'https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1';
const RECOVERY_EXT = `${GITHUB}#webauthn-proxy-extension-domain-recovery--dev-passkeys`;

export const en = {
	chrome: {
		nav: {
			/** The pill next to the logo. */
			alpha: 'Alpha',
			blog: 'Blog',
			docs: 'Docs',
			about: 'About',
			whyVela: 'Why we built it',
			howItWorks: 'How it works',
			pricing: 'Pricing',
			faq: 'FAQ',
			createWallet: 'Get Vela',
			signIn: 'Sign in',
			/** aria-labels — read aloud, never seen. Still copy. */
			primary: 'Primary',
			toggleMenu: 'Toggle menu'
		},
		footer: {
			tagline: 'An Ethereum wallet you actually own. No seed phrase.',
			label: 'Footer',
			columns: {
				resources: 'Resources',
				infrastructure: 'Infrastructure',
				community: 'Community',
				legal: 'Legal'
			},
			links: {
				docs: 'Docs',
				whitepaper: 'Whitepaper',
				audits: 'Audits & known issues',
				roadmap: 'Roadmap',
				blog: 'Blog',
				about: 'About',
				chainSetup: 'Chain setup',
				selfHosting: 'Self-hosting guide',
				privacy: 'Privacy',
				terms: 'Terms'
			}
		},
		language: {
			/** Button label and aria-label for the switcher. */
			label: 'Language',
			choose: 'Choose a language'
		},
		/**
		 * The docs sidebar. This is CHROME, not page content: it frames every docs
		 * page including the ones still falling back to English, so it merges per
		 * key and a missing title simply stays English rather than blanking a
		 * navigation row. Order and slugs stay in `content/sidebar.ts` — a
		 * translator names things, they do not reorder the docs.
		 */
		docs: {
			groups: {
				gettingStarted: 'Getting Started',
				using: 'Using Vela',
				keys: 'Keys & recovery',
				security: 'Security',
				selfHost: 'Run it yourself',
				reference: 'Reference'
			},
			titles: {
				introduction: 'Introduction',
				'why-vela': 'Why we built Vela',
				install: 'Install Vela',
				'create-wallet': 'Create your wallet',
				'send-and-receive': 'Send & receive',
				'networks-and-fees': 'Networks & fees',
				passkeys: 'How passkeys work',
				signers: 'Signers & security keys',
				'clear-signing': 'Clear signing',
				'clear-signing-self-host': 'Self-host the signing page',
				'self-hosting': 'Self-hosting guide',
				'bybit-attack': 'The Bybit attack',
				recovery: 'Recovery & sign-in',
				'account-contract': 'The account contract',
				'security-audits': 'Audits & known issues',
				whitepaper: 'Whitepaper',
				faq: 'FAQ'
			}
		},
		englishOnly: {
			/** Badge on a link that leads to an English-only page (FR-020). */
			badge: 'EN',
			title: 'This page is only available in English'
		}
	},

	notice: {
		/** Shown above an English body served at a localized URL (FR-017). */
		fallback: 'This page has not been translated yet. The text below is in English.',
		offer: {
			/**
			 * The offer banner (FR-013). Rendered in the language being OFFERED —
			 * it is shown to somebody who may not read the page they are on — and it
			 * never navigates on its own.
			 */
			message: 'This page is also available in English.',
			accept: 'Read in English',
			dismiss: 'Dismiss'
		}
	},

	home: {
		meta: {
			title: 'Vela Wallet — An Ethereum wallet you actually own',
			description:
				'An open-source Ethereum wallet you can run without us. Sign with passkeys or security keys — no seed phrase. Your account is an unmodified Safe, and every service behind it can be replaced.',
			ogTitle: 'Vela Wallet — An Ethereum wallet you actually own',
			ogDescription:
				'Open-source Ethereum wallet on an unmodified Safe. Passkeys or security keys, no seed phrase, and services you can replace with your own.',
			/** schema.org Organization description (FR-024). */
			organization:
				'An open-source, self-custodial Ethereum wallet built on an unmodified Safe, signed with passkeys or security keys, with no seed phrase.'
		},

		hero: {
			/**
			 * Founder-approved. The headline is verbatim from
			 * specs/059/approved-copy.md; the subtitle was cut down to its first
			 * sentence on 2026-09-15 — the rest of it ("open source and
			 * self-hostable… even if we disappear") became a fact on the right,
			 * where it can be a hook instead of a clause.
			 *
			 * Rewritten 2026-09-16 into two beats: WHERE the signing happens, then
			 * what we never get. "Never sees" was a claim about our conduct;
			 * "never receives" is a claim about where the key is, which is the one
			 * we can actually stand behind — and the device is the half a reader
			 * checks first. All fifteen locales were realigned to it the same day
			 * (reviews/single-string.md).
			 */
			headline: 'An Ethereum wallet you actually own',
			subtitle: 'Signing happens on your device. Vela never receives your passkey.',
			ctaCreate: 'Getting started',
			ctaCode: 'Read the code',
			/**
			 * Four hooks. Written for a reader who can check them, because that is
			 * who lands here — and each one links to a page that exists to prove
			 * that single claim rather than to a doc that mentions it in passing.
			 *
			 *   1. The differentiator. "No seed phrase" is table stakes for ANY
			 *      passkey wallet; an unmodified, audited, standard account
			 *      contract underneath is not.
			 *   2. The real answer to "what if I lose my phone" — not "it syncs",
			 *      which is a property of the OS and can be switched off, but up
			 *      to seven signers fixed at creation, hardware keys included.
			 *   3. Bybit, because it is the case that proves the point: the Safe
			 *      contracts held, the INTERFACE lied, and the hardware wallets
			 *      signed anyway.
			 *   4. The one line no wallet in the comparison below can copy.
			 *
			 * Dropped, and why: "no seed phrase" (table stakes, sells the
			 * category not the product); "one address, every chain" (the networks
			 * have their own section now); "no one can freeze your money, not even
			 * us" (false — a stablecoin issuer can freeze any address, in any
			 * wallet; the accurate version of that claim lives in the whitepaper
			 * and the trade-offs section, not in a hook).
			 *
			 * Rewritten 2026-09-16: all four terms became plain statements of
			 * fact. They had drifted into four different voices — a noun phrase, a
			 * piece of advice ("Don't bet the wallet on one device"), two
			 * fragments, and a conditional — which made the column read as a
			 * slogan wall. A claim a reader is invited to CHECK has to be stated,
			 * not performed; #3 also picked up the hedge it always needed, since
			 * "isn't necessarily" is the true version and "is another" is not.
			 */
			facts: [
				{
					term: 'Your account is an unmodified Safe v1.4.1.',
					link: "Your account is Safe's contract, not one of ours"
				},
				{
					term: 'Your wallet can have more than one signing key.',
					link: 'Up to seven keys, set at creation — hardware keys included'
				},
				{
					term: 'What you see on screen isn’t necessarily what gets signed.',
					link: 'How Bybit lost $1.5B, and what Vela does about it'
				},
				{
					term: 'Access to your wallet doesn’t depend on Vela staying online.',
					link: 'What to run yourself, and what still works without getvela.app'
				}
			]
		},

		/**
		 * The four hooks above get a screen of their own, and a screen needs to
		 * say what it is. One line, and deliberately not a heading: the claims are
		 * the loud part, this is the caption under the exhibit.
		 *
		 * Rewritten 2026-09-16. It used to count the rows ("Four things worth
		 * checking…") — which the numbers already do — and to hedge behind "a
		 * wallet", as if the four claims were about wallets in general. They are
		 * about this one. What the line adds now is the moment: before the
		 * deposit, while walking away is still free. No full stop; it is a
		 * caption, not a sentence.
		 */
		facts: {
			lede: 'Before you put assets in Vela'
		},

		/**
		 * The on-chain wallet counter, rendered in the hero as a stamped seal.
		 * The label is a noun phrase, not a sentence fragment: the number sits
		 * above it, so "355 wallets created on-chain" reads as two lines, not as
		 * a sentence cut in half.
		 */
		seal: {
			label: 'wallets created on-chain',
			loading: 'Loading wallet count',
			verify: 'Every wallet is on-chain — see the registry'
		},

		/**
		 * The short version. The essay that used to live here — twelve words, what
		 * passkeys change, what we gave up — is now /docs/why-vela, because a
		 * landing page is not where somebody reads nine paragraphs. What stays is
		 * the part that answers "why does this exist and not just Base Account".
		 *
		 * p2 rewritten 2026-09-16. It was two fragments — "Same passkey sign-in.
		 * None of the lock-in." — which read as a spec line where the paragraph
		 * wants a sentence, and "none of the lock-in" claimed a result rather than
		 * the work: what we actually did was keep the sign-in and then make sure
		 * the wallet does not need us.
		 */
		why: {
			heading: 'Why we built it',
			p1: 'We used <a href="https://docs.cdp.coinbase.com/coinbase-wallet/overview" target="_blank" rel="noopener">Base Account</a> (now part of Coinbase Wallet) every day and liked it. Then we hit the walls: a recovery phrase created on a website you just have to trust, no custom networks, nothing you can host yourself, and a signing service whose code isn’t public — if it goes away, there is no published way for your passkey to reach your account.',
			p2: 'Vela is the version we were willing to keep money in. We kept passkey signing, but made sure the wallet doesn’t depend on us.',
			more: 'The long version — where are you supposed to keep twelve words?'
		},

		/**
		 * Immediately after the pitch, on purpose. Every one of these is a reason
		 * somebody should NOT use Vela today, written before they deposit rather
		 * than discovered after.
		 *
		 * items[0] cut from four paragraphs to three on 2026-09-16. What went was
		 * the paragraph that explained at length that the fee shown is the fee
		 * charged; "fixed when you sign" says it in four words, and a trade-off
		 * that spends a quarter of itself on its own defence stops reading as a
		 * trade-off. Nothing that costs the reader money was dropped: it still
		 * says the gas is higher, that we take a service fee on top of the
		 * on-chain cost, and that you can leave.
		 */
		tradeoffs: {
			heading: 'The trade-offs, up front',
			lede: 'Three reasons not to use Vela — while your money is still somewhere else.',
			items: [
				{
					title: 'Every transaction pays a relay fee on top of the gas',
					body: 'A Vela transaction verifies your passkey signature on-chain and runs through ERC-4337, so it uses several times the gas of a plain transfer.\n\nThe relay pays that gas and charges a fee: three times the gas it reserves, which usually comes to several times the real on-chain cost, with a $0.01 minimum. The exact amount is shown before you sign and can’t change afterwards.\n\nYou can point the wallet at another relay or <a href="/docs/self-hosting#relay">run your own</a> — the fee then goes to whoever runs it.'
				},
				{
					title: 'Every key is a way into your wallet',
					body: 'You choose up to seven keys when you create the wallet, and they can’t be changed afterwards.\n\nVela is 1-of-n: any one of those keys can spend from the wallet on its own. Each extra key is another way back in if you lose one — and another key you have to protect.'
				},
				{
					title: 'The contracts are audited. Vela’s own code is not.',
					body: 'Your wallet runs on Safe v1.4.1 and Safe’s passkey and ERC-4337 modules, unmodified, and all of them have published third-party audits — which find problems but can’t prove there are none left.\n\nVela’s own apps and services have not had a third-party audit, and none is scheduled. The code is public for anyone to read.'
				}
			],
			close: 'If one of those three is unacceptable to you, Vela is not for you yet.'
		},

		compare: {
			heading: 'How Vela compares',
			/**
			 * A heading and twelve rows. Nothing else, on purpose (2026-09-16).
			 *
			 * There used to be a `desc` above the table and a five-paragraph `note`
			 * below it summing up what each of the three wallets is good at. Both
			 * are gone: a reader who has just read twelve rows does not need to be
			 * told what they say, and prose that explains a table it sits next to
			 * reads as filler however carefully it is written.
			 *
			 * The rows carry the comparison. If one of them is unclear, the fix is
			 * that row — not a paragraph around it, which would also mean fifteen
			 * translations to keep true as rows change.
			 */
			rows: [
				{
					feature: 'Account type',
					vela: '<a href="/docs/account-contract">Safe v1.4.1 smart account</a>',
					metamask: 'EOA; upgraded to a smart account (EIP-7702) by default for new users',
					base: 'Coinbase smart account'
				},
				{
					feature: 'Signing key',
					vela: 'Passkeys or security keys, up to seven',
					metamask: 'The private key behind a seed phrase; hardware wallets supported',
					base: 'Passkey or recovery phrase'
				},
				{
					feature: 'Transaction gas',
					vela: 'ERC-4337 overhead, plus a relay fee that is a multiple of the gas',
					metamask: "A plain transaction's gas",
					base: 'ERC-4337 overhead'
				},
				{
					feature: 'Sponsored gas',
					vela: 'Not offered',
					metamask: 'On some networks',
					base: 'Where the app sponsors it'
				},
				{
					feature: 'Custom networks',
					vela: "Any EVM network that meets Vela's requirements; you can deploy most missing contracts yourself",
					metamask: 'Custom EVM networks and RPCs',
					base: 'A fixed list of networks'
				},
				{
					feature: 'Losing one key',
					vela: 'Sign with another key you added or synced',
					metamask: 'Restore from the seed phrase',
					base: 'Sign with another key you added or synced'
				},
				{
					feature: 'Batched transactions',
					vela: 'Supported',
					metamask: 'Supported through its EIP-7702 smart account',
					base: 'Supported'
				},
				{
					feature: 'Decoded before signing',
					vela: 'Supported',
					metamask: 'Supported',
					base: 'Supported'
				},
				{
					feature: 'An extra check',
					vela: 'An independent signing page, built but not yet connected to the apps',
					metamask: 'Third-party risk alerts',
					base: '\u2014'
				},
				{
					feature: 'Full self-hosting',
					vela: 'Apps and every backend service; passkeys stay tied to getvela.app',
					metamask: 'Client and RPC are yours to choose or run',
					base: 'Not available'
				},
				{
					feature: 'Source code',
					vela: '<a href="https://github.com/orgs/mondaylabsltd/repositories" target="_blank" rel="noopener">Apps and services public</a>; MIT except the index, whose licence is pending',
					metamask: 'Public, under a licence that allows non-commercial use only',
					base: 'Contracts and SDK open; the signing service is not'
				},
				{
					feature: 'Maturity',
					vela: 'New, few users',
					metamask: 'Years in the market, a large user base',
					base: 'Maintained by Coinbase'
				}
			]
		},

		/**
		 * Three cards, and each one is now a title and a price. The paragraph
		 * under each price went on 2026-09-16 with the rest of the landing page's
		 * prose: "Open it, prove who you are, and that's it" explained a price of
		 * "Free", which the price had already said.
		 *
		 * Two things went with them and are flagged, not replaced: the link to the
		 * GitHub releases page (the hero's "view the code" button still goes to
		 * the repo, but nothing on the site now points at the builds), and the
		 * promise that there is never a subscription — a claim the site no longer
		 * makes anywhere, including the docs. If either matters, it belongs in the
		 * card's price line or in a docs page, not in a paragraph restored here.
		 */
		pricing: {
			heading: 'Free and open. Pay only if you want to.',
			intro: 'You pay for convenience, not for access.',
			cards: [
				{
					title: 'Web, browser extension and desktop',
					price: 'Free'
				},
				{
					title: 'iPhone and Android, built yourself',
					price: 'Free'
				},
				{
					title: 'iPhone and Android, from the stores',
					price: 'One-time purchase · coming soon'
				}
			]
		},

		networks: {
			heading: '24 networks built in. Add your own',
			body: 'Your wallet has the same address on every network. When you add another EVM chain, Vela checks that it has the RIP-7212 precompile and the Safe and ERC-4337 contracts it needs. If some contracts are missing, <a href="/chain-setup">chain setup</a> shows which, and deploys the ones anyone can deploy.',
			link: 'Networks & fees, in detail'
		},

		faq: {
			/**
			 * Seven questions, down from eleven on 2026-09-16, and reordered so the
			 * section answers what is LEFT rather than repeating the page above it.
			 * Four went because the page had already answered them: seed phrase /
			 * extension / hardware wallet (the hero), which chains and tokens (the
			 * networks section), what it costs (the first trade-off plus the pricing
			 * cards), whether the code is audited (the third trade-off). A fifth —
			 * "can I add a second key?" — dissolved into 1, 3 and 5, which is where a
			 * reader actually needs the rule that the key set is fixed at creation.
			 *
			 * The order widens by scope: what I have to do (1), what I can do with it
			 * (2), the three ways it goes wrong — the device (3), my own hand (4), the
			 * platform account behind it (5) — and then the two questions about us:
			 * the company that could misbehave (6), and the company that could
			 * disappear (7).
			 *
			 * Two paragraphs per answer, blank-line separated as in `tradeoffs`, and
			 * no links: each of these is a complete answer, not a doorway. Two stale
			 * claims died with the old list — the dApp answer sent people to
			 * WalletPair, which the wallet no longer supports at all, and the shutdown answer
			 * pointed at a "recovery extension" deleted in spec 039. What replaces
			 * them is what ships: the provider the apps and the extension inject, and
			 * `app-web/clearsigning`, whose relying party is still getvela.app.
			 */
			heading: 'FAQ',
			items: [
				{
					q: 'What do I need to create a wallet?',
					a: 'A phone or computer that supports passkeys, or hardware security keys \u2014 two, if you use only security keys. No seed phrase, email or starting balance.\n\nChoose all your keys, up to seven, when you create the wallet. You can\u2019t add more later.'
				},
				{
					q: 'Can I use Vela with dApps?',
					a: 'Yes. dApps see Vela the way they see any browser wallet.\n\nThat works in the Vela browser extension and in the browser built into the desktop (macOS, Windows), iPhone and Android apps. The web wallet doesn\u2019t connect to dApps.'
				},
				{
					q: 'What if I lose my phone?',
					a: 'If your passkey is synced with iCloud Keychain or Google Password Manager, you can recover it on a new device and keep using the same wallet.\n\nIf it was not synced, you will need another key you added when creating the wallet.'
				},
				{
					q: 'What if I delete my passkey?',
					a: 'That key is permanently lost. If it was your only key, you will lose access to the wallet.'
				},
				{
					q: 'What if my Apple or Google account is compromised?',
					a: 'If someone gains access to your synced passkeys, they may be able to access your wallet.\n\nIf you do not want to rely on Apple or Google, use a USB/NFC security key instead.'
				},
				{
					q: 'What can Vela do to my money, and what does it know about me?',
					a: 'Vela cannot move or freeze your funds. Only your keys control the wallet.\n\nThere\u2019s no email or account. Your public keys, wallet name and address are public on-chain, and Vela\u2019s relay sees the transactions you send through it. Token issuers can still blocklist addresses.'
				},
				{
					q: 'What if Vela shuts down or getvela.app goes offline?',
					a: 'Your funds stay in your Safe on-chain, and every service Vela runs can be replaced with your own.\n\nIf getvela.app goes offline, the Vela browser extension still signs with your existing keys \u2014 this device\u2019s passkey, a security key, or a phone by QR code \u2014 and so do apps you build yourself, with a phone or a security key.'
				}
			]
		}
	},

	about: {
		meta: {
			title: 'About',
			description:
				'Who builds Vela Wallet: a small UK company and its founder, working in public on an open-source, self-custodial wallet with no seed phrase.',
			/** schema.org AboutPage name. */
			pageName: 'About Vela Wallet'
		},
		eyebrow: 'About',
		heading: 'Who builds Vela.',
		lede: 'Vela is made by MONDAY LABS LTD, a small company in the United Kingdom, and built in public: the apps, the backend services and this site are all on GitHub, and the person who wrote them answers issues there.',
		team: {
			/** The person's name is a proper noun and stays in the component. */
			role: 'Founder & Engineer',
			bio: 'Builds Vela end to end — the apps, the services and this site — and writes about it on the blog as it happens.'
		},
		valuesHeading: 'What we believe',
		values: [
			{
				title: 'Self-custody, for real',
				body: 'We cannot move, freeze or recover your funds. That is how the wallet is built, not a policy we could change.'
			},
			{
				title: 'No seed phrases',
				body: 'A seed phrase is a secret people are asked to guard perfectly for years. We replaced it with passkeys: keys held by your devices or a security key, used after your face, fingerprint or PIN.'
			},
			{
				title: 'Open source',
				body: 'The apps and services are public on GitHub, so you can check what they do instead of taking our word for it.'
			},
			{
				title: 'Honest about trade-offs',
				body: "Every design choice gives something up. We write down what, and why — in the docs and on the blog — instead of pretending there's no cost."
			}
		]
	},

	roadmap: {
		meta: {
			title: 'Roadmap — Vela Wallet',
			description:
				"What Vela has shipped since April 2026 and what's coming next — built in the open. Directions, not deadlines."
		},
		heading: 'Roadmap',
		lede: 'Vela has shipped continuously since April 2026, in the open. Here\'s the trail so far and where it\'s headed — directions, not deadlines. Want something on it? <a href="https://github.com/mondaylabsltd/vela-wallet/issues" target="_blank" rel="noopener">Open an issue</a>.',
		upcomingHeading: 'Up next',
		shippedHeading: 'Shipped',
		statusLabels: {
			now: 'In progress',
			next: 'Next',
			later: 'Exploring'
		},
		upcoming: [
			{
				title: 'iPhone and Android in the stores',
				body: 'The native apps work and are being tested on real devices. Next is the App Store and Google Play release, as a one-time purchase.'
			},
			{
				title: 'Every app honouring your own services',
				body: 'Today some apps ignore parts of Settings → Service Endpoints — the iPhone app entirely, and the web and desktop apps for the passkey index. They should all honour it, and the relay should be able to read chain data from your own server.'
			},
			{
				title: 'The independent signing page, connected',
				body: 'The signing page that decodes and signs a request on its own is built. Next: let the apps hand their requests to it, so a program you loaded yourself checks every signature.'
			},
			{
				title: 'Wider clear-signing coverage',
				body: 'More contracts and chains shown as human-readable intent, so fewer transactions fall back to blind signing.'
			},
			{
				title: 'See every coin you receive',
				body: 'A native-coin deposit that arrives through another contract (an exchange withdrawal, a router, a multisig) leaves no log on many networks, so it can be missing from your activity. We want to trace those so every deposit shows up.'
			}
		],
		shipped: [
			{
				title: 'Your keys on-chain, with a backup on Ethereum',
				body: 'Sign in straight from the on-chain registry when the index is down, see every key your wallet was created with, and copy the record to Ethereum.'
			},
			{
				title: 'Downloads for every desktop',
				body: 'The desktop app for macOS, Windows and Linux, and the browser extension, download directly from this site.'
			},
			{
				title: '24 networks',
				body: 'Twelve more networks built in — Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume and XRPL EVM — and a chain-setup page for bringing Vela to another chain.'
			},
			{
				title: 'One core, native apps',
				body: 'The wallet rebuilt as one Rust core that holds every rule, with native apps on top: SwiftUI on iPhone, Compose on Android, a Rust desktop app and the web. The original React Native app was retired.'
			},
			{
				title: 'dApps, wherever the wallet runs',
				body: 'The browser extension, and a browser built into the desktop, iPhone and Android apps, connect to dApps like any browser wallet.'
			},
			{
				title: 'Up to seven keys, security keys included',
				body: 'Create a wallet with several passkeys, phones or YubiKey-class security keys, any one of which can sign.'
			},
			{
				title: 'Send to many at once',
				body: 'Split one token among many people, sweep many tokens to one address, or import a payment list from a spreadsheet — each as a single transaction. Contacts and groups too.'
			},
			{
				title: 'Fifteen languages',
				body: 'The wallet in fifteen languages, with local currency and number formats.'
			},
			{
				title: 'Clear signing (ERC-7730)',
				body: 'Transactions show what they do — amount, recipient, intent — instead of raw hex, and an unlimited token approval can’t be submitted.'
			},
			{
				title: 'Vela is born',
				body: 'The first commit: Safe smart accounts, ERC-4337, passkey sign-in and no seed phrase, from day one.'
			}
		]
	},

	getStarted: {
		meta: {
			title: 'Get Vela',
			description:
				'Vela runs in your browser today, with a browser extension and desktop apps you can download now, and phone apps on the way. Pick the one you want.'
		},
		heading: 'Get Vela',
		lede: 'One wallet and one address, wherever you open it. Start in your browser — nothing to install — or take the same wallet to your browser toolbar or your desktop, and soon your phone.',
		recommended: 'Recommended',
		availableNow: 'Available now',
		comingSoon: 'Coming soon',
		/**
		 * The phone card's second route (spec 063). Phones have no packages on
		 * GitHub — the stores are the channel, and they are paid — so the honest
		 * alternative to "buy it" is the one `fundingNote` already promises.
		 */
		sourceCta: 'Build it from source',
		platforms: {
			web: {
				title: 'Web',
				blurb:
					'Nothing to install and nothing to update. Open it, confirm with one of your keys, and your wallet is there — on any device with a recent browser. To connect to dApps, use the extension.',
				action: 'Open the web wallet'
			},
			desktop: {
				title: 'Desktop',
				systems: 'macOS · Windows · Linux',
				blurb:
					'A native app for when the wallet is something you keep open. Same address, same passkey, same chains.',
				stores: 'Microsoft Store'
			},
			mobile: {
				title: 'Mobile',
				systems: 'iOS · Android',
				blurb:
					'Sign with Face ID or a fingerprint on the device your passkey already lives on. The builds are in real-device testing.',
				stores: 'App Store · Google Play'
			},
			extension: {
				title: 'Browser extension',
				systems: 'Chrome · Edge · Brave',
				blurb:
					'Puts the wallet in your toolbar and lets dApps talk to it directly — no pairing step, no second device.',
				stores: 'Chrome Web Store'
			}
		},
		/**
		 * The honest version of "please buy it from the store". A store download
		 * is paid and it is how the project is funded; building from source is
		 * free and always will be. Saying both is the same posture the pricing
		 * section already takes — you pay for convenience, not access.
		 */
		fundingNote:
			'Store downloads are paid, and they are how a small independent team funds building Vela in the open. The code is public, so you can always build it yourself instead — same app, no charge.',
		storeNote:
			'Not on the stores yet. Until then, the desktop apps and the extension download from this page, ready to install. The phone apps do not: they come from the stores, or from the source.',
		/**
		 * Direct downloads (spec 065). The page used to send people to a GitHub
		 * page listing fourteen files and ask them to know which was theirs.
		 *
		 * The platform labels are written for someone who does not know the word
		 * "aarch64" — the parenthesis names the thing they DO know (the chip's
		 * brand, the distribution). Product and distribution names stay as they
		 * are in every language.
		 */
		downloads: {
			/** On the column that matches the visitor's system. A guess, so it only highlights — every other column stays as reachable. */
			yourSystem: 'Your system',
			versionLine: 'Version {{version}}',
			comingShortly: 'Coming shortly',
			unavailable:
				'That download is not up yet — it usually follows a new release within a few hours. Everything else here is ready.',
			windowsNote:
				'Windows will say it "protected your PC": the installer is not code-signed yet. Choose "More info", then "Run anyway".',
			verify: 'Checksums and source on GitHub',
			/** The row label beside the Windows architecture chips. */
			installer: 'Installer',
			/** The three Mac chips. "x64" and "Arm64" are not here: they are the same in every language. */
			chips: {
				appleSilicon: 'Apple silicon',
				intel: 'Intel chip',
				universal: 'Universal'
			},
			/** The full name of each download — what a screen reader hears for a chip that shows only "Arm64". */
			labels: {
				'macos-arm64': 'macOS — Apple silicon (M1 and later)',
				'macos-x64': 'macOS — Intel',
				'macos-universal': 'macOS',
				'windows-x64': 'Windows',
				'windows-arm64': 'Windows — ARM (Snapdragon, Surface Pro X)',
				'linux-deb-x64': 'Linux — Ubuntu, Debian, Mint (.deb)',
				'linux-deb-arm64': 'Linux — Ubuntu, Debian, Mint (.deb, ARM)',
				'linux-rpm-x64': 'Linux — Fedora, openSUSE (.rpm)',
				'linux-rpm-arm64': 'Linux — Fedora, openSUSE (.rpm, ARM)',
				'linux-flatpak-x64': 'Linux — any distribution (Flatpak)',
				'linux-flatpak-arm64': 'Linux — any distribution (Flatpak, ARM)',
				extension: 'Browser extension'
			},
			extension: {
				action: 'Download the extension',
				/** A zip a person does not know how to load is not a download (A2). */
				stepsTitle: 'Then, until it is on the Chrome Web Store:',
				steps: [
					'Unzip it into a folder you will keep — the browser runs the extension from there.',
					'Open chrome://extensions and turn on "Developer mode".',
					'Click "Load unpacked" and choose that folder.'
				]
			}
		}
	},
	/**
	 * Chain setup — the page an operator uses to find out whether Vela can run
	 * on their chain, and to put the missing pieces there if it can.
	 *
	 * The order of the copy is the order of the page: verdict first, then what
	 * is missing, then only the steps that lead somewhere. A person should be
	 * able to read the first card and stop.
	 */
	chainSetup: {
		meta: {
			title: 'Set up a chain for Vela',
			description:
				'Check whether an EVM chain has everything Vela needs — the eleven contracts and the P-256 precompile — and deploy what is missing.'
		},
		heading: 'Set up a chain for Vela',
		lede: 'Vela runs on any EVM chain that has eleven known contracts and one precompile. Enter a chain and this page will tell you which of those it has, which it lacks, and who can put them there.',
		input: {
			label: 'Chain, by name, ID or symbol — or an RPC URL',
			placeholder: 'e.g. Arc, 5042, USDC — or https://rpc.example.org',
			action: 'Check',
			suggestions: 'Matching chains',
			hint: 'Names, IDs and gas-coin symbols are looked up in the chain directory. An RPC URL is used as given, so this works for a private or local chain too.'
		},
		resolving: 'Finding endpoints…',
		checking: 'Checking the chain…',
		rpcUsed: 'Reading from {{host}} · {{ms}} ms',
		changeRpc: 'Change endpoint',
		recheck: 'Check again',
		errors: {
			'not-a-chain': 'Enter a numeric chain ID, or an RPC URL starting with https://.',
			'no-rpc': 'No public endpoint is known for this chain. Paste an RPC URL instead.',
			'rpc-unreachable':
				'None of the endpoints answered for this chain. Paste an RPC URL that does.',
			unknown: 'Something failed while checking. Try again.'
		},
		verdict: {
			ready: {
				title: 'Vela works here',
				body: 'Every contract Vela needs is deployed and the P-256 precompile answers. Add this chain in the wallet and it will work — same address as everywhere else.',
				action: 'How to add it in Vela'
			},
			needsSetup: {
				title: '{{count}} of 11 contracts missing',
				body: 'The P-256 precompile is present, so this chain can be made ready. The steps below are exactly what is missing, in the order they have to happen, each with who can do it.'
			},
			blocked: {
				title: 'Vela cannot run here',
				body: 'This chain has no P-256 verifier at 0x100 (RIP-7212). That is not something anyone can deploy: the address is part of how every Vela address is derived, so a different verifier would mean different addresses on every chain, for everyone. Only the chain itself can add the precompile.',
				link: 'What RIP-7212 is'
			},
			provisional:
				'Some reads did not answer, so this verdict is provisional. Try another endpoint or check again.',
			mismatch:
				'Something else is deployed at this address. Vela cannot use this chain until the chain resolves that; nothing here can be deployed over it.'
		},
		checklist: {
			heading: 'What Vela needs',
			p256: 'P-256 precompile (RIP-7212)',
			p256What:
				'Verifies passkey signatures. Native on this chain, or a contract at 0x100 — either works.',
			present: 'Present',
			missing: 'Missing',
			unknown: 'Could not read',
			mismatch: 'Wrong contract'
		},
		plan: {
			heading: 'Set up',
			lede: 'Two of the contracts are deployed by broadcasting a transaction that was signed once, years ago, with no key — anyone can send it after funding its sender. One can only be deployed by Safe. The rest go through a factory, from a throwaway key made in this browser.',
			who: {
				anyone: 'Anyone — including you, now',
				safe: 'Only Safe',
				deployer: 'The throwaway key below'
			},
			fund: {
				sendTo: 'Send exactly {{amount}} {{symbol}} to',
				balance: 'Has {{amount}} {{symbol}}',
				waiting: 'Waiting for the funds to arrive…',
				ready: 'Funded — ready to broadcast',
				action: 'Broadcast',
				why: 'The transaction is pre-signed and pays a fixed fee; the sender address has no owner, so anything beyond that amount is lost. Send exactly the amount.'
			},
			external: {
				body: 'Safe signs a fresh deployment for each chain from an address they hold. Open a request, then come back and check again once it lands.',
				action: 'Request it from Safe',
				guide: 'How Safe adds a chain',
				blocked: 'Waiting on this before the {{count}} Safe contracts below can be deployed.'
			},
			create2: {
				blockedBy: 'Needs {{name}} first',
				action: 'Deploy',
				all: 'Deploy all {{count}}',
				deploying: 'Deploying…'
			},
			deployer: {
				heading: 'Throwaway deployer',
				body: 'A key generated in this browser, kept in this browser, for these deployments only. Fund it with a little gas; sweep the leftover back when you are done.',
				create: 'Create a deployer key',
				address: 'Address',
				balance: 'Balance',
				estimate: 'About {{amount}} {{symbol}} should cover the remaining steps',
				export: 'Save the key',
				sweep: 'Sweep leftover back',
				sweepTo: 'Your address',
				swept: 'Sent — the leftover is on its way.',
				nothingToSweep: 'Nothing worth sweeping.',
				forget: 'Forget this key',
				forgetWarn: 'Only after sweeping: a forgotten key with funds on it is money gone.',
				keyWarn:
					'This key is stored in your browser. Anyone with it can spend what is on it — which should only ever be the gas you sent for this page.'
			},
			status: {
				sending: 'Sending…',
				confirming: 'Confirming…',
				done: 'Deployed',
				failed: 'Failed',
				view: 'View transaction'
			},
			failures: {
				'insufficient-funds': 'Not enough gas on the sender. Top it up and try again.',
				'already-sent': 'A transaction is already pending. Wait for it, then check again.',
				nonce: 'An earlier transaction from this key is still pending. Wait, then retry.',
				reverted: 'The transaction reverted. Check it on the explorer.',
				'not-mined': 'Not mined within two minutes. It may still land — check again in a moment.',
				'no-code': 'The transaction was mined but no code appeared. Check it on the explorer.',
				'blocked-by-factory': 'Its factory is not deployed yet.'
			},
			complete: {
				title: 'All set',
				body: 'Every contract is now on this chain, and the verdict above says so. Add the chain in Vela — same address as everywhere else — and sweep the leftover gas back to yourself.'
			}
		},
		copy: 'Copy',
		copied: 'Copied',
		about: {
			heading: 'What this page checks, and why',
			items: [
				{
					q: 'Why these eleven contracts?',
					a: 'They are the exact set the wallet itself checks before it lets you add a network: the account contract and its factory, the ERC-4337 EntryPoint and Safe’s module for it, the passkey signer, and the two CREATE2 factories everything else is deployed through. The list on this page is read from the wallet’s source, so the two cannot disagree.'
				},
				{
					q: 'Why is the same address used on every chain?',
					a: 'Every contract here is deployed to a deterministic address — either a keyless transaction whose sender is fixed, or CREATE2 through a factory. Because the addresses are the same everywhere, a Vela account has the same address on every chain, and a passkey created once works on all of them.'
				},
				{
					q: 'Why can’t the P-256 precompile be deployed?',
					a: 'The verifier’s address, 0x100, is written into the setup data every Vela address is derived from. Pointing at a verifier contract somewhere else would change every address on every chain, for everyone. So a chain either has RIP-7212 or Vela cannot run on it — and only the chain’s operators can change that.'
				},
				{
					q: 'Is the gas I send refundable?',
					a: 'For the two keyless deployments, no: their sender has no owner, and the pre-signed transaction spends a fixed fee. Send exactly what the step says. For the throwaway deployer, yes — sweep the leftover back to yourself when you are done.'
				}
			]
		}
	}
} as const;

/** The shape of a complete catalog. Translations are checked against this. */
export type Messages = typeof en;

/** A translation may be partial; completeness is evaluated per page namespace. */
export type PartialMessages = DeepPartial<Messages>;

type DeepPartial<T> = T extends readonly (infer U)[]
	? readonly DeepPartial<U>[]
	: T extends object
		? { [K in keyof T]?: DeepPartial<T[K]> }
		: T;

export default en;
