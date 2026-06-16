/**
 * Docs sidebar. This is the single source of truth for docs ordering and
 * grouping (VitePress-style). Each `slug` must match a file in
 * `src/content/docs/<slug>.md`. The first item ("introduction") is served at
 * `/docs`; everything else lives under `/docs/<slug>`.
 */
export interface SidebarItem {
	slug: string;
	title: string;
}

export interface SidebarGroup {
	title: string;
	items: SidebarItem[];
}

export const sidebar: SidebarGroup[] = [
	{
		title: 'Getting Started',
		items: [
			{ slug: 'introduction', title: 'Introduction' },
			{ slug: 'install', title: 'Install Vela' },
			{ slug: 'create-wallet', title: 'Create your wallet' }
		]
	},
	{
		title: 'Using Vela',
		items: [
			{ slug: 'send-and-receive', title: 'Send & receive' },
			{ slug: 'networks-and-fees', title: 'Networks & fees' }
		]
	},
	{
		title: 'Security',
		items: [
			{ slug: 'passkeys', title: 'How passkeys work' },
			{ slug: 'recovery', title: 'Recovery & sign-in' }
		]
	},
	{
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
