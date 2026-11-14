/**
 * The Safari package (native AUDIT §6.5): one extension code base, built for
 * Safari by `bun run build:extension:safari`, embedded by the iOS app's Safari
 * Web Extension target. These read the derivation (`lib/safari-manifest.js`),
 * the adapter (`safari/shim.js`) and the BUILT packages — absent, they fail
 * and say how to build them, like `package.test.ts`: a package nobody built is
 * not a package anybody checked.
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { packageBelowFloor } from '$lib/testing/syntax-floor';
import * as derive from '../../../extension/lib/safari-manifest.js';
import * as shim from '../../../extension/safari/shim.js';

const APP_ROOT = join(import.meta.dirname, '../../..');
const SOURCE = JSON.parse(readFileSync(join(APP_ROOT, 'extension/manifest.json'), 'utf8'));
const DIST = process.env.VELA_SAFARI_EXTENSION_DIST
	? resolve(APP_ROOT, process.env.VELA_SAFARI_EXTENSION_DIST)
	: join(APP_ROOT, 'extension/dist-safari');
const RELEASE_DIST = `${DIST}-release`;
const PBXPROJ = join(APP_ROOT, '../../app-ios/VelaWallet/VelaWallet.xcodeproj/project.pbxproj');
const WASM_URL_MODULE = join(APP_ROOT, '../../rust/pkg-web/vela_core_wasm_url.js');

function files(dir: string, prefix = ''): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		const relative = prefix ? `${prefix}/${name}` : name;
		return statSync(path).isDirectory() ? files(path, relative) : [relative];
	});
}

/**
 * The scripts `extension/build.mjs --safari` bundles itself (its Safari
 * `ENTRIES`) that both packages carry; the harness doorways (`dev-bridge.js`,
 * `dev-handoff.js`) are bundled for the development package only.
 */
const BUNDLED = [
	'inpage.js',
	'content.js',
	'background.js',
	'panel.js',
	'open.js',
	'safari-inject.js',
	'safari-attest.js'
];

const INLINE_SCRIPT = /<script(?![^>]*\ssrc=)([^>]*)>([\s\S]*?)<\/script>/g;
const developerFiles = (dist: string) =>
	files(dist).filter((path) => /(^|\/)(parallel|gallery)(\.|\/)|^dev\/|^dev-/.test(path));

