// Build the signer page as ONE file (spec 076, FR-003 / T020).
//
//   bun samples/build-single.mjs           → adds dist/b/<sha256>/sign.html and
//                                            rewrites dist/index.json
//   bun samples/build-single.mjs --check   → fails if this build is not already
//                                            published, or the index has drifted
//
// Zero dependencies, like the page it builds: `node:crypto`, `node:fs`,
// `node:path`, `node:url` and nothing else — no npm, no lockfile, nothing to
// audit. Bun is the runtime (owner, 2026-09-23), and Node runs it unchanged.
//
// **They agree, and that is the point.** The same sources under Bun 1.4.2 and
// under Node 22 produce the same 312257 bytes and the same sha256
// (810db7c5…, measured). A build whose output depended on which runtime ran it
// would be a build nobody else could reproduce, and an unreproducible hash
// verifies nothing.
//
// Why one file. The wallet checks the page before opening it by hashing the
// bytes it receives (076). One file means ONE hash covers every executable
// byte, with no manifest of subresources to keep honest — and a subresource is
// exactly where a replaced build would hide, because nobody hashes those.
//
// The second prize is bigger: with no subresources the page can carry
// `default-src 'none'` INSIDE the hashed bytes, so a page that matches the
// hash structurally cannot send what it sees anywhere. It renders
// attacker-supplied data — a dApp's transaction, token names, a contract's
// own strings — and that is the one place a signer page must not be able to
// leak from.
//
// This does NOT replace `sign.html`. The folder stays hand-written and
// zero-build — `src/sign.html` and `src/lib/*.js` are what a person reads and
// edits —
// and this produces the artefact that is PUBLISHED.
//
// **`dist/` is the deployment, and it is in git** (owner, 2026-09-23). It holds
// EVERY version ever published, each at its own content-addressed path, plus an
// index of them:
//
//     dist/index.json            what is published
//     dist/b/<sha256>/sign.html  a version, for ever
//
// Deploying is copying that directory. Two things follow, and both are the
// point: "a published path never goes away" stops being a discipline and
// becomes a fact anyone can see in the history — losing a version means
// deleting a committed file — and the index is GENERATED from the directory, so
// it cannot claim a version that is not there.
//
// A build therefore APPENDS. It never overwrites and never removes; the only
// way a version leaves is a deliberate deletion, in a commit, with a reason.
//
// (Until 2026-09-23 there was a harder reason: the folder was also a Chrome MV3
// extension, and MV3 refuses to load a page with an inline <script> at all.
// The extension is gone; the source/artefact split stayed because it is the
// better shape, not because it was forced.)
//
// Reproducibility is load-bearing: if this build is not reproducible the hash
// is not reproducible, and nobody can independently verify a published
// version. So it does exactly one thing — concatenate, in the order the page
// already names — with no minifier, no timestamp and no environment in it.
//
// What the single file gives up, deliberately:
//
//   · the favicon — decoration, and a subresource;
//   · remote chain/token logos from `ethereum-data.getvela.app`. The page
//     already falls back to a drawn letter when an image fails ("a logo is
//     never allowed to leave a hole where a symbol should be", render.js), and
//     the identicon — the actual anti-poisoning signal — is computed locally;
//   · any network at all. The `memberProof` ceremony used to fetch its
//     challenge from the registry; it now computes it from what the request
//     carries (`lib/ceremony.js`), so no ceremony needs `connect-src`.

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
/// The signing page's source. `src/` is the boundary: what is in it is what
/// the artefact is made of, and nothing else in this folder is.
const SRC = join(ROOT, 'src');
const SOURCE = join(SRC, 'sign.html');
const DIST = join(ROOT, 'dist');
const INDEX = join(DIST, 'index.json');
const pageAt = (hash) => join(DIST, 'b', hash, 'sign.html');

const sha256 = (text) => createHash('sha256').update(text, 'utf8').digest('hex');
const cspHash = (text) => `'sha256-${createHash('sha256').update(text, 'utf8').digest('base64')}'`;

/** The stylesheet and the scripts the page names, in the order it names them. */
function partsOf(html) {
	const styles = [...html.matchAll(/<link\s+rel="stylesheet"\s+href="([^"]+)"\s*\/?>/g)].map(
		(m) => m[1]
	);
	const scripts = [...html.matchAll(/<script\s+src="([^"]+)"\s*><\/script>/g)].map((m) => m[1]);
	return { styles, scripts };
}

function read(relative) {
	const path = join(SRC, relative);
	if (!existsSync(path)) throw new Error(`sign.html names ${relative}, which is not there`);
	// LF only, and no trailing-newline surprises: the bytes must not depend on
	// whoever last edited the file on which platform.
	return readFileSync(path, 'utf8').replace(/\r\n/g, '\n').replace(/\s*$/, '\n');
}

