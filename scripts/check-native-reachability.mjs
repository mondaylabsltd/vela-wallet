#!/usr/bin/env node
/**
 * Every screen family a native client ships must be reachable from that
 * client's navigation root.
 *
 * This exists because spec 022 shipped 9,695 lines of drawn, translated,
 * fixture-complete UI on three clients that no person could open, and every
 * gate in the repository stayed green throughout. The specific shapes:
 *
 *   - desktop  `src/explore/` and `src/signing/` exist on disk and are not
 *              declared in `main.rs`, so rustc never reads them. Rust does not
 *              warn about a directory nobody declared.
 *   - Android  `feature/explore/` and `feature/signing/` compile (the package
 *              is on the source path) but no `VelaDestinations` route and no
 *              `composable(...)` reaches them.
 *   - iOS      `Features/Explore/` and `Features/Signing/` compile (folder-
 *              synced target) but `PageOverride.Page` has no case and
 *              `ExploreScreen(` is never instantiated.
 *
 * Note what those three have in common: on every platform the compiler was
 * happy. Only navigation can see this, which is why the check is navigation-
 * shaped rather than a lint.
 *
 * Deliberately static — no cargo, no gradle, no xcodebuild. It runs in the
 * `app` job, which already exists, in well under a second. A guard that needs
 * a 25-minute toolchain is a guard somebody eventually skips.
 *
 * Usage: node scripts/check-native-reachability.mjs
 * Exit 0 = every screen family is reachable. Exit 1 = names the orphans.
 */

import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(ROOT, p), 'utf8');
const dirs = (p) =>
	readdirSync(join(ROOT, p), { withFileTypes: true })
		.filter((e) => e.isDirectory())
		.map((e) => e.name);

/**
 * Screen families that are legitimately not routed, each with its reason.
 *
 * EMPTY, and that is the finding: every candidate exemption checked out as
 * genuinely reachable (desktop `ui`/`ctap`/`executor` are all declared in
 * main.rs; iOS `Gallery` is instantiated at RootView.swift:70). A guard that
 * ships pre-populated with exemptions nobody needs is a guard the next orphan
 * hides behind. An entry here must name the screen and the reason it has no
 * route — never a bare name, and never a whole directory added to make a red
 * check go green.
 */
const EXEMPT = {
	desktop: new Set([]),
	android: new Set([]),
	ios: new Set([])
};

const failures = [];

// --- desktop: a directory under src/ that main.rs never declares ------------
{
	const declared = new Set(
		[...read('app-desktop/vela-wallet/src/main.rs').matchAll(/^mod\s+([a-z_]+);/gm)].map(
			(m) => m[1]
		)
	);
	const present = dirs('app-desktop/vela-wallet/src');
	const orphans = present.filter((d) => !declared.has(d) && !EXEMPT.desktop.has(d));
	if (orphans.length) {
		failures.push(
			`desktop: ${orphans.length} module(s) on disk that src/main.rs never declares, ` +
				`so rustc never compiles them: ${orphans.join(', ')}\n` +
				`         Fix: add \`mod <name>;\` to app-desktop/vela-wallet/src/main.rs ` +
				`and route to it.`
		);
	}
}

// A screen family is reachable if the navigation root renders one of its
// composables/views, OR if some already-reachable family does. Reachability is
// TRANSITIVE and both platforms rely on it: `signing` is never rendered by the
// navigation root — the browser raises it — so a check that only looked one hop
// from the root would call the signing sheet dead while a person can open it.
//
// Imports are stripped before anything is matched. An `import
// ...feature.explore.ExploreScreen` line survives the deletion of every call to
// it (Kotlin warns about an unused import; it does not error), so counting
// imports as usage lets a dead screen look reachable. Found the honest way, by
// deleting the call and watching an earlier version of this check stay green.
function reachableFamilies({ base, navPath, isSource, declPattern }) {
	const strip = (text) =>
		text
			.split('\n')
			.filter((line) => !line.trimStart().startsWith('import '))
			.join('\n');

	const families = dirs(base);
	const declares = new Map(); // family -> Set<symbol it declares>
	const body = new Map(); // family -> its own source, imports stripped

	for (const family of families) {
		const files = readdirSync(join(ROOT, base, family)).filter(isSource);
		const text = files.map((f) => read(join(base, family, f))).join('\n');
		body.set(family, strip(text));
		declares.set(
			family,
			new Set([...text.matchAll(declPattern)].map((m) => m[1]))
		);
	}

	const renders = (text, family) => [...declares.get(family)].some((sym) => text.includes(`${sym}(`));

	const reached = new Set();
	let frontier = [strip(read(navPath))];
	while (frontier.length) {
		const next = [];
		for (const text of frontier) {
			for (const family of families) {
				if (reached.has(family)) continue;
				if (renders(text, family)) {
					reached.add(family);
					next.push(body.get(family));
				}
			}
		}
		frontier = next;
	}
	return families.filter((f) => !reached.has(f));
}

// --- Android ---------------------------------------------------------------
{
	const orphans = reachableFamilies({
		base: 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature',
		navPath: 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet/navigation/VelaNavHost.kt',
		isSource: (f) => f.endsWith('.kt'),
		declPattern: /^fun ([A-Z][A-Za-z0-9]*)\(/gm
	}).filter((f) => !EXEMPT.android.has(f));
	if (orphans.length) {
		failures.push(
			`android: ${orphans.length} feature package(s) nothing reachable from ` +
				`VelaNavHost renders: ${orphans.join(', ')}\n` +
				`         Fix: add a VelaDestinations route and a composable(...), or render ` +
				`it from a screen that already has one.`
		);
	}
}

// --- iOS -------------------------------------------------------------------
{
	const orphans = reachableFamilies({
		base: 'app-ios/VelaWallet/VelaWallet/Features',
		navPath: 'app-ios/VelaWallet/VelaWallet/App/RootView.swift',
		isSource: (f) => f.endsWith('.swift'),
		declPattern: /^struct ([A-Z][A-Za-z0-9]*): View/gm
	}).filter((f) => !EXEMPT.ios.has(f));
	if (orphans.length) {
		failures.push(
			`ios: ${orphans.length} Features/ folder(s) nothing reachable from ` +
				`RootView.swift renders: ${orphans.join(', ')}\n` +
				`     Fix: add a PageOverride.Page case and render it from RootView, or ` +
				`render it from a view that RootView already reaches.`
		);
	}
}

if (failures.length) {
	console.error('Unreachable screen families — drawn, compiled or not, and unopenable:\n');
	for (const f of failures) console.error(`  ${f}\n`);
	console.error(
		'Each of these is UI a person cannot get to. If one is intentionally not\n' +
			'routed, add it to EXEMPT in this file WITH the reason — never silently.'
	);
	process.exit(1);
}

console.log('native reachability: every screen family is reachable from its navigation root');