describe('the Safari manifest, derived from the Chrome one', () => {
	const release = derive.safariManifest(SOURCE, { development: false });
	const development = derive.safariManifest(SOURCE, { development: true });

	it('keeps every promise the Chrome manifest makes that Safari can keep', () => {
		for (const key of ['manifest_version', 'name', 'version', 'description', 'icons', 'action']) {
			expect(release[key], key).toEqual(SOURCE[key]);
		}
		// The core compiles (wasm), the passkey's relying party, every site.
		expect(release.content_security_policy).toEqual(SOURCE.content_security_policy);
		expect(release.host_permissions).toEqual(SOURCE.host_permissions);
		expect(release.permissions).toEqual(['storage', 'tabs']);
		// Nothing remote runs, no page messages the worker, no popup.
		expect(release.externally_connectable).toBeUndefined();
		expect(release.action?.default_popup).toBeUndefined();
	});

	it('drops what only Chrome reads, and asks for nothing native', () => {
		for (const key of ['key', 'minimum_chrome_version', 'side_panel', 'incognito']) {
			expect(release[key], key).toBeUndefined();
		}
		expect(release.permissions).not.toContain('sidePanel');
		// No native messaging: nothing crosses from Safari into the app.
		expect(release.permissions).not.toContain('nativeMessaging');
	});

	it('runs the worker as a non-persistent background page — what iOS Safari keeps alive', () => {
		expect(release.background).toEqual({ scripts: ['background.js'], persistent: false });
	});

	it("floors Safari at the iOS app's own deployment target", () => {
		const targets = [
			...readFileSync(PBXPROJ, 'utf8').matchAll(/IPHONEOS_DEPLOYMENT_TARGET = ([\d.]+);/g)
		].map((m) => m[1]);
		expect(new Set(targets)).toEqual(new Set([derive.SAFARI_MIN_VERSION]));
		expect(release.browser_specific_settings).toEqual({
			safari: { strict_min_version: derive.SAFARI_MIN_VERSION }
		});
	});

	it('puts the provider in the MAIN world, and injects it where that world is not offered', () => {
		const [main, isolated] = release.content_scripts;
		expect(main).toMatchObject({ js: ['inpage.js'], world: 'MAIN', run_at: 'document_start' });
		// The injector runs before content.js, and only inpage.js is reachable
		// by a page — the one file the injector names.
		expect(isolated).toMatchObject({
			js: ['safari-inject.js', 'content.js'],
			run_at: 'document_start'
		});
		expect(isolated.world).toBeUndefined();
		expect(release.web_accessible_resources).toEqual([
			{ resources: ['inpage.js'], matches: main.matches }
		]);
		expect(release.content_scripts).toHaveLength(3);
	});

	it('vouches for itself to the signing page only, never to other sites', () => {
		// The attestation names this extension's per-launch origin: on the
		// official page in what ships, plus the developer's loopback page in
		// the development package — and on no other site.
		type Script = { js: string[]; matches: string[] };
		const attestOn = (m: typeof release) =>
			(m.content_scripts as Script[]).filter((s) => s.js.includes('safari-attest.js'));
		expect(attestOn(release)).toEqual([
			{
				matches: ['https://sign.getvela.app/*'],
				js: ['safari-attest.js'],
				run_at: 'document_start',
				all_frames: false
			}
		]);
		expect(attestOn(development)[0].matches).toEqual([
			'https://sign.getvela.app/*',
			...derive.LOOPBACK_MATCHES
		]);
	});

	it('adds the harness doorway in the development package only, on loopback only', () => {
		const added = development.content_scripts.slice(3);
		expect(added).toEqual([
			{
				matches: derive.LOOPBACK_MATCHES,
				js: ['dev-bridge.js'],
				run_at: 'document_end',
				all_frames: false
			}
		]);
		expect(development.web_accessible_resources.slice(1)).toEqual([
			{ resources: derive.DEV_PAGES, matches: derive.LOOPBACK_MATCHES }
		]);
		expect(JSON.stringify(release)).not.toMatch(/dev-/);
	});
});

