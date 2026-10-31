/**
 * The provider, put into the page's MAIN world where Safari would not
 * (isolated world, `document_start`, before `content.js`).
 *
 * Safari runs the manifest's `world: "MAIN"` entry only from Safari 18; on an
 * older Safari that entry runs ISOLATED, where `inpage.js`'s own world guard
 * stops it. When the MAIN world has the provider, it has marked the shared DOM
 * (`data-vela-inpage`, set synchronously at its first line) and there is
 * nothing to do; otherwise the provider goes in as a page `<script>` from the
 * package (`web_accessible_resources`), the technique every Safari wallet
 * extension uses. `inpage.js` refuses to install twice either way.
 */
(() => {
	const root = document.documentElement;
	if (!root || root.hasAttribute('data-vela-inpage')) return;
	const api = globalThis.chrome?.runtime ? globalThis.chrome : globalThis.browser;
	if (!api?.runtime?.getURL) return;
	const script = document.createElement('script');
	script.src = api.runtime.getURL('inpage.js');
	script.async = false;
	script.onload = () => script.remove();
	(document.head || root).prepend(script);
})();
