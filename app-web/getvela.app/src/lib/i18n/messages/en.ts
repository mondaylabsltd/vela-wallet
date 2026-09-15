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
 * What does NOT belong here: proper nouns. "GitHub", "Telegram", "WalletPair",
 * "Vela Relay", "RSS" are names, not words — they stay in the components, where
 * no translator will be tempted to localize them.
 *
 * `chrome` and `notice` are special: they must be COMPLETE in all fifteen
 * locales at all times, because they are what an untranslated page is wrapped
 * in. The one sentence a reader of a fallback page must be able to read is the
 * one telling them the rest is English — it cannot itself fall back.
 */

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
			createWallet: 'Create wallet',
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
	}
} as const;

/** The shape of a complete catalog. Translations are checked against this. */
export type Messages = typeof en;

/** A translation may be partial; completeness is evaluated per page namespace. */
export type PartialMessages = DeepPartial<Messages>;

type DeepPartial<T> = T extends object ? { [K in keyof T]?: DeepPartial<T[K]> } : T;

export default en;