describe('the Safari adapter (safari/shim.js)', () => {
	/** A Safari-shaped API: `withWindows` false is iOS, which has no `windows` at all. */
	function fakeSafari(withWindows: boolean) {
		const removed: ((tabId: number) => void)[] = [];
		let next = 40;
		const open = new Map<number, { id: number; windowId: number; url: string }>();
		const tabs = {
			create: vi.fn(async (opts: { url: string; openerTabId?: number }) => {
				const tab = { id: next++, windowId: 1, url: opts.url };
				open.set(tab.id, tab);
				return tab;
			}),
			remove: vi.fn(async (id: number) => {
				open.delete(id);
				for (const f of removed) f(id);
			}),
			update: vi.fn(async (id: number) => open.get(id)),
			get: vi.fn(async (id: number) => {
				const tab = open.get(id);
				if (!tab) throw new Error('no tab');
				return tab;
			}),
			query: vi.fn(async () => [...open.values()]),
			onRemoved: { addListener: (f: (id: number) => void) => removed.push(f) }
		};
		const windows = {
			create: vi.fn(async () => ({ id: 7 })),
			remove: vi.fn(async () => {}),
			update: vi.fn(async (id: number) => ({ id })),
			getAll: vi.fn(async () => [{ id: 1 }]),
			onRemoved: { addListener: vi.fn() }
		};
		const kept: Record<string, unknown> = {};
		const native = Object.freeze({
			runtime: { id: 'app.getvela.VelaWallet.SafariExtension' },
			storage: {
				session: {
					set: async (items: Record<string, unknown>) => void Object.assign(kept, items),
					get: async (key: string) => (key in kept ? { [key]: kept[key] } : {}),
					remove: async (key: string) => void delete kept[key]
				}
			},
			tabs,
			...(withWindows ? { windows } : {})
		});
		return { chrome: shim.withTabSurfaces(native), native, tabs, windows };
	}

	it('never writes onto Safari’s own objects (they are host objects)', () => {
		const { chrome, native } = fakeSafari(false);
		expect(Object.isFrozen(native)).toBe(true);
		expect(chrome.runtime.id).toBe(native.runtime.id);
		expect(typeof chrome.windows.create).toBe('function');
	});

	it.each([
		['iOS (no windows API)', false],
		['macOS (windows API)', true]
	])(
		'%s: a request rises as a tab of the asking dApp, under a virtual window id',
		async (_n, w) => {
			const { chrome, tabs } = fakeSafari(w as boolean);
			const created = await chrome.windows.create({
				url: 'safari-web-extension://X/open.html?rid=12%3Aabc',
				type: 'popup',
				focused: true
			});
			expect(tabs.create).toHaveBeenCalledWith({
				url: 'safari-web-extension://X/open.html?rid=12%3Aabc',
				active: true,
				openerTabId: 12
			});
			expect(created.id).toBe(shim.virtualWindowId(40));
			expect(shim.isVirtualWindow(created.id)).toBe(true);
			expect(shim.tabOfVirtual(created.id)).toBe(40);
		}
	);

	it('closes, focuses and finds that tab by its virtual id — never a real window', async () => {
		const { chrome, tabs } = fakeSafari(false);
		const heard: number[] = [];
		chrome.windows.onRemoved.addListener((id: number) => heard.push(id));
		const { id } = await chrome.windows.create({ url: 'x://y/open.html?rid=3:1', type: 'popup' });
		expect(await chrome.tabs.query({ windowId: id })).toHaveLength(1);
		const all = (await chrome.windows.getAll()).map((x: { id: number }) => x.id);
		expect(all).toEqual([1, id]); // the tab's real window, and the virtual one
		await chrome.windows.update(id, { focused: true });
		expect(tabs.update).toHaveBeenCalledWith(40, { active: true });
		await chrome.windows.remove(id);
		expect(tabs.remove).toHaveBeenCalledWith(40);
		// …and the person is put back on the dApp that asked (tab 3), not on
		// whichever tab Safari would show next.
		expect(tabs.update).toHaveBeenCalledWith(3, { active: true });
		expect(heard).toContain(id); // the worker hears its "window" close
	});

	it('leaves every real window to Safari where Safari has windows', async () => {
		const { chrome, tabs, windows } = fakeSafari(true);
		await chrome.windows.create({ url: 'https://a.example/', type: 'normal' });
		await chrome.windows.remove(5);
		expect(windows.create).toHaveBeenCalled();
		expect(windows.remove).toHaveBeenCalledWith(5);
		expect(tabs.create).not.toHaveBeenCalled();
		expect(tabs.remove).not.toHaveBeenCalled();
	});

	it('drops `windowType` from a tab query (Safari has normal windows only)', async () => {
		const { chrome, tabs } = fakeSafari(false);
		await chrome.tabs.query({ url: 'x://*', windowType: 'normal' });
		expect(tabs.query).toHaveBeenCalledWith({ url: 'x://*' });
	});

	it('reads the dApp tab from the request id the doorway carries', () => {
		expect(shim.openerOf('safari-web-extension://U/open.html?rid=7%3Ap-1')).toBe(7);
		expect(shim.openerOf('safari-web-extension://U/open.html')).toBeUndefined();
		expect(shim.openerOf('not a url')).toBeUndefined();
	});
});

