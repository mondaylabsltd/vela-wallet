#!/usr/bin/env node
/**
 * The security docs' claims check (Round 23).
 *
 *   node scripts/ci/check-doc-claims.mjs              check the default documents
 *   node scripts/ci/check-doc-claims.mjs <file.md>…   check these documents instead
 *   node scripts/ci/check-doc-claims.mjs --self-test  prove the checker bites
 *
 * A security document is only worth reading if what it cites is there. For
 * every document checked (default: docs/SECURITY-MODEL.md,
 * docs/security/hardening.md, docs/security/review-pack.md,
 * docs/security/verify.md) this fails when:
 *
 *   - a repository path in a code span (`rust/crates/vela-core/src/app/send.rs`,
 *     `tests/vectors/rules/rp-id.json`, `e2e/page-from-bytes.e2e.ts`) names
 *     nothing in the tree (tracked, or new and not ignored). A path is read
 *     from the repository root, then from the document's directory, then as
 *     the tail of a tracked path (so a
 *     crate-relative `tests/…` path resolves); a bare file name
 *     (`fee_policy.rs`) must be some tracked file's or directory's name;
 *   - a symbol reference names nothing:
 *       `module::symbol` (Rust; `app::corroborate`, `integrity::FROM_BYTES`,
 *       `launch::CheckedPage::opens_from_bytes`): the module resolves to a
 *       tracked `module.rs` / `module/mod.rs`, and every later segment occurs
 *       as a word in it;
 *       `Type::Member` (`ConfirmBlock::BlindRefused`): some tracked source
 *       declares `Type`, and `Member` occurs in a file that declares or
 *       implements it;
 *       `path#symbol` (any language: `app-web/…/catalog.ts#TX_OUTCOME_EVENTS`):
 *       the file exists and the symbol occurs in it as a word;
 *   - a line anchor `path:LINE` carries no symbol, or `path:LINE#symbol` names
 *     a symbol that is not within two lines of LINE. Line numbers rot; prefer
 *     `module::symbol` or `path#symbol`;
 *   - a relative Markdown link points at no tracked file or directory;
 *   - an advisory id (`VELA-2026-001`) has no file in docs/security/advisories/,
 *     or a docs/KNOWN-BUGS.md id (`SEC-PARENT-2`, `R22-PLAN-1`, `R20-S1`) has
 *     no heading there. An id is any token of a family KNOWN-BUGS.md uses.
 *
 * Not checked, on purpose: anything that is not this repository's — absolute
 * paths (`/rustc/<commit>`), URLs, commands with placeholders (`<h>`, `…`, `*`),
 * and other repositories' files, which the documents write as `repo:path`
 * (`vela-relay:docs/fees.md`) so that they read as foreign. Prose is not
 * checked either: this proves a citation exists, not that it supports the
 * sentence around it. That is the reviewer's job.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, normalize, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = normalize(join(dirname(fileURLToPath(import.meta.url)), '..', '..'));

export const DEFAULT_DOCS = [
	'docs/SECURITY-MODEL.md',
	'docs/security/hardening.md',
	'docs/security/review-pack.md',
	'docs/security/verify.md'
];

/** Top-level names a slash path without an extension must start with to be read as ours. */
const KNOWN_FIRST = new Set([
	'rust',
	'app-web',
	'app-ios',
	'app-android',
	'app-desktop',
	'docs',
	'scripts',
	'specs',
	'.github',
	'assets',
	'tests',
	'e2e',
	'samples',
	'extension',
	'src',
	'vendor',
	'lib',
	'Dev',
	'dist',
	'b'
]);

const FILE_EXT =
	/\.(rs|ts|js|mjs|cjs|svelte|kt|kts|swift|json|md|toml|ya?ml|sh|hex|html|plist|txt|patch|py|Dockerfile|lock|entitlements|xcconfig|gradle)$/;

/** Crate/self prefixes stripped from a Rust path before it is resolved. */
const RUST_PREFIX = new Set(['crate', 'self', 'super', 'vela_core', 'vela_agent', 'vela_core_wasm', 'vela_core_uniffi']);
/** Rust paths into crates that are not ours; not checked. */
const RUST_FOREIGN = new Set(['std', 'core', 'alloc', 'alloy', 'alloy_primitives', 'serde', 'serde_json', 'crux_core', 'tokio', 'ureq', 'p256', 'uniffi']);

