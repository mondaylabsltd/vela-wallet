#!/usr/bin/env node
/**
 * The vendored PeerConnect client is what its recorded source builds (§38,
 * review 1-10).
 *
 * `app-web/vela-wallet/src/lib/vendor/peerconnect/` ships PeerConnect's built
 * `js/` package — a wasm-bindgen glue file and the WebAssembly module inlined
 * as base64 — because PeerConnect is not on a registry. A built artefact in
 * the tree is exactly where a substituted dependency would hide, so its
 * provenance is written down (`VENDOR.json`) and checked two ways:
 *
 *   node scripts/check-peerconnect-vendor.mjs
 *       Every vendored file still has the sha256 VENDOR.json records — no
 *       toolchain, a second; CI's `app` job.
 *
 *   node scripts/check-peerconnect-vendor.mjs --rebuild [<peerconnect checkout or URL>]
 *       Rebuild from source and compare byte for byte: a clean clone of the
 *       recorded commit (default: VENDOR.json's `repository`), the recorded
 *       Rust toolchain and wasm-bindgen, the paths of this machine mapped out
 *       of the module, then `vela-local.patch` (Vela's one local change) on
 *       the upstream `index.js` / `index.d.ts`. CI's `supply-chain` job.
 *
 *       `dist/module.js` is compared as the WebAssembly it inlines, without
 *       two custom sections nothing reads at run time, both of which name the
 *       BUILDER rather than the build:
 *         - `producers`: wasm-bindgen in its own words, which depend on how
 *           the CLI was installed (`0.2.129` from `cargo install`,
 *           `0.2.129 (165586f85)` as the release binary CI downloads) — the
 *           web core's build drops it too (rust/scripts/build-web.mjs);
 *         - `name`: the debug names of the functions, each carrying its
 *           crate's hash (`js_sys[5e0a…]::Object::new`), which Cargo derives
 *           from `rustc -vV` — host triple included — so a Linux x86_64 build
 *           and a macOS arm64 one name the same code differently.
 *       Every other section — the types, imports, functions, code, data and
 *       exports, i.e. everything that runs — must match byte for byte
 *       (measured: CI's Linux build and a macOS build differ in those two
 *       sections and nowhere else).
 *
 *   node scripts/check-peerconnect-vendor.mjs --rebuild <…> --write
 *       The same build, written into the vendor directory, and VENDOR.json's
 *       hashes rewritten — how an update is made (then run the check).
 *
 * Needs, for --rebuild: git, rustup with the recorded toolchain (and its
 * wasm32-unknown-unknown target), and `wasm-bindgen` at the recorded version
 * on PATH (`cargo install wasm-bindgen-cli --locked --version <v>`).
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const VENDOR = join(ROOT, 'app-web/vela-wallet/src/lib/vendor/peerconnect');
const MANIFEST = join(VENDOR, 'VENDOR.json');
const manifest = JSON.parse(readFileSync(MANIFEST, 'utf8'));

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const args = process.argv.slice(2);
const rebuild = args.includes('--rebuild');
const write = args.includes('--write');
const source = args.find((a, i) => i > 0 && args[i - 1] === '--rebuild' && !a.startsWith('--'));

function checkRecorded() {
	const drift = [];
	for (const [file, want] of Object.entries(manifest.files)) {
		const path = join(VENDOR, file);
		const got = existsSync(path) ? sha256(readFileSync(path)) : 'missing';
		if (got !== want) drift.push(`  ${file}: recorded ${want}, vendored ${got}`);
	}
	const patch = sha256(readFileSync(join(VENDOR, 'vela-local.patch')));
	if (patch !== manifest.localPatch.sha256) {
		drift.push(`  vela-local.patch: recorded ${manifest.localPatch.sha256}, vendored ${patch}`);
	}
	if (drift.length) {
		console.error(
			'check-peerconnect-vendor: the vendored PeerConnect client is not what VENDOR.json records:\n' +
				drift.join('\n') +
				'\nAn update is a rebuild from a recorded commit: see the script header.'
		);
		process.exit(1);
	}
	console.log(
		`check-peerconnect-vendor: ${Object.keys(manifest.files).length} files match VENDOR.json ` +
			`(PeerConnect ${manifest.commit.slice(0, 12)})`
	);
}

/** The WebAssembly `module.js` inlines (js/scripts/inline.mjs: one base64 string). */
function inlinedWasm(bytes) {
	const base64 = /export default "([A-Za-z0-9+/=]+)";/.exec(bytes.toString('utf8'))?.[1];
	if (!base64) throw new Error('check-peerconnect-vendor: dist/module.js does not inline a module');
	return Buffer.from(base64, 'base64');
}

/** The module without its `producers` and `name` custom sections (see the header). */
function withoutBuilderSections(wasm) {
	const uleb = (i) => {
		let value = 0;
		let shift = 0;
		for (;;) {
			const byte = wasm[i++];
			value += (byte & 0x7f) * 2 ** shift;
			shift += 7;
			if (!(byte & 0x80)) return [value, i];
		}
	};
	const kept = [wasm.subarray(0, 8)];
	let off = 8;
	while (off < wasm.length) {
		const start = off;
		const id = wasm[off++];
		const [size, body] = uleb(off);
		off = body + size;
		if (id === 0) {
			const [nameLength, name] = uleb(body);
			const section = wasm.subarray(name, name + nameLength).toString('utf8');
			if (section === 'producers' || section === 'name') continue;
		}
		kept.push(wasm.subarray(start, off));
	}
	return Buffer.concat(kept);
}