describe.each([
	['development', DIST],
	['release', RELEASE_DIST]
])('the Safari %s package', (name, dist) => {
	const manifest = () => JSON.parse(readFileSync(join(dist, 'manifest.json'), 'utf8'));

	it('exists — run `bun run build:extension:safari` before trusting this file', () => {
		expect(existsSync(join(dist, 'manifest.json'))).toBe(true);
	});

	it('carries exactly the derived manifest', () => {
		expect(manifest()).toEqual(
			derive.safariManifest(SOURCE, { development: name === 'development' })
		);
	});

	it('carries every script its manifest names, the adapter in front of the worker', () => {
		for (const script of [
			...manifest().background.scripts,
			...manifest().content_scripts.flatMap((s: { js: string[] }) => s.js)
		]) {
			expect(existsSync(join(dist, script)), script).toBe(true);
		}
		const worker = readFileSync(join(dist, 'background.js'), 'utf8');
		expect(worker.indexOf('withTabSurfaces')).toBeGreaterThan(-1);
		expect(worker.indexOf('withTabSurfaces')).toBeLessThan(worker.indexOf('vela.surface'));
	});

	it('is the Safari build of the pages: their scheme is `safari-web-extension:`', () => {
		const scripts = files(dist).filter((f) => f.endsWith('.js'));
		const text = scripts.map((f) => readFileSync(join(dist, f), 'utf8'));
		expect(text.some((t) => /["'`]safari-web-extension:["'`]/.test(t))).toBe(true);
		expect(text.some((t) => t.includes('__VELA_EXTENSION_PROTOCOL__'))).toBe(false);
	});

	it('has no top-level `_` name and no inline script', () => {
		expect(readdirSync(dist).filter((n) => n.startsWith('_'))).toEqual([]);
		const carriers = files(dist)
			.filter((f) => f.endsWith('.html'))
			.filter((f) =>
				[...readFileSync(join(dist, f), 'utf8').matchAll(INLINE_SCRIPT)].some(
					(m) => m[2].trim().length > 0
				)
			);
		expect(carriers).toEqual([]);
	});

	/**
	 * §31: the Safari package's dApp door is the same request surface as
	 * Chrome's (`DappRequestHost` → the resident `sign_request` machine), which
	 * `e2e/parent-safe-guard.e2e.ts` drives in the Chrome package — Playwright
	 * loads no Safari extension. What this package must carry for that proof
	 * to hold here: the device's multisig list fed to the machine, the
	 * machine's isOwner read answered, its refusal special-cased as the
	 * self-call one is, and the catalog's sentence for it.
	 */
	it('carries the parent-Safe guard wiring and its words (§31)', () => {
		const text = files(dist)
			.filter((f) => f.endsWith('.js') || f.endsWith('.html') || f.endsWith('.json'))
			.map((f) => readFileSync(join(dist, f), 'utf8'))
			.join('\n');
		for (const wire of [
			'multisigs_changed',
			'read_safe_owner',
			'safe_owner_read',
			'parent_safe_blocked',
			'parent_safe'
		]) {
			// The minifier writes string literals in any of the three quotes.
			expect(new RegExp(`["'\`]${wire}["'\`]`).test(text), wire).toBe(true);
		}
		expect(text).toContain(
			'This site asked your account to change or approve something for a multisig'
		);
	});

	it('holds every script it ships to the Safari 17.4 floor', () => {
		// `build.mjs` bundles with Bun, which lowers no syntax for a browser
		// version, and vite's chunks and the lifted boot scripts ship too.
		const scripts = files(dist);
		for (const script of BUNDLED) expect(scripts, script).toContain(script);
		expect(packageBelowFloor(dist)).toEqual({});
	}, 60_000); // ~16 MB of script parsed: well past the 5 s default under a full run

	it('carries the very core its code names', () => {
		const current = /export const WASM_URL = '\/([^']+)'/.exec(
			readFileSync(WASM_URL_MODULE, 'utf8')
		)?.[1];
		expect(current).toBeTruthy();
		expect(existsSync(join(dist, current as string))).toBe(true);
	});
});

describe('the Safari development package', () => {
	it('keeps the parallel space and the harness doorway', () => {
		expect(existsSync(join(DIST, 'en/parallel.html'))).toBe(true);
		for (const page of [...derive.DEV_PAGES, ...derive.DEV_SCRIPTS]) {
			expect(existsSync(join(DIST, page)), page).toBe(true);
		}
	});
});

describe('the Safari release package — what the App Store receives', () => {
	it('carries no developer page, no harness doorway', () => {
		expect(developerFiles(RELEASE_DIST)).toEqual([]);
	});
});
