#!/usr/bin/env node
/**
 * The built app must not import `swift_getExtendedFunctionTypeMetadata`.
 *
 * A runtime landmine, found as F33 (spec 087). The deployment target is
 * iOS 17.4; that entry point first shipped in the Swift 6.0 runtime (iOS 18).
 * The compiler knows: typed-throws and `@isolated(any)` function types whose
 * metadata would need it are build ERRORS below iOS 18. Swift 6.2 forgot the
 * third user — `nonisolated(nonsending)` function types — and emits a weak
 * import with no fallback. On iOS 17 the weak import is NULL, so the first
 * time anything asks for that metadata the app jumps to address 0:
 *
 *     EXC_BAD_ACCESS (KERN_INVALID_ADDRESS at 0x0)
 *       type metadata accessor for nonisolated(nonsending) () async -> Data?
 *       FeedbackSender.add(datas:)
 *
 * The target builds with SWIFT_APPROACHABLE_CONCURRENCY, which turns on
 * `NonisolatedNonsendingByDefault`, so EVERY unannotated async function type
 * is `nonisolated(nonsending)`. Its metadata is asked for when the type is a
 * generic argument (`[() async -> Data?]`, `Optional`, a key path) and when
 * anything reflects a stored property of that type — `Mirror`, `dump`, and
 * SwiftUI's AttributeGraph walking a View's fields. iOS 18 and later carry
 * the entry point, which is why a suite on the newest simulator never sees it.
 *
 * The fix is to say the isolation: `@MainActor` for a port the main actor
 * calls, `@concurrent` for one called off it. Either is ordinary function
 * metadata that every runtime since iOS 15 can build. Upstream:
 * swiftlang/swift #85017 / #86468, reported fixed in Swift 6.3 (Xcode 26.4);
 * until every toolchain that builds this app is past it, this check is the
 * guard, and it reads the binary, not the source, so it cannot miss a spelling.
 *
 * Usage:
 *   node app-ios/scripts/check-ios17-function-metadata.mjs <path>...
 *       Each path is a Mach-O file or a directory searched for them (a
 *       `.app`, an `.xcarchive`, `Build/Products`, or
 *       `Build/Intermediates.noindex` for one line per source file).
 *   node app-ios/scripts/check-ios17-function-metadata.mjs --self-test
 *       Compile two snippets for iOS 17.4 and prove the check flags the bad
 *       one and passes the annotated one.
 *
 * Exit 0 = clean. Exit 1 = names each binary and the function types to annotate.
 */
