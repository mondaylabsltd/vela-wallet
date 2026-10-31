/**
 * Vela's content script on the trusted signing page (native AUDIT §6.5):
 * vouches for this extension, and carries the hand-off between the page and
 * the wallet.
 *
 * Why it exists — two facts measured on iOS Safari:
 *
 *   1. Safari makes the extension's origin up (`safari-web-extension://<UUID>`,
 *      per install and anew at launch), so the page cannot list it the way it
 *      lists the Chrome id. This script posts the extension's CURRENT origin
 *      into the page's own window (`{vela:'extension', origin}`), where only
 *      code running in that window can post from, and the page answers an
 *      opener whose origin is one so attested
 *      (`app-web/trusted-signer/src/lib/intake.js` `hearAttestations`).
 *   2. A page an extension page opens has NO `window.opener` on iOS Safari, so
 *      the hand-off's postMessage has no window to travel between. This script
 *      is the window: what the wallet sends (`vela.signer.toPage`, by
 *      `tabs.sendMessage` to this tab) is posted into the page as
 *      `{relay:'to-page', from:<origin>, …}`; what the page answers
 *      (`{relay:'to-extension', …}`) goes back with `runtime.sendMessage`
 *      (`vela.signer.fromPage`), which the wallet page hears with the
 *      browser's own word for which tab and URL sent it
 *      (`src/lib/extension/safari-signer.ts`).
 *
 * Safari runs it on the signing page only (`lib/safari-manifest.js`
 * `SIGNING_PAGES`) — on any other site the origin would be a per-session
 * identifier handed to a stranger.
 */
(() => {
	const api = globalThis.chrome?.runtime ? globalThis.chrome : globalThis.browser;
	if (!api?.runtime?.getURL) return;
	const origin = api.runtime.getURL('').replace(/\/$/, '');
	const say = () => window.postMessage({ vela: 'extension', origin }, location.origin);

	window.addEventListener('message', (event) => {
		if (event.source !== window) return;
		const data = event.data;
		if (!data || typeof data !== 'object') return;
		if (data.vela === 'extension?') {
			say();
			return;
		}
		if (data.relay !== 'to-extension') return;
		const message = { ...data };
		delete message.relay;
		try {
			const sent = api.runtime.sendMessage({ type: 'vela.signer.fromPage', data: message });
			if (sent && typeof sent.catch === 'function') sent.catch(() => {});
		} catch {
			/* the extension went away: the page times out on its own */
		}
	});

	api.runtime.onMessage.addListener((message) => {
		if (
			message?.type !== 'vela.signer.toPage' ||
			!message.data ||
			typeof message.data !== 'object'
		) {
			return undefined;
		}
		window.postMessage({ ...message.data, relay: 'to-page', from: origin }, location.origin);
		return undefined;
	});

	say();
})();
