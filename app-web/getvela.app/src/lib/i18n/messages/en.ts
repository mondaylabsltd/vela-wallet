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
			whyVela: 'Why Vela',
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
			/** Founder-approved, verbatim. See specs/059/approved-copy.md. */
			headline: 'An Ethereum wallet you actually own',
			subtitle:
				'Sign with a passkey Vela never sees. Open source and self-hostable on most EVM chains — so it keeps working even if we disappear.',
			ctaCreate: 'Getting started',
			ctaCode: 'Read the code',
			facts: [
				{
					term: 'Safe v1.4.1, unmodified',
					detail: 'Third-party audited contracts, deployed exactly as published.'
				},
				{
					term: '12 chains built in, plus your own',
					detail:
						"Any EVM chain with the RIP-7212 precompile and Safe v1.4.1's contracts deployed. One address on every chain."
				},
				{
					term: '1-of-n signers, security keys included',
					detail:
						'Passkeys, a nearby device, or a USB/NFC key — up to seven, any one of which signs. Chosen when you create the wallet.'
				}
			]
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

		doesLess: {
			heading: 'A wallet that does less — on purpose.',
			p1: 'No NFT gallery. No built-in swaps. No DeFi dashboard. Nothing engineered to pull you toward the next thing to click.',
			p2: 'Vela holds ETH and ERC-20s. When you want to use a dApp, you connect to the one you choose through <a href="https://walletpair.org/" target="_blank" rel="noopener">WalletPair</a>.',
			p3: "That's the whole product.",
			p4: 'Because every extra feature inside a wallet is more code to trust and more UI standing between you and your money. Vela stays small on purpose: fewer paths to attack, fewer moving parts to audit, and fewer chances to make a bad click.'
		},

		why: {
			heading: 'Why we built Vela',
			p1: "We didn't set out to build another wallet. We started with a question we could never answer cleanly:",
			beat: 'Where are you supposed to keep twelve words?',
			p2: 'Put them in Notes and you\'re one stolen phone away from trouble. Write them on paper, and now you\'re thinking about fire, water, moving apartments, roommates, trash bags, and whether future-you will remember where "the safe place" was. The honest answer, for a lot of people, is a screenshot in the camera roll. Everyone knows it\'s wrong. They do it anyway — because the "right" answer is too hard to live with.',
			p3: 'Then passkeys changed what a wallet could feel like. We used <a href="https://account.base.app" target="_blank" rel="noopener">Base Account</a> every day, and signing with Face ID felt obvious in a way seed phrases never did — less like handling hazardous material, more like using the rest of the internet. But the more we used it, the more we hit edges we couldn\'t ignore: a recovery key generated in a browser that you just had to trust, no custom networks, no way to host it ourselves. And the quiet problem was the biggest one — if the service disappeared, the wallet disappeared with it.',
			p4: 'So we built the version we wanted to depend on.',
			p5: 'Vela is <strong>a passkey wallet you can fully own.</strong> Your passkey stays where your device already protects it — iCloud Keychain or Google Password Manager. When you sign a transaction, Vela sends a challenge to your device; your device signs it and sends back just the signature. Vela never sees the key itself. Most wallets still have a dangerous moment, even if it\'s brief: words on a screen, a seed phrase in memory, a recovery key sitting in a browser tab. Vela is designed so that moment never exists. <strong>We can\'t access your keys. Not "we promise not to" — we architecturally can\'t.</strong>',
			p6: `We made Vela open source so you can check that for yourself, and self-hostable so your wallet never depends on our company staying online. And we built on unmodified <a href="${SAFE_REPO}" target="_blank" rel="noopener">Safe contracts</a> because the boring, battle-tested path is the right one when people's money is involved — the same contracts already securing billions on-chain.`,
			p7: "There's still a trade-off. With Vela, your Apple or Google account matters, because that's where your passkey lives. Lose that account, or delete the passkey, and there's no seed phrase, no support reset, no back door. But every self-custodial wallet asks you to choose which risk you'd rather live with. A seed phrase can be copied, screenshotted, phished, or typed into the wrong site at 1 a.m. A passkey is different: there are no words to reveal, no secret to paste, and no fake site that can trick you into handing it over. Your device signs for the real domain, or it does not sign.",
			p8: "That's why we built Vela — a wallet with no seed phrase to hide, no recovery key to trust, and no company you have to hope will stay around forever."
		},

		compare: {
			heading: 'How Vela compares',
			desc: 'The differences that matter once you actually own your keys.',
			rows: [
				{
					feature: 'Where your signing key lives',
					vela: 'Apple / Google Password Manager',
					metamask: 'In the app',
					base: 'Apple / Google Password Manager'
				},
				{
					feature: 'Key ever exposed to the app?',
					vela: 'No',
					metamask: 'Yes',
					base: 'No'
				},
				{
					feature: 'Open source',
					vela: '<a href="https://github.com/orgs/mondaylabsltd/repositories" target="_blank" rel="noopener">All of it</a>',
					metamask: 'Partial',
					base: 'Partial'
				},
				{
					feature: 'Account contract',
					vela: '<a href="/docs/security-audits">Safe v1.4.1 — ecosystem standard</a>',
					metamask: 'EOA',
					base: 'Custom (audited)'
				},
				{
					feature: 'Networks supported',
					vela: '12 built-in + custom',
					metamask: 'Any EVM',
					base: 'Base-first, few'
				},
				{
					feature: 'Keeps working if the vendor disappears',
					vela: `Yes — <a href="${GITHUB}#self-deploy-service-endpoints" target="_blank" rel="noopener">self-hostable</a>`,
					metamask: 'Yes',
					base: 'Signing depends on vendor infra'
				},
				{
					feature: 'What you must back up',
					vela: 'Nothing — your passkey syncs automatically',
					metamask: 'Seed phrase',
					base: 'Recovery key'
				}
			],
			note: "You can self-host everything Vela builds. A few data sources (some chains' history, long-tail prices, threat scanning) come from third-party providers — swap in your own node or key."
		},

		how: {
			heading: 'How Vela works',
			desc: 'What happens at each step.',
			steps: [
				{
					title: 'Create a wallet',
					body: `Authenticate with Face ID or fingerprint. Your device creates a passkey and derives a <a href="${SAFE_REPO}" target="_blank" rel="noopener">Safe</a> smart account address from it — one address across all supported chains. No gas cost upfront. The contract deploys on-chain with your first transaction.`
				},
				{
					title: 'Sign a transaction',
					body: 'The app builds a transaction and sends a signing challenge to your device. Your device signs it with the passkey and sends back just the signature — Vela never sees the key. The signed transaction goes on-chain through an <a href="https://eips.ethereum.org/EIPS/eip-4337" target="_blank" rel="noopener">ERC-4337</a> bundler.'
				},
				{
					title: 'Sign in on a new device',
					body: 'Get a new phone, sign in with the same Apple or Google account. Your passkey syncs automatically through iCloud Keychain or Google Password Manager. Same address, same assets, same chains — no seed phrase to import, no recovery key to enter.'
				}
			],
			tech: {
				heading: 'Technical details',
				wallet: 'Wallet',
				authentication: 'Authentication',
				accountType: 'Account type',
				accountTypeValue: '(Smart Account)',
				signerModule: 'Signer module',
				networks: 'Networks',
				networksValue: '12 EVM chains (+ custom)',
				sourceCode: 'Source code'
			},
			networkNote:
				'Custom networks need more than EVM compatibility — the chain must have the RIP-7212 P256 precompile and Vela\'s Safe + ERC-4337 contracts deployed. Vela checks this when you add one, and the <a href="https://biubiu.tools/apps/vela-wallet-chain-setup" target="_blank" rel="noopener">Chain Setup tool</a> can deploy them on chains that don\'t — including your own local testnet.'
		},

		pricing: {
			heading: 'Free and open. Pay only if you want to.',
			intro:
				"You're paying for convenience, not access. Everything is open source and self-hostable — nothing locks you in.",
			cards: [
				{
					title: 'Web wallet',
					price: 'Free',
					body: 'The web wallet is free, open source, and self-hostable. No install, no seed phrase — just authenticate and go.'
				},
				{
					title: 'Mobile app',
					price: 'Funds the project',
					body: "The mobile app, when it ships, will be a paid download, priced by region — it's how a small, independent team funds building Vela in the open. It's open source too, so you can always build it from source and install it on your own phone for free."
				},
				{
					title: 'Bundler gas fee',
					price: 'Network gas + service fee',
					body: 'Transactions go through an ERC-4337 bundler. You pay network gas plus a service fee — the exact amount is quoted and shown to you before you sign, so you always know the total up front. You can skip the fee entirely by running a compatible <a href="https://github.com/mondaylabsltd/vela-relay" target="_blank" rel="noopener">self-hosted bundler</a>.'
				}
			],
			note: "Funded by the people who use it. Don't want to pay? Use the web wallet free, self-host the services, run your own bundler — and owe us nothing."
		},

		faq: {
			heading: 'FAQ',
			desc: "What you'd want to know before putting real money in.",
			items: [
				{
					q: 'What chains does Vela support?',
					a: "Twelve chains are built in: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Monad, World Chain, and Tempo. You can add other EVM networks yourself if the chain has the RIP-7212 P256 precompile and Vela's Safe + ERC-4337 contracts deployed — Vela checks when you add one. Same wallet address across all chains."
				},
				{
					q: 'How is Vela different from Coinbase Smart Wallet or other passkey wallets?',
					a: "Most passkey wallets are closed-source and run on infrastructure you can't control. If the company pivots or shuts down, you're stuck. Vela is fully open source and self-hostable — the app, the bundler, and all backend services. You can add custom networks, run your own bundler to skip fees, and keep using your wallet even if getvela.app disappears. No recovery keys generated in a browser. No vendor lock-in."
				},
				{
					q: 'Can I use Vela with dApps?',
					a: 'Yes. Pair your phone with the <a href="https://walletpair.org/" target="_blank" rel="noopener">WalletPair extension</a> and sign transactions on desktop dApps using your phone\'s passkey.'
				},
				{
					q: 'Do I pay more gas than a regular wallet?',
					a: "Yes. Smart account transactions have extra overhead from on-chain signature verification and the ERC-4337 EntryPoint. Expect roughly 1.5–3x the gas of a standard wallet transfer, depending on the chain — plus the bundler service fee described under pricing, unless you run your own bundler. Either way, the exact total is shown before you sign. That's the cost of passkey signing, no seed phrase, and one address across all chains."
				},
				{
					q: 'What if I lose my phone?',
					a: "Your passkey is backed up through iCloud Keychain (iOS) or Google Password Manager (Android) — as long as that sync is turned on. With it on, get a new phone, sign in with the same Apple/Google account, and your wallet is right there. If you've turned that sync off, your passkey stays on your old phone only, and losing the device means losing access."
				},
				{
					q: 'What if I accidentally delete my passkey?',
					a: "It's gone — and so is access to your wallet. There's no recovery mechanism. This is irreversible. If you ever clean up your password manager, know what each passkey is for before you remove it."
				},
				{
					q: 'What if my Apple or Google account is compromised?',
					a: 'Anyone who can access your Apple/Google account and use your passkey could access your wallet. Enable two-factor authentication and use a strong, unique password — your Apple/Google account is part of your wallet security.'
				},
				{
					q: 'Can I add a second passkey as backup?',
					a: 'Yes — up to seven signers per wallet, and any one of them can sign on its own. They can be passkeys on different devices, a nearby phone you scan, or a USB/NFC security key. The catch: you choose them <strong>when you create the wallet</strong>, because your address is derived from the full set of keys — adding one later would be a different address. On top of that, each passkey is replicated by its own sync (iCloud Keychain, Google Password Manager) across your trusted devices.'
				},
				{
					q: 'What happens if Vela shuts down?',
					a: `Your wallet is a Safe smart contract on-chain — it doesn't depend on Vela's servers. The app and all backend services (chain data, passkey index, bundler) are open source, so you can deploy your own Vela interface and run your own services. Because your passkey signer is Vela-specific and bound to the getvela.app domain, you keep signing through Vela's own open-source code — your self-hosted instance plus the <a href="${RECOVERY_EXT}" target="_blank" rel="noopener">recovery extension</a> — not a generic Safe app.`
				},
				{
					q: 'What if the getvela.app domain goes offline?',
					a: `Your funds stay on-chain regardless. Since passkeys are tied to a domain, Vela provides an open-source <a href="${RECOVERY_EXT}" target="_blank" rel="noopener">recovery extension</a> that lets you use your existing passkey from another domain or localhost.`
				},
				{
					q: 'Has the code been audited?',
					a: `The Safe contracts and Safe WebAuthn signer module that Vela uses have been audited. Vela's own app code hasn't been independently audited yet — all source code is <a href="${GITHUB}" target="_blank" rel="noopener">public</a> for review. Every contract we depend on, its audit report, and the known issues we track are documented in <a href="/docs/security-audits" data-rybbit-event="audits_open" data-rybbit-prop-location="faq">Audits &amp; known issues</a>.`
				}
			]
		},

		cta: {
			heading: 'Ready to try it?',
			sub: 'The web wallet is live and free. No install, no seed phrase — just authenticate and go.',
			button: 'Create a wallet',
			cards: [
				{ title: 'Open source', body: "Every line is on GitHub. Verify, don't trust." },
				{ title: 'Self-hostable', body: 'Run your own bundler and services.' },
				{
					title: 'Battle-tested',
					body: 'Your account is an <a href="/docs/security-audits">audited</a>, unmodified Safe v1.4.1.'
				},
				{ title: 'No seed phrase', body: 'Sign with a passkey. Nothing to write down.' }
			],
			divider: 'mobile apps coming soon',
			followDesc:
				'Follow <a href="https://x.com/realvelawallet" target="_blank" rel="noopener">@realvelawallet</a> and we\'ll post the moment iOS &amp; Android go live.',
			followX: 'Follow on X',
			joinTelegram: 'Join Telegram'
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
