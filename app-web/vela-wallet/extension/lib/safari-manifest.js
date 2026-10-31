/**
 * The Safari package's manifest, derived from the Chrome one (native AUDIT
 * §6.5) — ONE source manifest, so a permission, a version or a CSP can never
 * disagree between the two browsers. Pure: `build.mjs --safari` writes what
 * this returns, and `safari-package.test.ts` reads both.
 *
 * What differs, and why (each measured on iOS Safari, see
 * `extension/README.md` § Safari):
 *
 *   - no `key`, no `minimum_chrome_version`: Chrome-only. Safari names the
 *     extension by the containing app's bundle id; its pages' origin is
 *     `safari-web-extension://<UUID>`, a UUID Safari makes up itself;
 *   - no `side_panel`, no `sidePanel` permission: Safari has neither. A dApp's
 *     request rises in a tab of the wallet instead (`safari/shim.js`);
 *   - no `incognito`: Safari does not read it; a person turns the extension
 *     on for private tabs in Safari's own settings;
 *   - `background` is a NON-PERSISTENT PAGE (`scripts` + `persistent: false`),
 *     not a service worker: what iOS Safari runs reliably. The worker's code
 *     is written to be woken anywhere already (its ledger is in
 *     `storage.session`), so the same bundle runs there;
 *   - the provider: Safari 18 runs a `world: "MAIN"` content script; before it
 *     the same entry runs isolated, where `inpage.js`'s own world guard stops
 *     it. So the MAIN-world entry stays, and `safari-inject.js` (isolated)
 *     adds the provider as a page `<script>` when the MAIN world did not —
 *     which is why `inpage.js` is web-accessible here and not on Chrome. On
 *     Chrome that listing was a free "is Vela installed?" probe at the pinned
 *     id (spec 089); Safari's UUID is made up anew, so there is no fixed URL
 *     to probe;
 *   - `browser_specific_settings.safari.strict_min_version`: the app's own
 *     floor (iOS 17.4 ships Safari 17.4).
 *
 * The DEVELOPMENT package adds the harness's doorway (`dev-bridge.js`, on
 * loopback pages only, and `dev-seed.html` / `dev-probe.html`) — the Safari
 * twin of the natives' debug launch inputs. The release package carries none
 * of it (`safari-package.test.ts`, `check-ios-store-binary.sh`).
 */

/** Safari's floor: the iOS app's deployment target (`IPHONEOS_DEPLOYMENT_TARGET`). */
export const SAFARI_MIN_VERSION = '17.4';

/** The pages only the loopback harness may open (development package only). */
export const DEV_PAGES = ['dev-seed.html', 'dev-probe.html', 'dev-handoff.html'];
/** The scripts only the development package carries. */
export const DEV_SCRIPTS = ['dev-bridge.js', 'dev-seed.js', 'dev-probe.js', 'dev-handoff.js'];
/** Where the harness's test dApp is served from. */
export const LOOPBACK_MATCHES = ['http://127.0.0.1/*', 'http://localhost/*'];

/**
 * Where `safari-attest.js` vouches for the extension: the official signing
 * page — and, in the development package, the developer's loopback page (the
 * page answers a loopback wallet only from the loopback). Nowhere else: the
 * extension's origin is a per-launch identifier no other site may learn.
 */
export const SIGNING_PAGES = ['https://sign.getvela.app/*'];

/** Chrome-only keys the Safari manifest drops. */
const CHROME_ONLY = ['key', 'minimum_chrome_version', 'side_panel', 'incognito'];

/**
 * @param {Record<string, any>} chrome the source manifest (`extension/manifest.json`)
 * @param {{ development?: boolean }} [options]
 */
export function safariManifest(chrome, { development = false } = {}) {
	const manifest = structuredClone(chrome);
	for (const key of CHROME_ONLY) delete manifest[key];
	manifest.permissions = (manifest.permissions ?? []).filter((p) => p !== 'sidePanel');
	manifest.background = { scripts: ['background.js'], persistent: false };
	manifest.browser_specific_settings = { safari: { strict_min_version: SAFARI_MIN_VERSION } };

	const pages = (chrome.content_scripts ?? []).find((entry) => entry.js?.includes('content.js'));
	const matches = pages?.matches ?? ['*://*/*'];
	manifest.content_scripts = [
		{ matches, js: ['inpage.js'], run_at: 'document_start', world: 'MAIN', all_frames: false },
		{
			matches,
			js: ['safari-inject.js', 'content.js'],
			run_at: 'document_start',
			all_frames: false
		},
		{
			matches: development ? [...SIGNING_PAGES, ...LOOPBACK_MATCHES] : SIGNING_PAGES,
			js: ['safari-attest.js'],
			run_at: 'document_start',
			all_frames: false
		}
	];
	manifest.web_accessible_resources = [{ resources: ['inpage.js'], matches }];

	if (development) {
		manifest.content_scripts.push({
			matches: LOOPBACK_MATCHES,
			js: ['dev-bridge.js'],
			run_at: 'document_end',
			all_frames: false
		});
		manifest.web_accessible_resources.push({ resources: DEV_PAGES, matches: LOOPBACK_MATCHES });
	}
	return manifest;
}