/** What a rebuild is compared by: the bytes, or for module.js the module minus its builder's sections. */
const comparable = (file, bytes) =>
	sha256(file === 'dist/module.js' ? withoutBuilderSections(inlinedWasm(bytes)) : bytes);

function run(cmd, argv, options = {}) {
	return execFileSync(cmd, argv, { stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', ...options });
}

function build() {
	const { commit, toolchain, wasmBindgen } = manifest;
	const have = run('wasm-bindgen', ['--version']).trim();
	if (!have.startsWith(`wasm-bindgen ${wasmBindgen}`)) {
		throw new Error(`wasm-bindgen ${wasmBindgen} is needed on PATH, found "${have}"`);
	}
	const work = mkdtempSync(join(tmpdir(), 'peerconnect-vendor-'));
	try {
		const checkout = join(work, 'peerconnect');
		run('git', ['clone', '--quiet', '--no-checkout', source ?? manifest.repository, checkout]);
		run('git', ['-C', checkout, 'checkout', '--quiet', commit]);
		const sysroot = run('rustup', ['run', toolchain, 'rustc', '--print', 'sysroot']).trim();
		const hash = /commit-hash: ([0-9a-f]+)/.exec(run('rustup', ['run', toolchain, 'rustc', '-vV']))?.[1];
		const cargoHome = process.env.CARGO_HOME ?? join(homedir(), '.cargo');
		// The same three mappings as the web core's build (rust/scripts/build-web.mjs):
		// no builder's path in the module, so any machine builds the same bytes.
		const rustflags = [
			`--remap-path-prefix=${cargoHome}=/cargo`,
			`--remap-path-prefix=${join(sysroot, 'lib/rustlib/src/rust')}=/rustc/${hash}`,
			`--remap-path-prefix=${checkout}=/peerconnect`
		].join(' ');
		const target = join(work, 'target');
		run(
			'rustup',
			['run', toolchain, 'cargo', 'build', '--locked', '-p', 'peerconnect-wasm', '--target', 'wasm32-unknown-unknown', '--profile', 'wasm'],
			{ cwd: checkout, env: { ...process.env, RUSTFLAGS: rustflags, CARGO_TARGET_DIR: target }, stdio: ['ignore', 'inherit', 'inherit'] }
		);
		const out = join(work, 'dist');
		mkdirSync(out);
		run('wasm-bindgen', [
			'--target', 'web', '--weak-refs', '--out-dir', out, '--out-name', 'peerconnect',
			join(target, 'wasm32-unknown-unknown/wasm/peerconnect_wasm.wasm')
		]);
		// js/scripts/inline.mjs, as PeerConnect's own build runs it — no wasm-opt
		// (its build runs one only if it finds one; the vendored module has none).
		run('node', [join(checkout, 'js/scripts/inline.mjs'), join(out, 'peerconnect_bg.wasm'), join(out, 'module.js')]);
		run('git', ['-C', checkout, 'apply', join(VENDOR, 'vela-local.patch')]);
		return {
			'index.js': readFileSync(join(checkout, 'js/index.js')),
			'index.d.ts': readFileSync(join(checkout, 'js/index.d.ts')),
			'dist/peerconnect.js': readFileSync(join(out, 'peerconnect.js')),
			'dist/peerconnect.d.ts': readFileSync(join(out, 'peerconnect.d.ts')),
			'dist/module.js': readFileSync(join(out, 'module.js'))
		};
	} finally {
		rmSync(work, { recursive: true, force: true });
	}
}

checkRecorded();
if (rebuild) {
	const built = build();
	// VELA_PEERCONNECT_OUT: keep what this build made, for reading when it
	// differs (CI uploads it on failure, as it does the web core's).
	const keep = process.env.VELA_PEERCONNECT_OUT;
	if (keep) {
		for (const [file, bytes] of Object.entries(built)) {
			mkdirSync(dirname(join(keep, file)), { recursive: true });
			writeFileSync(join(keep, file), bytes);
		}
	}
	if (write) {
		for (const [file, bytes] of Object.entries(built)) {
			writeFileSync(join(VENDOR, file), bytes);
			manifest.files[file] = sha256(bytes);
		}
		writeFileSync(MANIFEST, `${JSON.stringify(manifest, null, 2)}\n`);
		console.log('check-peerconnect-vendor: rebuilt and written; VENDOR.json updated');
	} else {
		// The vendored files already match VENDOR.json (checkRecorded above).
		const same = ([file, bytes]) =>
			comparable(file, bytes) === comparable(file, readFileSync(join(VENDOR, file)));
		const differ = Object.entries(built).filter((entry) => !same(entry));
		for (const entry of Object.entries(built)) {
			const [file, bytes] = entry;
			const exact = sha256(bytes) === manifest.files[file];
			const label = exact ? 'same   ' : same(entry) ? 'same*  ' : 'DIFFERS';
			console.log(`  ${label} ${file} ${sha256(bytes)}`);
		}
		if (Object.entries(built).some((entry) => same(entry) && sha256(entry[1]) !== manifest.files[entry[0]])) {
			console.log('  * the same module; only its `producers` and `name` sections differ (see the header)');
		}
		if (differ.length) {
			console.error(
				`check-peerconnect-vendor: PeerConnect ${manifest.commit.slice(0, 12)} does not build the vendored client`
			);
			process.exit(1);
		}
		const exact = Object.entries(built).every(([file, bytes]) => sha256(bytes) === manifest.files[file]);
		console.log(
			`check-peerconnect-vendor: PeerConnect ${manifest.commit.slice(0, 12)} rebuilds the vendored client ` +
				(exact ? 'byte for byte' : 'byte for byte outside the producers and name sections')
		);
	}
}
