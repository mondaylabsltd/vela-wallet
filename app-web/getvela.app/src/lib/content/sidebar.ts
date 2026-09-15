/**
 * Docs sidebar. This is the single source of truth for docs ordering and
 * grouping (VitePress-style). Each `slug` must match a file in
 * `src/content/docs/<slug>.md`. The first item ("introduction") is served at
 * `/docs`; everything else lives under `/docs/<slug>`.
 */
export interface SidebarItem {
	slug: string;
	/** English title. The rendered title comes from `chrome.docs.titles[slug]`. */
	title: string;
}

export interface SidebarGroup {
	/** Key into `chrome.docs.groups` — the group's name is translated copy. */
	key: 'gettingStarted' | 'using' | 'security' | 'reference';
	/** English title, kept here so this file still reads as the source of truth. */
	title: string;
	items: SidebarItem[];
}

export const sidebar: SidebarGroup[] = [
	{
		key: 'gettingStarted',
		title: 'Getting Started',
		items: [
			{ slug: 'introduction', title: 'Introduction' },
			{ slug: 'why-vela', title: 'Why we built Vela' },
			{ slug: 'install', title: 'Install Vela' },
			{ slug: 'create-wallet', title: 'Create your wallet' }
		]
	},
	{
		key: 'using',
		title: 'Using Vela',
		items: [
			{ slug: 'send-and-receive', title: 'Send & receive' },
			{ slug: 'networks-and-fees', title: 'Networks & fees' }
		]
	},
	{
		key: 'security',
		title: 'Security',
		items: [
			{ slug: 'passkeys', title: 'How passkeys work' },
			{ slug: 'signers', title: 'Signers & security keys' },
			{ slug: 'clear-signing', title: 'Clear signing' },
			{ slug: 'clear-signing-self-host', title: 'Self-host the signing page' },
			{ slug: 'bybit-attack', title: 'The Bybit attack' },
			{ slug: 'recovery', title: 'Recovery & sign-in' },
			{ slug: 'account-contract', title: 'The account contract' },
			{ slug: 'security-audits', title: 'Audits & known issues' }
		]
	},
	{
		key: 'reference',
		title: 'Reference',
		items: [
			{ slug: 'whitepaper', title: 'Whitepaper' },
			{ slug: 'faq', title: 'FAQ' }
		]
	}
];

/** Flattened sidebar order, used for prev/next navigation. */
export const flatSidebar: SidebarItem[] = sidebar.flatMap((group) => group.items);

/** The slug rendered at the bare `/docs` route. */
export const DOCS_INDEX_SLUG = 'introduction';

/** Resolve the URL for a docs slug. */
export function docHref(slug: string): string {
	return slug === DOCS_INDEX_SLUG ? '/docs' : `/docs/${slug}`;
}