const SOURCE_EXT = /\.(rs|ts|js|mjs|svelte|kt|swift)$/;

// ---------------------------------------------------------------------------
// The repository, as git tracks it
// ---------------------------------------------------------------------------

export function trackedFiles(root = ROOT) {
	try {
		const out = execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'], { cwd: root, maxBuffer: 256 * 1024 * 1024 });
		return out
			.toString('utf8')
			.split('\0')
			.filter(Boolean);
	} catch {
		throw new Error('check-doc-claims: `git ls-files` failed; run inside the repository');
	}
}

export function indexRepo(files) {
	const fileSet = new Set(files);
	const dirSet = new Set();
	const segments = new Set();
	for (const f of files) {
		const parts = f.split('/');
		for (let i = 1; i < parts.length; i++) dirSet.add(parts.slice(0, i).join('/'));
		for (const p of parts) segments.add(p);
	}
	return { files, fileSet, dirSet, segments, cache: new Map() };
}

function readTracked(repo, root, path) {
	if (repo.cache.has(path)) return repo.cache.get(path);
	let text = '';
	try {
		text = readFileSync(join(root, path), 'utf8');
	} catch {
		text = '';
	}
	repo.cache.set(path, text);
	return text;
}

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const hasWord = (text, word) => new RegExp(`(^|[^A-Za-z0-9_])${escapeRe(word)}($|[^A-Za-z0-9_])`).test(text);

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/** Every tracked file or directory `p` can name, read from the root, the document, or as a tail. */
function resolvePath(repo, p, docDir) {
	const clean = p.replace(/^\.\//, '').replace(/\/+$/, '');
	const isDir = p.endsWith('/');
	const hits = [];
	const consider = (c) => {
		const n = normalize(c).replace(/\\/g, '/');
		if (!isDir && repo.fileSet.has(n)) hits.push(n);
		if (repo.dirSet.has(n)) hits.push(n);
	};
	consider(clean);
	if (docDir) consider(join(docDir, clean));
	if (hits.length) return hits;
	if (!clean.includes('/')) {
		return repo.segments.has(clean) ? [clean] : [];
	}
	const tail = `/${clean}`;
	if (!isDir) for (const f of repo.files) if (f.endsWith(tail)) hits.push(f);
	for (const d of repo.dirSet) if (d.endsWith(tail)) hits.push(d);
	return hits;
}

// ---------------------------------------------------------------------------
// Rust symbols
// ---------------------------------------------------------------------------

function rustModuleFiles(repo, name) {
	const out = [];
	for (const f of repo.files) {
		if (!f.endsWith('.rs')) continue;
		if (f.endsWith(`/${name}.rs`) || f.endsWith(`/${name}/mod.rs`)) out.push(f);
	}
	return out;
}

function submoduleOf(repo, file, name) {
	const base = file.endsWith('/mod.rs') ? dirname(file) : file.replace(/\.rs$/, '');
	for (const c of [`${base}/${name}.rs`, `${base}/${name}/mod.rs`]) if (repo.fileSet.has(c)) return c;
	// `src/lib.rs` / `src/main.rs` declare siblings.
	if (/\/(lib|main)\.rs$/.test(file)) {
		const d = dirname(file);
		for (const c of [`${d}/${name}.rs`, `${d}/${name}/mod.rs`]) if (repo.fileSet.has(c)) return c;
	}
	return null;
}

const DECL = (name) =>
	new RegExp(
		`(struct|enum|trait|union|type|class|interface|object|protocol|typealias|fn|func|fun|const|static|let|var|mod)\\s+${escapeRe(name)}\\b`
	);

function typeFiles(repo, root, name) {
	const decl = DECL(name);
	const impl = new RegExp(`(impl(<[^>]*>)?\\s+(\\w+\\s+for\\s+)?${escapeRe(name)}\\b|extension\\s+${escapeRe(name)}\\b)`);
	const declared = [];
	const implemented = [];
	for (const f of repo.files) {
		if (!SOURCE_EXT.test(f)) continue;
		const t = readTracked(repo, root, f);
		if (!t.includes(name)) continue;
		if (decl.test(t)) declared.push(f);
		else if (impl.test(t)) implemented.push(f);
	}
	return { declared, implemented };
}

/** Why a Rust-style path names nothing, or null when it names something. */
function checkRustPath(repo, root, raw) {
	const path = raw.replace(/\(.*$/, '').replace(/!$/, '');
	let segs = path.split('::').filter(Boolean);
	while (segs.length && RUST_PREFIX.has(segs[0])) segs = segs.slice(1);
	if (!segs.length) return null;
	if (RUST_FOREIGN.has(segs[0])) return null;
	const first = segs[0];
	if (/^[a-z_][a-z0-9_]*$/.test(first)) {
		const starts = rustModuleFiles(repo, first);
		if (!starts.length) return `no Rust module \`${first}\``;
		const failures = [];
		for (const start of starts) {
			let file = start;
			let ok = true;
			let missing = '';
			for (const seg of segs.slice(1)) {
				const sub = /^[a-z_][a-z0-9_]*$/.test(seg) ? submoduleOf(repo, file, seg) : null;
				if (sub && !hasWord(readTracked(repo, root, file), `fn ${seg}`)) {
					file = sub;
					continue;
				}
				if (!hasWord(readTracked(repo, root, file), seg)) {
					ok = false;
					missing = seg;
					break;
				}
			}
			if (ok) return null;
			failures.push(`\`${missing}\` not in ${start}`);
		}
		return failures.join('; ');
	}
	if (/^[A-Z]/.test(first)) {
		const { declared, implemented } = typeFiles(repo, root, first);
		if (!declared.length) return `no tracked source declares \`${first}\``;
		const rest = segs.slice(1);
		if (!rest.length) return null;
		const pool = [...declared, ...implemented];
		for (const f of pool) {
			const t = readTracked(repo, root, f);
			if (rest.every((s) => hasWord(t, s))) return null;
		}
		return `\`${rest.join('::')}\` not found where \`${first}\` is declared or implemented (${declared.slice(0, 3).join(', ')})`;
	}
	return null;
}

// ---------------------------------------------------------------------------
// Ids
// ---------------------------------------------------------------------------

export function knownBugIds(root = ROOT) {
	const text = readFileSync(join(root, 'docs/KNOWN-BUGS.md'), 'utf8');
	const ids = new Set();
	for (const m of text.matchAll(/^##\s+(?:\(former\)\s+)?([A-Z][A-Za-z0-9-]*-[A-Z]?\d+)\b/gm)) ids.add(m[1]);
	const families = new Set([...ids].map((id) => id.replace(/[A-Z]?\d+$/, '')));
	return { ids, families };
}

export function advisoryIds(repo) {
	const ids = new Set();
	for (const f of repo.files) {
		const m = f.match(/^docs\/security\/advisories\/(VELA-\d{4}-\d{3})/);
		if (m) ids.add(m[1]);
	}
	return ids;
}

// ---------------------------------------------------------------------------
// One document
// ---------------------------------------------------------------------------

function stripFences(md) {
	// Fenced blocks hold diagrams, commands and output, not citations; keep the
	// line count so a finding names the right line.
	return md.replace(/^(```|~~~)[^\n]*\n[\s\S]*?^\1[^\n]*$/gm, (b) => b.replace(/[^\n]/g, ''));
}

function lineOf(text, index) {
	let n = 1;
	for (let i = 0; i < index && i < text.length; i++) if (text.charCodeAt(i) === 10) n++;
	return n;
}

/** Each claim in a code span: the tokens worth checking. */
function spanTokens(span) {
	const s = span.trim();
	// A whole span that is a Rust or `Type::Member` path, maybe with call args.
	if (/^[A-Za-z_][\w]*(::[A-Za-z_][\w]*)+(!|\([^`]*\))?$/.test(s)) return [{ kind: 'rust', value: s }];
	const out = [];
	for (const tok of s.split(/\s+/)) {
		if (!tok) continue;
		if (/[<>{}…*$`'"|=,;()[\]]/.test(tok) && !/^[\w./-]+(:\d+)?#[\w$]+$/.test(tok)) continue;
		if (tok.includes('://') || tok.startsWith('/') || tok.startsWith('-') || tok.startsWith('~')) continue;
		if (/^[A-Za-z_][\w]*(::[A-Za-z_][\w]*)+!?$/.test(tok)) {
			out.push({ kind: 'rust', value: tok });
			continue;
		}
		const anchor = tok.match(/^([\w.@/+-]+?)(?::(\d+)(?:-\d+)?)?(?:#([\w$]+))?$/);
		if (!anchor) continue;
		const [, p, line, sym] = anchor;
		const hasExt = FILE_EXT.test(p) || /(^|\/)Dockerfile$/.test(p) || /\.Dockerfile$/.test(p);
		const slash = p.includes('/');
		if (!hasExt && !(slash && (p.endsWith('/') || KNOWN_FIRST.has(p.split('/')[0])))) continue;
		if (!hasExt && !slash) continue;
		// Domain names and versions end in things like `.app`, `.dev`; FILE_EXT keeps them out.
		out.push({ kind: 'path', value: p, line: line ? Number(line) : null, sym: sym ?? null });
	}
	return out;
}

/** The problems in one document; `text`, when given, stands in for the file's contents. */
export function checkDocument(repo, root, docPath, known, advisories, text = null) {
	const problems = [];
	const abs = join(root, docPath);
	if (text === null && !existsSync(abs)) return [{ line: 0, message: `document ${docPath} does not exist` }];
	const raw = text ?? readFileSync(abs, 'utf8');
	const md = stripFences(raw);
	const docDir = dirname(docPath);
	const seen = new Set();
	const report = (index, message) => {
		const line = lineOf(md, index);
		const key = `${line}:${message}`;
		if (seen.has(key)) return;
		seen.add(key);
		problems.push({ line, message });
	};

	// Code spans.
	for (const m of md.matchAll(/(`+)([^`\n]+?)\1/g)) {
		for (const t of spanTokens(m[2])) {
			if (t.kind === 'rust') {
				const why = checkRustPath(repo, root, t.value);
				if (why) report(m.index, `\`${t.value}\`: ${why}`);
				continue;
			}
			const hits = resolvePath(repo, t.value, docDir);
			if (!hits.length) {
				report(m.index, `\`${t.value}\`: no tracked file or directory`);
				continue;
			}
			const files = hits.filter((h) => repo.fileSet.has(h));
			if (t.line !== null && !t.sym) {
				report(m.index, `\`${t.value}:${t.line}\`: a line anchor needs a symbol (\`path:LINE#symbol\`), or better \`path#symbol\``);
				continue;
			}
			if (t.sym) {
				if (!files.length) {
					report(m.index, `\`${t.value}#${t.sym}\`: a symbol needs a file, not a directory`);
					continue;
				}
				const good = files.some((f) => {
					const text = readTracked(repo, root, f);
					if (t.line === null) return hasWord(text, t.sym);
					const lines = text.split('\n');
					const from = Math.max(0, t.line - 3);
					const to = Math.min(lines.length, t.line + 2);
					return from < lines.length && hasWord(lines.slice(from, to).join('\n'), t.sym);
				});
				if (!good)
					report(
						m.index,
						t.line === null
							? `\`${t.value}#${t.sym}\`: \`${t.sym}\` does not occur in ${files[0]}`
							: `\`${t.value}:${t.line}#${t.sym}\`: \`${t.sym}\` is not within two lines of line ${t.line}`
					);
			}
		}
	}

	// Relative Markdown links (outside code spans).
	const noSpans = md.replace(/(`+)[^`\n]+?\1/g, (s) => ' '.repeat(s.length));
	for (const m of noSpans.matchAll(/\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
		const target = m[1];
		if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) continue;
		const path = decodeURIComponent(target.split('#')[0]);
		if (!path) continue;
		const resolved = normalize(join(docDir, path)).replace(/\\/g, '/').replace(/\/+$/, '');
		if (resolved.startsWith('..')) {
			report(m.index, `link \`${target}\` leaves the repository`);
			continue;
		}
		if (!repo.fileSet.has(resolved) && !repo.dirSet.has(resolved) && !existsSync(join(root, resolved)))
			report(m.index, `link \`${target}\`: ${resolved} is not in the repository`);
	}

	// Ids.
	for (const m of noSpans.matchAll(/\bVELA-\d{4}-\d{3}\b/g)) {
		if (!advisories.has(m[0])) report(m.index, `advisory ${m[0]} has no file in docs/security/advisories/`);
	}
	for (const m of md.matchAll(/\bVELA-\d{4}-\d{3}\b/g)) {
		if (!advisories.has(m[0])) report(m.index, `advisory ${m[0]} has no file in docs/security/advisories/`);
	}
	for (const m of md.matchAll(/(?<![A-Za-z0-9-])([A-Z][A-Za-z0-9]*(?:-[A-Z][A-Za-z0-9]*)*-[A-Z]?\d+)(?![A-Za-z0-9-])/g)) {
		const id = m[1];
		const family = id.replace(/[A-Z]?\d+$/, '');
		if (!known.families.has(family)) continue;
		if (!known.ids.has(id)) report(m.index, `KNOWN-BUGS id ${id} has no heading in docs/KNOWN-BUGS.md`);
	}
	return problems;
}

// ---------------------------------------------------------------------------
// Self-test: the checker must bite
// ---------------------------------------------------------------------------

function selfTest() {
	const files = trackedFiles();
	const repo = indexRepo(files);
	const known = knownBugIds();
	const advisories = advisoryIds(repo);
	const cases = [
		['`rust/crates/vela-core/src/app/sign_confirm.rs`', 0],
		['`rust/crates/vela-core/src/app/no_such_file.rs`', 1],
		['`sign_confirm::posture`', 0],
		['`sign_confirm::no_such_symbol_r23`', 1],
		['`integrity::FROM_BYTES`', 0],
		['`ConfirmBlock::BlindRefused`', 0],
		['`ConfirmBlock::NoSuchVariantR23`', 1],
		['`NoSuchTypeR23::X`', 1],
		['`rust/crates/vela-core/src/app/sign_confirm.rs#HOLD_TO_CONFIRM_MS`', 0],
		['`rust/crates/vela-core/src/app/sign_confirm.rs#NO_SUCH_R23`', 1],
		['`rust/crates/vela-core/src/app/sign_confirm.rs:147`', 1],
		['`tests/vectors/rules/rp-id.json`', 0],
		['`fee_policy.rs`', 0],
		['`no_such_r23.rs`', 1],
		['[x](../SECURITY.md)', 0],
		['[x](../NO-SUCH-R23.md)', 1],
		['VELA-2026-001', 0],
		['VELA-2026-999', 1],
		['SEC-PARENT-2', 0],
		['SEC-PARENT-99', 1],
		['`curl -s https://sign.getvela.app/b/<h>/sign | shasum -a 256`', 0],
		['`/rustc/<commit>/`', 0],
		['`vela-relay:docs/fees.md`', 0]
	];
	const tmpName = 'docs/.check-doc-claims-self-test.md';
	let failed = 0;
	for (const [text, want] of cases) {
		const problems = checkDocument(repo, ROOT, tmpName, known, advisories, text);
		const got = problems.length;
		const ok = want === 0 ? got === 0 : got > 0;
		if (!ok) {
			failed++;
			console.error(`self-test: ${JSON.stringify(text)} → ${got} problem(s), wanted ${want ? 'some' : 'none'}`);
			for (const p of problems) console.error(`    ${p.message}`);
		}
	}
	if (failed) {
		console.error(`check-doc-claims self-test: ${failed} of ${cases.length} cases wrong`);
		process.exit(1);
	}
	console.log(`check-doc-claims self-test: ${cases.length} cases bite as expected`);
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function main() {
	const args = process.argv.slice(2);
	if (args.includes('--self-test')) return selfTest();
	const docs = args.length ? args : DEFAULT_DOCS;
	const files = trackedFiles();
	const repo = indexRepo(files);
	const known = knownBugIds();
	const advisories = advisoryIds(repo);
	let total = 0;
	for (const doc of docs) {
		const rel = relative(ROOT, join(process.cwd(), doc)).replace(/\\/g, '/');
		const docPath = existsSync(join(ROOT, doc)) ? doc : rel;
		const problems = checkDocument(repo, ROOT, docPath, known, advisories);
		if (!problems.length) {
			console.log(`ok    ${docPath}`);
			continue;
		}
		total += problems.length;
		for (const p of problems) console.error(`${docPath}:${p.line}: ${p.message}`);
	}
	if (total) {
		console.error(`check-doc-claims: ${total} citation(s) name nothing in this tree`);
		process.exit(1);
	}
	console.log('check-doc-claims: every cited path, symbol and id exists');
}

main();
