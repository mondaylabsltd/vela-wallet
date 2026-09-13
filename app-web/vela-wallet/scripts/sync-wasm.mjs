#!/usr/bin/env node
/**
 * Put the committed vela-core wasm where the BROWSER can fetch it.
 *
 * `rust/scripts/build-web.mjs` writes one fingerprinted artifact to the repo's
 * `assets/wasm/` directory and names it in `rust/pkg-web/vela_core_wasm_url.js`.
 * That is where this app's build-time consumers, the corpus replay and
 * scripts/onchain read it from disk (the path is a leftover of the retired
 * Expo client, kept because every reader names it). SvelteKit serves
 * `static/`, not `public/`, so
 * the onboarding runtime path needs a copy there — under the same fingerprinted
 * name, so `WASM_URL` resolves in dev and in production without a second URL.
 *
 * The copy is gitignored on purpose: a 3.4 MB binary committed twice is a
 * 3.4 MB binary committed twice. This script is the generator, and `--check`
 * is the gate — it fails when the copy is missing or stale rather than letting
 * a build ship a wasm that does not match `pkg-web`.
 *
 * Usage: node scripts/sync-wasm.mjs [--check]
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const APP = resolve(HERE, '..');
const REPO = resolve(APP, '../..');
const STATIC = join(APP, 'static');
const check = process.argv.includes('--check');

function fail(message) {
	console.error(`sync-wasm: ${message}`);
	process.exit(1);
}

const urlModule = join(REPO, 'rust/pkg-web/vela_core_wasm_url.js');
if (!existsSync(urlModule))
	fail(`${urlModule} missing — run \`npm run build:wasm\` at the repo root`);

const match = /export const WASM_URL = '([^']+)'/.exec(readFileSync(urlModule, 'utf8'));
if (!match) fail('could not read WASM_URL from vela_core_wasm_url.js');

const asset = match[1].replace(/^\//, '');
const source = join(REPO, 'assets', 'wasm', asset);
if (!existsSync(source))
	fail(`assets/wasm/${asset} missing — run \`npm run build:wasm\` at the repo root`);

const target = join(STATIC, asset);
const bytes = readFileSync(source);

const upToDate = existsSync(target) && readFileSync(target).equals(bytes);

if (check) {
	if (!upToDate) fail(`static/${asset} is missing or stale — run \`npm run sync:wasm\``);
	console.log(`sync-wasm: static/${asset} matches pkg-web (${bytes.length} bytes)`);
	process.exit(0);
}

mkdirSync(STATIC, { recursive: true });
// Drop superseded fingerprints so `static/` never accumulates dead 3.4 MB files.
for (const name of readdirSync(STATIC)) {
	if (/^vela_core_bg\..*\.wasm$/.test(name) && name !== asset) {
		rmSync(join(STATIC, name));
		console.log(`sync-wasm: removed stale static/${name}`);
	}
}
if (!upToDate) writeFileSync(target, bytes);
console.log(
	`sync-wasm: static/${asset} ${upToDate ? 'already current' : 'written'} (${bytes.length} bytes)`
);
