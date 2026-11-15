export const seoConfig = {
	siteName: 'Vela Wallet',
	domain: 'https://getvela.app',

	// GitHub repository, used for "Edit this page on GitHub" links in the docs.
	// `repoBranch` is the default branch; `repoAppDir` is the path from the repo
	// root to this SvelteKit app. If the site ever moves to its own repo, update
	// these three values and every edit link follows.
	github: 'https://github.com/mondaylabsltd/vela-wallet',
	repoBranch: 'main',
	// Moved under app-web/ with the other web apps; the old path made every
	// "Edit this page on GitHub" link a 404 (found 2026-10-09, spec 101).
	repoAppDir: 'app-web/getvela.app'
};