export function build() {
	const html = readFileSync(SOURCE, 'utf8').replace(/\r\n/g, '\n');
	const { styles, scripts } = partsOf(html);
	if (styles.length === 0 || scripts.length === 0) {
		throw new Error('sign.html names no stylesheet or no scripts — has its shape changed?');
	}

	// ONE style block and ONE script block: each is a single CSP hash, and the
	// scripts are all IIFEs registering on a namespace, so concatenating them
	// in the page's own order is what the browser already does.
	const styleText = `\n${styles.map(read).join('\n')}`;
	const scriptText = `\n${scripts.map(read).join('\n')}`;

	// `frame-ancestors` is deliberately absent: a <meta> CSP cannot carry it
	// (browsers ignore it there), so writing it would be a claim this file
	// cannot keep. It is on the response header where the page is served
	// (`HEADERS`, spec 102).
	const csp = [
		"default-src 'none'",
		`script-src ${cspHash(scriptText)}`,
		`style-src ${cspHash(styleText)}`,
		// Identicons and small logos the wallet passes in arrive as data: URIs.
		'img-src data:',
		"base-uri 'none'",
		"form-action 'none'"
	].join('; ');

	let out = html;
	// Drop every subresource the page names; they are about to be inline.
	out = out.replace(/\s*<link\s+rel="stylesheet"[^>]*>/g, '');
	out = out.replace(/\s*<link\s+rel="(?:alternate )?icon"[^>]*>/g, '');
	out = out.replace(/\s*<script\s+src="[^"]+"\s*><\/script>/g, '');
	// Every insertion goes through a replacer FUNCTION, never a replacement
	// STRING. In a replacement string `$&`, `` $` ``, `$'` and `$1` are
	// substitutions, and JavaScript source is full of `$` — so the bytes that
	// land in the file would not be the bytes that were hashed, and the browser
	// refuses to run the script. Measured, not guessed: the first build did
	// exactly that, and the page loaded with its own code blocked.
	// The CSP goes FIRST in <head>, so it governs everything after it.
	out = out.replace(
		'<meta charset="utf-8" />',
		() => `<meta charset="utf-8" />\n    <meta http-equiv="Content-Security-Policy" content="${csp}" />`
	);
	// The element's text content must be EXACTLY what was hashed: a CSP hash
	// covers the content byte for byte, so even the indentation before a
	// closing tag would make the browser refuse to run it.
	out = out.replace('</head>', () => `  <style>${styleText}</style>\n  </head>`);
	out = out.replace('</body>', () => `  <script>${scriptText}</script>\n  </body>`);
	// One trailing newline, always.
	out = `${out.replace(/\s*$/, '')}\n`;

	return { html: out, hash: sha256(out), csp, styles, scripts };
}

/**
 * Every version the dist directory actually holds, by reading it — never by
 * trusting the index, which is the thing being generated.
 */
function publishedVersions() {
	const root = join(DIST, 'b');
	if (!existsSync(root)) return [];
	return readdirSync(root, { withFileTypes: true })
		.filter((entry) => entry.isDirectory() && /^[0-9a-f]{64}$/.test(entry.name))
		.map((entry) => entry.name)
		.filter((hash) => existsSync(pageAt(hash)))
		.sort();
}

/** The index, as bytes. Sorted, so the file is stable in git. */
function indexText(versions) {
	return `${JSON.stringify({ versions: [...versions].sort() }, null, 2)}\n`;
}

