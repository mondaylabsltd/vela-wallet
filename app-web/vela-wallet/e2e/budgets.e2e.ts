/**
 * The budgets, re-asserted now that the money path is aboard (spec 026 T262 —
 * SC-207).
 *
 * 026 added two dependencies and a dev harness that carries PRIVATE KEYS. The
 * promise is that none of it reaches the page most people arrive on. The
 * neighbouring suites already pin their halves — welcome-ssr proves Welcome
 * fetches no wasm and the deployed Worker carries none, parallel-entry and
 * batch prove the fixture keys and the spreadsheet parser stay off the
 * WALLET's startup path. Two things were left unpinned, and this file holds
 * them:
 *
 *   1. the landing page itself — the one 15 locales are prerendered for, and
 *      the one a stranger sees first;
 *   2. the artifact count across the MONEY routes: six more state machines
 *      wired must still cost exactly one 3.4 MB download, and the build must
 *      carry exactly one of them.
 */
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import QRCode from 'qrcode';
import { expect, test } from '@playwright/test';
import { chunkSource, chunksCarrying, collectScripts, en } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

const APP_ROOT = join(import.meta.dirname, '..');

/** The first fixture private key, verbatim — the thing that must never ship. */
const FIXTURE_SEED = 'd80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5';
/** SheetJS announces itself in every build it is part of. */
const SHEETJS = /sheetjs|XLSX\.utils|SheetJS/i;
/**
 * The dApp channel's own vocabulary (spec 027). The layer entered the graph
 * when the wallet, settings and request routes started using it, and Welcome
 * has no business carrying a line of it — it is the page a stranger meets, and
 * a wallet that has not been opened has no connections to speak of.
 */
const DAPP_CHANNEL = /vela-1193|vela\.perm\.|vela\.ext\.cache/;

test('the landing page carries neither the fixture keys nor the spreadsheet parser', async ({
	page
}) => {
	const scripts = collectScripts(page);
	await denyOffOrigin(page);

	// Both faces of the landing page: a stranger's first run (the 020 intro
	// carousel) and the returning visit that lands on Welcome itself.
	await page.goto('/en');
	await page.waitForLoadState('networkidle');
	await page.evaluate(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await page.goto('/en');
	await page.waitForLoadState('networkidle');
	expect(scripts.length).toBeGreaterThan(0);

	expect(chunksCarrying(scripts, FIXTURE_SEED), 'a fixture private key reached Welcome').toEqual(
		[]
	);
	expect(chunksCarrying(scripts, SHEETJS), 'the spreadsheet parser reached Welcome').toEqual([]);
	expect(chunksCarrying(scripts, DAPP_CHANNEL), 'the dApp channel reached Welcome').toEqual([]);
});

test('the money routes load ONE core artifact, and the build ships exactly one', async ({
	page
}) => {
	const wasmUrls = new Set<string>();
	page.on('request', (request) => {
		if (request.url().endsWith('.wasm')) wasmUrls.add(request.url());
	});
	await denyOffOrigin(page);

	// The whole money surface in one context: the harness that swaps the
	// signer, the wallet the tracker boots on, and the send flow itself.
	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.waitForLoadState('networkidle');

	// One artifact, not a per-feature zoo — wiring six more machines costs
	// 0 bytes, which is the entire reason the core is one binary.
	expect([...wasmUrls]).toHaveLength(1);
	expect([...wasmUrls][0]).toMatch(/vela_core_bg\.[0-9a-f]+\.wasm$/);

	// And the build has exactly one to serve: `sync-wasm` drops superseded
	// fingerprints so `static/` cannot accumulate dead 3.4 MB files, but only
	// the built output proves what a deploy would actually carry.
	const artifacts = readdirSync(join(APP_ROOT, '.svelte-kit/output/client')).filter((name) =>
		/^vela_core_bg\..*\.wasm$/.test(name)
	);
	expect(artifacts).toHaveLength(1);
	expect('/' + artifacts[0]).toBe(new URL([...wasmUrls][0]).pathname);
});

/**
 * Spec 028 T460: the QR decoders are lazy, and stay lazy.
 *
 * 028 added the first wasm that is not the core — `@undecaf/zbar-wasm` (239 KB)
 * — and `jsqr` (a 130 KB chunk). The promise D45 made is that both exist only
 * while a scanner is open: nothing on Welcome, nothing on the wallet's startup
 * path, and the moment someone opens the scanner they arrive. The third part
 * is the control — an assertion about absence is only worth something if the
 * markers would have fired.
 *
 * The markers are the decoders' OWN literals: `onlyInvert` is a jsqr option
 * value that `qr-decode.ts` never spells (it says `attemptBoth`), and the zbar
 * glue is the one chunk that names the `zbar.<hash>.wasm` asset.
 */
const JSQR = /onlyInvert/;
const ZBAR_GLUE = /zbar\.[A-Za-z0-9_-]+\.wasm/;

test('the QR decoders reach neither Welcome nor the wallet’s startup path — and do reach the scanner', async ({
	page
}) => {
	const scripts = collectScripts(page);
	const wasm: string[] = [];
	page.on('request', (request) => {
		if (request.url().endsWith('.wasm')) wasm.push(new URL(request.url()).pathname);
	});
	await denyOffOrigin(page);

	// 1. Welcome: no decoder, no wasm of any kind.
	await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await page.goto('/en');
	await page.waitForLoadState('networkidle');
	// An absence assertion is only worth something if the chunks can be read
	// at all: a URL that maps to no file reads as '' and "carries" nothing.
	expect(
		scripts.filter((url) => chunkSource(url) !== '').length,
		'no served chunk could be read from the build output — every budget below would pass vacuously'
	).toBeGreaterThan(0);
	expect(chunksCarrying(scripts, JSQR), 'jsqr reached Welcome').toEqual([]);
	expect(chunksCarrying(scripts, ZBAR_GLUE), 'the zbar glue reached Welcome').toEqual([]);
	expect(wasm, 'Welcome fetched wasm').toEqual([]);

	// 2. The wallet's startup path: the core, and only the core.
	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(chunksCarrying(scripts, JSQR), 'jsqr reached the wallet home').toEqual([]);
	expect(chunksCarrying(scripts, ZBAR_GLUE), 'the zbar glue reached the wallet home').toEqual([]);
	expect(
		wasm.filter((path) => !/vela_core_bg\./.test(path)),
		'a second wasm on startup'
	).toEqual([]);

	// 3. The control: open the scanner and both decoders' CODE arrives; decode
	//    something and the zbar BINARY arrives. Two steps on purpose — the
	//    239 KB wasm is fetched on the first decode, not when the scanner
	//    opens, and a budget nobody can trip is not a budget.
	await page
		.getByRole('button', { name: en('componentsUi.dock.scan') })
		.first()
		.click();
	await expect(page.getByText(en('componentsUi.scanner.gallery')).first()).toBeVisible();
	await expect
		.poll(() => chunksCarrying(scripts, JSQR).length + chunksCarrying(scripts, ZBAR_GLUE).length, {
			timeout: 20_000
		})
		.toBeGreaterThanOrEqual(2);
	expect(
		wasm.some((path) => /zbar\./.test(path)),
		'the zbar binary arrived before any decode'
	).toBe(false);

	await page.locator('input[type="file"]').setInputFiles({
		name: 'code.png',
		mimeType: 'image/png',
		buffer: await QRCode.toBuffer('0x' + 'a1'.repeat(20), { margin: 4, width: 600, type: 'png' })
	});
	await expect.poll(() => wasm.some((path) => /zbar\./.test(path)), { timeout: 20_000 }).toBe(true);
});
