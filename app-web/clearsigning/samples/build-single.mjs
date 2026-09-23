// Build the signer page as ONE file (spec 076, FR-003 / T020).
//
//   node samples/build-single.mjs          → writes dist/sign.html, prints the hash
//   node samples/build-single.mjs --check  → fails if dist/sign.html is not what
//                                            these sources produce
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
// zero-build — `sign.html` and `lib/*.js` are what a person reads and edits —
// and this produces the artefact that is PUBLISHED, at the content-addressed
// path `/b/<sha256>/sign.html`. `--check` keeps the two from drifting.
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
//   · the `memberProof` ceremony's registry fetch (`lib/ceremony.js`
//     `fetchMemberChallenge`). Transaction signing never touches the network,
//     but that one ceremony does, and `connect-src` would hand the page a way
//     out. The fix is for the WALLET to fetch the challenge and pass it in
//     over the channel — the page already refuses any challenge it cannot
//     recompute itself, so nothing is weakened by where it arrives from.
//     Until that lands, memberProof does not work in the single-file build.

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
const SOURCE = join(ROOT, 'sign.html');
const OUT = join(ROOT, 'dist', 'sign.html');

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
	const path = join(ROOT, relative);
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
	// cannot keep. It belongs on the response header where the page is served.
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

	if (check) {
		if (!existsSync(OUT)) {
			console.error('build-single --check: dist/sign.html is missing — run the build');
			process.exit(1);
		}
		const onDisk = readFileSync(OUT, 'utf8');
		if (onDisk !== built.html) {
			console.error(
				`build-single --check: dist/sign.html is stale\n` +
					`  on disk: ${sha256(onDisk)}\n  sources: ${built.hash}`
			);
			process.exit(1);
		}
		// Reproducible, not merely deterministic-looking: build it again and
		// insist on the same bytes. A hash nobody else can reproduce verifies
		// nothing.
		if (build().hash !== built.hash) {
			console.error('build-single --check: two builds of the same sources disagree');
			process.exit(1);
		}
		console.log(`build-single --check: dist/sign.html is current (sha256 ${built.hash})`);
		return;
	}

	mkdirSync(dirname(OUT), { recursive: true });
	writeFileSync(OUT, built.html);
	console.log(
		`build-single: dist/sign.html — ${built.styles.length} stylesheet, ` +
			`${built.scripts.length} scripts, ${built.html.length} bytes`
	);
	console.log(`  sha256 ${built.hash}`);
	console.log(`  publish at https://sign.getvela.app/b/${built.hash}/sign.html`);
	console.log(`  CSP ${built.csp}`);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
	main();
}
