/**
 * DEVELOPMENT PACKAGE ONLY — the harness's way into the extension's pages.
 *
 * Safari makes up the extension's origin (`safari-web-extension://<UUID>`)
 * anew, so a test cannot type a URL into the package the way the Chrome e2e
 * does with its pinned id. This content script runs on LOOPBACK pages only
 * (the harness's test dApp) and turns a link that names one of the developer
 * pages — `<a data-vela-dev="dev-seed.html?…">` — into that page's real URL.
 * Tapping it opens the page in the same tab.
 *
 * It names nothing else: only `DEV_PAGES`, and the release package carries
 * neither this script nor those pages (`safari-package.test.ts`,
 * `app-ios/scripts/check-ios-store-binary.sh`).
 */
import { DEV_PAGES } from '../lib/safari-manifest.js';

(() => {
	const api = globalThis.chrome?.runtime ? globalThis.chrome : globalThis.browser;
	if (!api?.runtime?.getURL) return;
	const wire = () => {
		for (const link of document.querySelectorAll('a[data-vela-dev]')) {
			const target = String(link.getAttribute('data-vela-dev') ?? '');
			const page = target.split('?')[0];
			if (!DEV_PAGES.includes(page)) continue;
			link.setAttribute('href', api.runtime.getURL(target));
			link.setAttribute('data-vela-dev-wired', '1');
		}
	};
	wire();
	new MutationObserver(wire).observe(document.documentElement, { childList: true, subtree: true });
})();