import { execFileSync } from 'node:child_process';
import { closeSync, mkdtempSync, openSync, readSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const ENTRY = '_swift_getExtendedFunctionTypeMetadata';
/** Mangling suffix of a `nonisolated(nonsending)` function type's metadata accessor. */
const NONSENDING_ACCESSOR = /YCcMa$/;
const MACHO = new Set(['cffaedfe', 'cefaedfe', 'cafebabe', 'bebafeca']);

function isMachO(path) {
	const fd = openSync(path, 'r');
	try {
		const buf = Buffer.alloc(4);
		return readSync(fd, buf, 0, 4, 0) === 4 && MACHO.has(buf.toString('hex'));
	} finally {
		closeSync(fd);
	}
}

function machOFiles(path, out = []) {
	const st = statSync(path, { throwIfNoEntry: false });
	if (!st) throw new Error(`no such path: ${path}`);
	if (st.isDirectory()) {
		for (const name of readdirSync(path)) {
			const p = join(path, name);
			const s = statSync(p, { throwIfNoEntry: false });
			if (s && (s.isDirectory() || s.isFile())) machOFiles(p, out);
		}
	} else if (st.isFile() && st.size >= 4 && isMachO(path)) {
		out.push(path);
	}
	return out;
}

function nm(file) {
	try {
		return execFileSync('xcrun', ['nm', '-m', file], { encoding: 'utf8', maxBuffer: 1 << 30, stdio: ['ignore', 'pipe', 'ignore'] });
	} catch {
		return ''; // nm refuses some Mach-O kinds (e.g. a dSYM's companion); nothing to import there.
	}
}

function demangle(symbols) {
	if (symbols.length === 0) return [];
	const out = execFileSync('xcrun', ['swift-demangle', '--compact'], { input: symbols.join('\n'), encoding: 'utf8' });
	return out.trim().split('\n');
}

/** Every file importing the entry point, with the nonsending types it builds metadata for. */
export function scan(paths) {
	const offenders = [];
	let files = 0;
	for (const root of paths) {
		for (const file of machOFiles(root)) {
			files++;
			const table = nm(file);
			const imports = table.split('\n').some((l) => l.includes('(undefined)') && l.trimEnd().split(/\s+/).includes(ENTRY));
			if (!imports) continue;
			const accessors = [...new Set(table.split('\n').map((l) => l.trim().split(/\s+/).find((w) => NONSENDING_ACCESSOR.test(w))).filter(Boolean))];
			const types = demangle(accessors.map((a) => a.replace(/^_/, '')));
			offenders.push({ file, types: types.map((t) => t.replace(/^type metadata accessor for /, '')) });
		}
	}
	return { files, offenders };
}

function selfTest() {
	const dir = mkdtempSync(join(tmpdir(), 'ios17-fn-metadata-'));
	try {
		const sdk = execFileSync('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-path'], { encoding: 'utf8' }).trim();
		const compile = (name, body) => {
			const src = join(dir, `${name}.swift`);
			writeFileSync(src, body);
			execFileSync('xcrun', ['swiftc', '-c', src, '-o', join(dir, `${name}.o`), '-module-name', name,
				'-swift-version', '5', '-default-isolation', 'MainActor',
				'-enable-upcoming-feature', 'NonisolatedNonsendingByDefault',
				'-target', 'arm64-apple-ios17.4-simulator', '-sdk', sdk], { stdio: 'inherit' });
			return join(dir, `${name}.o`);
		};
		const bad = compile('Bad', 'func f(_ xs: [() async -> Int]) -> Int { xs.count }\n');
		const good = compile('Good', 'func f(_ xs: [@MainActor () async -> Int], _ ys: [@concurrent () async -> Int]) -> Int { xs.count + ys.count }\n');
		const b = scan([bad]);
		const g = scan([good]);
		if (g.offenders.length !== 0) {
			console.error('self-test FAILED: the annotated snippet was flagged.');
			process.exit(1);
		}
		if (b.offenders.length === 0) {
			// Not a broken check: a toolchain that no longer emits the import
			// (Swift 6.3+) has fixed the bug, and this guard has nothing to do.
			console.log('self-test: this toolchain no longer imports the entry point for nonsending metadata — the compiler bug is fixed here.');
			return;
		}
		console.log(`self-test OK: flagged ${b.offenders[0].types.join(', ') || 'the bad snippet'}; passed the annotated one.`);
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

if (import.meta.url === `file://${process.argv[1]}`) {
	const args = process.argv.slice(2);
	if (args[0] === '--self-test') {
		selfTest();
		process.exit(0);
	}
	if (args.length === 0) {
		console.error('usage: check-ios17-function-metadata.mjs <.app | .xcarchive | Build/Products | Mach-O>... | --self-test');
		process.exit(2);
	}
	const { files, offenders } = scan(args);
	if (files === 0) {
		console.error(`No Mach-O files under ${args.join(', ')} — nothing was checked. Build first.`);
		process.exit(1);
	}
	if (offenders.length === 0) {
		console.log(`OK: ${files} Mach-O file(s); none imports ${ENTRY.slice(1)} (absent from the iOS 17 runtime).`);
		process.exit(0);
	}
	console.error(`These binaries import ${ENTRY.slice(1)}, which iOS 17 does not have — a call to NULL there:\n`);
	for (const o of offenders) {
		console.error(`  ${o.file}`);
		for (const t of o.types) console.error(`      ${t}`);
	}
	console.error('\nGive each function type its isolation: `@MainActor` when the main actor calls it,');
	console.error('`@concurrent` when it runs off it (see FeedbackSender.Loader). Point this script at');
	console.error('Build/Intermediates.noindex to see which source file each one comes from.');
	process.exit(1);
}
