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
 * Proper nouns. "GitHub", "Telegram", "WalletPair", "MetaMask", "Safe", "Vela
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
				security: 'Security',
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
				'An open-source, self-hostable Ethereum wallet for ETH & ERC-20s. Sign with a passkey — no seed phrase, no hardware key, no lock-in. You pay for convenience, not access.',
			ogTitle: 'Vela Wallet — An Ethereum wallet you actually own',
			ogDescription:
				'Open-source, self-hostable wallet for ETH & ERC-20s. Passkey signing, no seed phrase, no lock-in. Compile it yourself if you want to.',
			/** schema.org Organization description (FR-024). */
			organization:
				'An open-source, self-hostable Ethereum wallet for ETH and ERC-20 tokens. Sign with a passkey — no seed phrase, no hardware key, no lock-in.'
		},

		hero: {
			/**
			 * Founder-approved. The headline is verbatim from
			 * specs/059/approved-copy.md; the subtitle was cut down to its first
			 * sentence on 2026-09-15 — the rest of it ("open source and
			 * self-hostable… even if we disappear") became a fact on the right,
			 * where it can be a hook instead of a clause.
			 */
			headline: 'An Ethereum wallet you actually own',
			subtitle: 'Sign with a passkey Vela never sees.',
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
			 */
			facts: [
				{
					term: 'An unmodified Safe v1.4.1.',
					link: "Your account is Safe's contract, not one of ours"
				},
				{
					term: "Don't bet the wallet on one device.",
					link: 'Up to seven keys, set at creation — hardware keys included'
				},
				{
					term: 'One transaction on the screen. Another one signed.',
					link: 'How Bybit lost $1.5B, and the path we close'
				},
				{
					term: 'If Vela disappears, your wallet does not.',
					link: 'Self-host the app, the relay and every backend service'
				}
			]
		},

		/**
		 * The four hooks above get a screen of their own, and a screen needs to
		 * say what it is. One line, and deliberately not a heading: the claims are
		 * the loud part, this is the caption under the exhibit.
		 */
		facts: {
			lede: 'Four things worth checking before a wallet holds your money.'
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
		 */
		why: {
			heading: 'Why we built it',
			p1: 'We used <a href="https://account.base.app" target="_blank" rel="noopener">Base Account</a> every day and liked it. Then we hit the walls: a recovery key generated in a browser you just have to trust, no custom networks, nothing you can host yourself — and if the service goes away, the wallet goes with it.',
			p2: 'Vela is the version we were willing to keep money in. Same passkey sign-in. None of the lock-in.',
			more: 'The long version — where are you supposed to keep twelve words?'
		},

		/**
		 * Immediately after the pitch, on purpose. Every one of these is a reason
		 * somebody should NOT use Vela today, written before they deposit rather
		 * than discovered after.
		 */
		tradeoffs: {
			heading: 'The trade-offs, up front',
			lede: 'Three reasons not to use Vela — while your money is still somewhere else.',
			items: [
				{
					title: 'Every transaction pays a fee that keeps the relay running.',
					body: 'A passkey signature is verified on-chain and the transaction goes through the ERC-4337 EntryPoint, which on its own costs more gas than a plain transfer. The relayer fronts that gas and submits the transaction, and is paid more than the gas actually costs on-chain — the difference is what runs the service. You see the total before you sign, and it is written into the transaction you sign: what you signed is what is charged, and it cannot move afterwards. The relayer is swappable in settings, or <a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener">run your own</a>.'
				},
				{
					title: 'Whatever holds your key is what you have to trust.',
					body: "A passkey either syncs with your Apple or Google account or lives on a USB/NFC security key, so one of those is what you are trusting — and hardware has had its own failures too. Afraid of losing a device: set several keys when you create the wallet, on different devices. Afraid of theft: anyone who can sign in to that Apple or Google account and unlock a device can spend your money, so the account has to be held to the same standard as the wallet. Afraid one of them lets you down: don't put every key in the same place — the first key can already be a security key, and a synced passkey plus a hardware key means no single vendor can lock you out. But be clear about what each extra key is: one more way back into the wallet, and one more thing that can be stolen — any single key signs on its own."
				},
				{
					title: 'You are trusting audited Safe contracts — and an audit is not a guarantee.',
					body: 'Your account is Safe v1.4.1 plus the WebAuthn signer module: audited by third parties, deployed at their canonical addresses, not one line changed by us. But an audit covers one version of one piece of code; it lowers the risk, it does not promise nothing will go wrong — and on-chain there is no support desk to roll anything back. The app code around those contracts is ours: public, readable, and not independently audited; we want that and have not scheduled it. <a href="/docs/security-audits">Audits &amp; known issues</a> lists every contract, every report and what we are watching. The software is provided as is, without warranty: the risk of what you put in is yours.'
				}
			],
			close: 'If one of those is a dealbreaker, Vela is not for you yet.'
		},

		/**
		 * The signing story, in two halves that must not be confused: what the
		 * wallet does today (shipped, checkable) and the independent signer
		 * (built and tested, not deployed). The `next.label` carries that second
		 * fact and is not decoration — see the founder ruling of 2026-09-15.
		 */
		signing: {
			heading: 'Sign what you see.',
			lede: 'Bybit lost $1.5 billion in February 2025 because the people signing approved a transaction whose on-screen summary did not match what was actually signed. Closing that gap is the entire job of a signing screen.',
			today: {
				label: 'Today',
				title: 'Every transaction is decoded before you approve it.',
				body: 'What it does, how much, and to whom — in plain language instead of hex, using <a href="/docs/clear-signing">ERC-7730</a> descriptors. An approval that would grant an unlimited allowance is rewritten to a finite amount you pick, and a last check refuses to submit one that is still unlimited. When a call cannot be decoded, Vela says so instead of showing a friendly summary it cannot stand behind.'
			},
			next: {
				label: 'Built and tested — not live yet',
				title: 'Next: a signing path with nothing to trust.',
				body: 'A second, independent way to sign the transactions you would rather check by hand: a zero-dependency static page — and a Chrome extension built from the same folder — that decodes the raw transaction itself and shows you the real calldata before your passkey signs it. No framework, no bundler, no network calls. It stays optional, because it is slower. And you will not have to use ours: deploy the page yourself, or run it from localhost.'
			},
			aside: {
				title: 'What it is for',
				items: [
					'Moving an amount you would not want to be wrong about.',
					"Approving a contract you haven't used before.",
					"Any time a dApp's own interface is the only thing telling you what you are signing."
				]
			}
		},

		compare: {
			heading: 'How Vela compares',
			desc: "Against the two wallets you'd actually consider instead.",
			rows: [
				{
					feature: 'Signing key lives in',
					vela: 'Apple / Google Password Manager',
					base: 'Apple / Google Password Manager',
					safe: 'Apple / Google Password Manager'
				},
				{
					feature: 'Signers',
					vela: '1-of-n: passkeys and hardware security keys, up to seven',
					base: 'Passkeys',
					safe: 'Any owner, m-of-n'
				},
				{
					feature: 'Independent sign-what-you-see path',
					vela: 'Zero-dependency page / extension, self-deployable — in testing',
					base: 'No',
					safe: 'Third-party hash-check tools'
				},
				{
					feature: 'Account contract',
					vela: '<a href="/docs/security-audits">Safe v1.4.1, unmodified</a>',
					base: 'Custom, audited',
					safe: 'Safe'
				},
				{
					feature: 'Open source',
					vela: '<a href="https://github.com/orgs/mondaylabsltd/repositories" target="_blank" rel="noopener">App + relayer + every backend service</a>',
					base: 'Partial',
					safe: 'App only; relies on Safe infrastructure'
				},
				{
					feature: 'Self-hostable end to end',
					vela: 'Yes',
					base: 'No',
					safe: 'Frontend only'
				},
				{
					feature: 'Custom networks',
					vela: 'Any chain with RIP-7212 — the <a href="https://biubiu.tools/apps/vela-wallet-chain-setup" target="_blank" rel="noopener">setup tool</a> deploys the contracts',
					base: 'Base-first, few',
					safe: 'Safe-supported chains only'
				},
				{
					feature: 'Platforms',
					vela: 'Web today; desktop, mobile and an extension from the same code, in testing',
					base: 'Web, mobile',
					safe: 'Web, mobile'
				},
				{
					feature: 'Recovery key generated in a browser',
					vela: 'No',
					base: 'Yes',
					safe: 'No'
				},
				{
					feature: 'Keeps signing if the vendor disappears',
					vela: 'Yes — self-host, plus an open-source domain-recovery extension',
					base: 'No',
					safe: 'Depends on the Safe transaction service'
				},
				{
					feature: 'Built for',
					vela: 'People who trust passkeys and want out of vendor lock-in',
					base: 'The Base ecosystem',
					safe: 'Multisig and teams'
				}
			],
			note: "Everything Vela builds, you can self-host. A few data sources — some chains' history, long-tail prices, threat scanning — come from third-party providers; swap in your own node or key."
		},

		pricing: {
			heading: 'Free and open. Pay only if you want to.',
			intro:
				"You're paying for convenience, not for access. Every piece is open source and self-hostable, so none of it is a gate.",
			cards: [
				{
					title: 'Web wallet',
					price: 'Free',
					body: "Open it, authenticate, done — nothing to install and nothing to pay. Host the same app yourself if you'd rather not use ours."
				},
				{
					title: 'Desktop & mobile apps',
					price: 'One-time purchase',
					body: 'In the app stores they will be a one-time purchase — never a subscription. It is how a small independent team funds building this in the open. They are open source too: build from source and install it yourself for free. Until they reach the stores, every build is on <a href="https://github.com/mondaylabsltd/vela-wallet/releases" target="_blank" rel="noopener">GitHub</a>.'
				},
				{
					title: 'Transactions',
					price: 'Network gas + relayer fee',
					body: 'A relayer fronts the gas and submits your transaction, and is paid the network cost plus a service fee — quoted before you sign, locked into what you sign, with a small minimum on very cheap transactions. Point Vela at <a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener">your own relay</a> and you pay us nothing.'
				}
			],
			note: 'No token, no ads, nothing sold about you. One more cost worth knowing: on each network your wallet needs a small, non-refundable deposit to activate its gas relayer account — Vela sponsors that for new wallets where it can. <a href="/docs/networks-and-fees">Networks &amp; fees</a> has the details.'
		},

		how: {
			heading: 'How Vela works',
			desc: 'Three steps. Nothing to write down.',
			steps: [
				{
					title: 'Create',
					body: 'Face ID, a fingerprint, or a security key creates a passkey. Your address is derived from it — the same address on every chain. Nothing to pay up front: the contract deploys with your first transaction.'
				},
				{
					title: 'Sign',
					body: 'Vela sends your device a challenge. The device returns a signature — never the key — and a relayer puts the signed operation on-chain.'
				},
				{
					title: 'Sign in anywhere',
					body: 'New device, same Apple or Google account: the passkey syncs, and the wallet is there. No seed phrase to import, no recovery key to type.'
				}
			],
			stack: {
				label: 'Under the hood',
				link: 'Read the whitepaper'
			}
		},

		networks: {
			heading: '12 networks built in. One address on all of them.',
			body: 'Add any other EVM chain that has the RIP-7212 P-256 precompile and Vela\'s Safe + ERC-4337 contracts deployed — Vela checks before it lets you add one. The <a href="https://biubiu.tools/apps/vela-wallet-chain-setup" target="_blank" rel="noopener">Chain Setup tool</a> can deploy them on a chain that has neither, including your own local testnet.',
			link: 'Networks & fees, in detail'
		},

		faq: {
			heading: 'FAQ',
			desc: "Roughly in the order people ask them — starting with what it is, ending with what happens if we're gone.",
			items: [
				{
					q: 'Do I need a seed phrase, an extension, or a hardware wallet?',
					a: 'None of the three. You create the wallet with Face ID, a fingerprint, or a security key, and that passkey is the signer. There is nothing to write down, and the web wallet runs in a browser with nothing to install.'
				},
				{
					q: 'Is it really self-custodial? What can Vela do to my money?',
					a: 'Nothing. Your wallet is a <a href="/docs/security-audits">Safe smart account</a> that only your passkey can operate, and the passkey never leaves your device — Vela sends a challenge and gets back a signature, never a key. We cannot move, freeze, or restore your funds. Not "we promise not to": there is no code path that could.'
				},
				{
					q: 'What if I lose my phone?',
					a: 'If your passkey syncs through iCloud Keychain or Google Password Manager, sign in on a new device with the same account and the wallet is there — same address, same assets, same chains. If you turned that sync off, the passkey only ever existed on the device you lost, and a second signer is what covers that case. Details in <a href="/docs/recovery">Recovery &amp; sign-in</a>.'
				},
				{
					q: 'Can I add a second key — another device, or a YubiKey?',
					a: 'Yes, and it is worth doing. A wallet can hold up to seven signers, any one of which signs on its own: passkeys on different devices, a nearby phone you scan, or a USB/NFC security key. One catch, and it matters — your address is derived from the full set of keys, so they are chosen <strong>when you create the wallet</strong>. Adding one later would be a different wallet at a different address.'
				},
				{
					q: 'What if I delete the passkey?',
					a: 'Then that key is gone, and if it was your only signer, so is access — no seed phrase, no support reset, no back door, for anyone including us. This is the sharpest edge in the design, and the answer to it is a second signer chosen at setup. If you ever tidy up your password manager, know what each passkey is for before you remove it.'
				},
				{
					q: 'What if my Apple or Google account is compromised?',
					a: 'Someone who gets into that account and can unlock a device it syncs to can use the passkey it holds. Turn on two-factor authentication there and use a unique password. If you would rather your wallet did not depend on that account at all, create it with a hardware security key as the signing method — that choice is available for the first key, not only for extra ones.'
				},
				{
					q: 'What does it cost to use?',
					a: 'The wallet is free and there is no token. You pay network gas out of your own balance plus the relayer\'s fee. Smart-account signing costs more gas than a plain transfer — a P-256 signature is verified on-chain and the call goes through the ERC-4337 EntryPoint — and the relayer that fronts that gas is paid more than the gas costs on-chain; the difference is what runs the service. The total is quoted before you sign and written into the transaction you sign, so what you signed is what is charged. Each network also needs a small, non-refundable deposit to activate its gas relayer account, which Vela sponsors for new wallets where it can. The relayer is swappable in settings, or <a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener">run your own</a>. Full breakdown in <a href="/docs/networks-and-fees">Networks &amp; fees</a>.'
				},
				{
					q: 'Which chains and tokens does it hold?',
					a: 'Native coins and ERC-20s on the twelve built-in chains — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Monad, World Chain and Tempo — plus any EVM chain you add yourself that meets the requirements above. Your address is the same on all of them.'
				},
				{
					q: 'Can I use it with dApps?',
					a: 'Yes. Pair your wallet with the <a href="https://walletpair.org/" target="_blank" rel="noopener">WalletPair</a> extension and sign for desktop dApps with your passkey — with the same decoded confirmation screen you get everywhere else in the wallet.'
				},
				{
					q: 'Has the code been audited?',
					a: 'The contracts have: Safe v1.4.1, the Safe 4337 module and the WebAuthn signer module were audited by third parties, and Vela deploys them unmodified. Vela\'s own app code has not been independently audited and no audit is scheduled — what stands in for one today is that all of it is <a href="https://github.com/mondaylabsltd/vela-wallet" target="_blank" rel="noopener">public</a>. Which contracts, which reports, which known issues: <a href="/docs/security-audits" data-rybbit-event="audits_open" data-rybbit-prop-location="faq">Audits &amp; known issues</a>.'
				},
				{
					q: 'What happens if Vela shuts down, or getvela.app goes offline?',
					a: "Your wallet is a Safe contract on-chain and does not depend on our servers. The app and every backend service — chain data, passkey index, relayer — are open source, so you can run your own. The one thing that needs care is the passkey: it is bound to the getvela.app domain, so signing goes through Vela's own open-source code — your self-hosted instance, or the open-source recovery extension that lets an existing passkey be used from another domain or localhost."
				}
			]
		}
	},

	about: {
		meta: {
			title: 'About',
			description:
				'The team and mission behind Vela Wallet — a self-custodial, open-source wallet with no seed phrase, built in the open.',
			/** schema.org AboutPage name. */
			pageName: 'About Vela Wallet'
		},
		eyebrow: 'About',
		heading: 'Who builds Vela.',
		lede: 'Vela is built in the open — the wallet, the smart contracts, and this very site. No faceless company behind it: just real code you can read, and a real person you can reach.',
		team: {
			/** The person's name is a proper noun and stays in the component. */
			role: 'Founder & Engineer',
			bio: 'Builds Vela end to end — the wallet, the contracts, and this site. Writing about the process as it happens.'
		},
		valuesHeading: 'What we believe',
		values: [
			{
				title: 'Self-custody, for real',
				body: 'Your keys, your coins — not a slogan but the architecture. We cannot move, freeze, or recover your funds, and we built it that way on purpose.'
			},
			{
				title: 'No seed phrases',
				body: 'The biggest cause of lost crypto is a string of words people were told to guard perfectly. We replaced it with a passkey: your face or fingerprint.'
			},
			{
				title: 'Open source',
				body: 'The wallet is public on GitHub. Trust should be verifiable, not asked for. Read the code, or follow along as we build it in the open.'
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
				title: 'See every coin you receive',
				body: 'A plain native-coin deposit — or coins that arrive through an internal call (an exchange withdrawal, a router, a multisig) — emits no on-chain log, so it can\u2019t show in your activity on most networks today. We\u2019re building a transfer service that traces blocks to surface these, so every deposit appears, on every chain.'
			},
			{
				title: 'Wider clear-signing coverage',
				body: 'More contracts and chains shown as human-readable intent, so fewer transactions fall back to blind signing.'
			},
			{
				title: 'Native iOS & Android apps',
				body: 'Vela runs on the web today; the mobile builds share the same code and are in real-device testing ahead of an App Store and Google Play release.'
			},
			{
				title: 'Sync across all your devices',
				body: 'On iOS and Android your accounts and networks already follow you through your platform\u2019s backup; on the web they stay in the browser. Next: your language, currency and formatting, plus one-tap restore of your whole setup on a new device — and a saved address book so you stop re-pasting addresses.'
			},
			{
				title: 'Reach further',
				body: 'DApp Connect from the desktop without your phone, more EVM networks (including a signing path for chains without the P-256 precompile), and an independent security audit of Vela\u2019s Safe + WebAuthn integration.'
			}
		],
		shipped: [
			{
				title: 'Localization & everyday polish',
				body: 'Multi-language support with instant switching (Russian and Italian added; 15 languages today), local currency and locale-aware formatting, a dynamic amount display, branded pull-to-refresh, pending-until-confirmed sends, and one-tap in-app feedback.'
			},
			{
				title: 'Payment-first home',
				body: 'The home screen rebuilt around your activity and balances.'
			},
			{
				title: 'Clear Signing (ERC-7730)',
				body: 'Transactions show what they actually do — amount, recipient, intent — in plain language instead of raw hex, with a preview harness and tests.'
			},
			{
				title: 'WalletPair dApp connect',
				body: 'End-to-end-encrypted pairing so you can sign for desktop dApps from your wallet.'
			},
			{
				title: 'dApp signing flow',
				body: 'Connection infrastructure and the signing-request experience.'
			},
			{
				title: 'The core wallet experience',
				body: 'A real design system, gas-tier selection and a redesigned confirm screen, a fullscreen QR scanner, and rebuilt receive, token, add-token and deposit screens.'
			},
			{
				title: 'Vela is born',
				body: 'The wallet launches on the web, from a single codebase that also builds for iOS and Android — Safe smart accounts (ERC-4337), passkey sign-in, and no seed phrase, from day one.'
			}
		]
	},

	getStarted: {
		meta: {
			title: 'Get Vela',
			description:
				'Vela runs in your browser today, with desktop, mobile and a browser extension built from the same code. Pick the one you want.'
		},
		heading: 'Get Vela',
		lede: 'One wallet, one address, built from one codebase. Start in your browser — nothing to install — or take the same wallet to your desktop, your phone, or your browser toolbar.',
		recommended: 'Recommended',
		availableNow: 'Available now',
		comingSoon: 'Coming soon',
		githubCta: 'Download from GitHub Releases',
		platforms: {
			web: {
				title: 'Web',
				blurb:
					'Nothing to install and nothing to update. Open it, authenticate with your passkey, and your wallet is there — on any device with a recent browser.',
				action: 'Open the web wallet'
			},
			desktop: {
				title: 'Desktop',
				systems: 'macOS · Windows · Linux',
				blurb:
					'A native app for when the wallet is something you keep open. Same address, same passkey, same chains.',
				stores: 'Mac App Store · Microsoft Store'
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
			'Store downloads are paid, and they are how a small independent team funds building Vela in the open. Everything is open source, so you can always build it yourself instead — same app, no charge.',
		storeNote: 'Not on the stores yet. Until then, every build is on GitHub.'
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