function main() {
	const check = process.argv.includes('--check');
	const built = build();
	const leaks = [...built.html.matchAll(/(?:src|href)="([^"]*)"/g)]
		.map((m) => m[1])
		.filter((v) => !v.startsWith('data:') && !v.startsWith('#'));
	if (leaks.length > 0) {
		console.error(`build-single: the page still names subresources: ${leaks.join(', ')}`);
		process.exit(1);
	}

	const already = publishedVersions();
	const wanted = indexText(already.includes(built.hash) ? already : [...already, built.hash]);

	if (check) {
		if (!existsSync(pageAt(built.hash))) {
			console.error(
				`build-single --check: these sources build ${built.hash}, which is not published.\n` +
					`  run: bun samples/build-single.mjs`
			);
			process.exit(1);
		}
		const onDisk = readFileSync(pageAt(built.hash), 'utf8');
		if (onDisk !== built.html) {
			// Same path, different bytes: impossible unless the file was edited
			// by hand, and a content-addressed path that lies about its content
			// is the one thing this whole design cannot tolerate.
			console.error(
				`build-single --check: dist/b/${built.hash}/sign.html does NOT hash to its own path\n` +
					`  on disk: ${sha256(onDisk)}`
			);
			process.exit(1);
		}
		// Every published path must hash to its own name, not just this one.
		for (const hash of already) {
			const bytes = readFileSync(pageAt(hash), 'utf8');
			if (sha256(bytes) !== hash) {
				console.error(`build-single --check: dist/b/${hash}/ holds ${sha256(bytes)}`);
				process.exit(1);
			}
		}
		if (!existsSync(INDEX) || readFileSync(INDEX, 'utf8') !== indexText(already)) {
			console.error('build-single --check: dist/index.json does not match what dist/b/ holds');
			process.exit(1);
		}
		const headers = join(DIST, '_headers');
		if (!existsSync(headers) || readFileSync(headers, 'utf8') !== HEADERS) {
			console.error('build-single --check: dist/_headers is missing or not this build\'s header rules');
			process.exit(1);
		}
		const root = join(DIST, 'sign.html');
		if (!existsSync(root) || readFileSync(root, 'utf8') !== built.html) {
			console.error(
				'build-single --check: dist/sign.html is not this build — a client that has ' +
					'not read the index would be served something else, or nothing'
			);
			process.exit(1);
		}
		// Reproducible, not merely deterministic-looking.
		if (build().hash !== built.hash) {
			console.error('build-single --check: two builds of the same sources disagree');
			process.exit(1);
		}
		console.log(
			`build-single --check: ${built.hash} is published; ` +
				`${already.length} version(s) in dist, index agrees`
		);
		return;
	}

	const fresh = !existsSync(pageAt(built.hash));
	mkdirSync(dirname(pageAt(built.hash)), { recursive: true });
	writeFileSync(pageAt(built.hash), built.html);
	writeFileSync(INDEX, wanted);
	// The root copy, so `<base>/sign.html` is not a 404.
	//
	// A client that has not yet read the index has no version to ask for, and
	// a deployment that served ONLY `b/<hash>/sign.html` gave it nowhere to go
	// — measured against the real endpoint: the launch URL fell back to the
	// root and the browser showed "No webpage was found".
	//
	// It is the same bytes as this build, written from the same string, so
	// there is nothing to keep in sync by hand; `--check` re-hashes it. A
	// client that opens the root gets the current page UNVERIFIED, which is
	// exactly what every client did before 076 — the check, when it is
	// enforced, always asks by hash.
	writeFileSync(join(DIST, 'sign.html'), built.html);
	// Spec 079: a version at `b/<sha256>/` never changes — its path IS its
	// content — so the browser may keep it for a year and open it with no
	// network (a signature needs none). The root stays revalidated: it moves
	// with every release. Spec 102: no page here may be framed (`HEADERS`).
	// Cloudflare Pages reads this file from the deploy root.
	writeFileSync(join(DIST, '_headers'), HEADERS);
	console.log(
		`build-single: ${fresh ? 'published' : 'already published'} ` +
			`dist/b/${built.hash}/sign.html — ${built.html.length} bytes, ` +
			`${built.styles.length} stylesheet, ${built.scripts.length} scripts`
	);
	console.log(`  index now lists ${JSON.parse(wanted).versions.length} version(s)`);
	console.log(`  deploy: copy dist/ to the signer host's root`);
	console.log(`  allow-set: add ${built.hash} to BUILD_ALLOWED, at the FRONT`);
	console.log(`  CSP ${built.csp}`);
}

/**
 * The host's header rules (Cloudflare Pages `_headers`). Every matching rule
 * applies, so a version under `/b/` gets both blocks.
 *
 * `/*` — never inside another page (spec 102 R7). A frame lets the page around
 * it draw over the card and the slide while the passkey prompt still names
 * Vela's domain. `frame-ancestors` has to be a response header: browsers
 * ignore it in the page's own `<meta>` CSP (see `build()`). `X-Frame-Options`
 * says the same to browsers that predate `frame-ancestors`. A header CSP is
 * enforced alongside the `<meta>` one, so this adds a restriction and lifts
 * none. Headers are not part of a version's hash, so this reaches every
 * version the host serves, old ones included.
 *
 * `/*` also says `no-transform`: the host must hand the browser the published
 * bytes. Cloudflare Web Analytics injected its beacon `<script>` before
 * `</body>` of every response a browser asked for as HTML, so the bytes a tab
 * ran hashed to a version nobody published (and the page was no longer
 * zero-dependency). `no-transform` tells the host not to rewrite the
 * response; the owner also turns the injection off for this site.
 *
 * `/b/*` — spec 079: a version never changes, so the browser may keep it for
 * a year and open it with no network. Pages joins this `Cache-Control` with
 * the one under `/*`.
 */
export const HEADERS = [
	'/*',
	"  Content-Security-Policy: frame-ancestors 'none'",
	'  X-Frame-Options: DENY',
	'  Cache-Control: no-transform',
	'',
	'/b/*',
	'  Cache-Control: public, max-age=31536000, immutable',
	'',
].join('\n');

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
	main();
}
