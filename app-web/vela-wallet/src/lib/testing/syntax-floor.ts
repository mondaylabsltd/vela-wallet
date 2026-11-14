/**
 * The syntax floor of the extension's bundled scripts — TESTS ONLY.
 *
 * `extension/build.mjs` bundles the page-side scripts with Bun's bundler,
 * which (unlike esbuild's `target`) does not lower syntax for a browser
 * version. So the floor is held here instead: every bundled script must parse
 * as a CLASSIC script on the oldest browsers the packages promise — Safari
 * 17.4 (the iOS app's floor, `extension/lib/safari-manifest.js`) and Chrome
 * 122 (`minimum_chrome_version`).
 *
 * What it refuses is the syntax those browsers do not have, plus what a
 * classic script can never hold:
 * - `import` / `export` statements and `import.meta` in a CLASSIC script —
 *   content scripts, the worker, the doorways and the boot scripts the build
 *   lifts out of each page — where each is a syntax error (the reason for
 *   bundling). The app's own chunks are ES modules (`{ module: true }`) and
 *   may hold them;
 * - `using` / `await using` (Chrome 134, no Safari);
 * - decorators (no browser);
 * - regular-expression modifiers, `(?i:…)` (Chrome 125, Safari 26).
 *
 * Parsed with the TypeScript compiler the app already builds with — no
 * parser added for this.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import ts from 'typescript';

/** The constructs in `source` the floor refuses, as `"<what> at <line>"`. */
export function belowFloor(
	source: string,
	fileName = 'bundle.js',
	{ module = false }: { module?: boolean } = {}
): string[] {
	const file = ts.createSourceFile(
		fileName,
		source,
		ts.ScriptTarget.Latest,
		true,
		ts.ScriptKind.JS
	);
	const found: string[] = [];
	const at = (node: ts.Node, what: string) =>
		found.push(`${what} at ${file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1}`);

	// A parse error is a script no browser runs at all.
	const parseErrors = (file as unknown as { parseDiagnostics?: ts.Diagnostic[] }).parseDiagnostics;
	for (const diagnostic of parseErrors ?? []) {
		found.push(`syntax error: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ')}`);
	}

	/** What only a module may hold: imports, exports, `import.meta`. */
	const moduleOnly = (node: ts.Node) => {
		if (ts.isImportDeclaration(node) || ts.isImportEqualsDeclaration(node)) at(node, 'import');
		else if (ts.isExportDeclaration(node) || ts.isExportAssignment(node)) at(node, 'export');
		else if (
			ts.isMetaProperty(node) &&
			node.keywordToken === ts.SyntaxKind.ImportKeyword &&
			node.name.text === 'meta'
		)
			at(node, 'import.meta');
		// `export function …` / `export const …` carry the keyword as a modifier.
		else if (
			ts.canHaveModifiers(node) &&
			ts.getModifiers(node)?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword)
		)
			at(node, 'export');
	};

	const visit = (node: ts.Node) => {
		if (!module) moduleOnly(node);
		if (ts.isDecorator(node)) at(node, 'decorator');
		else if (ts.isVariableDeclarationList(node)) {
			const scope = node.flags & ts.NodeFlags.BlockScoped;
			if (scope === ts.NodeFlags.AwaitUsing) at(node, 'await using');
			else if (scope === ts.NodeFlags.Using) at(node, 'using');
		} else if (
			node.kind === ts.SyntaxKind.RegularExpressionLiteral &&
			REGEX_MODIFIERS.test((node as ts.RegularExpressionLiteral).text)
		)
			at(node, 'regex modifiers');
		ts.forEachChild(node, visit);
	};
	visit(file);
	return found;
}

/** `(?i:…)`, `(?-s:…)`, `(?i-m:…)` — never `(?:…)`, and never an escaped `\(`. */
const REGEX_MODIFIERS = /(?<!\\)\(\?(?:[ims]+(?:-[ims]*)?|-[ims]+):/;

/** Every file under `dir`, as paths relative to it. */
function filesUnder(dir: string, prefix = ''): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		const relative = prefix ? `${prefix}/${name}` : name;
		return statSync(path).isDirectory() ? filesUnder(path, relative) : [relative];
	});
}

/**
 * Every script an extension package at `dist` ships, held to the floor, each
 * as it is loaded: as an ES module if it is one of the app's chunks under
 * `app/` or a page loads it with `type="module"`, else as a classic script —
 * what `build.mjs` bundles with Bun, the boot scripts it lifts out of each
 * page, any script it copies. A page that loads a chunk as a classic script
 * is refused too. `{}` when all is well, else what each file breaks.
 */
export function packageBelowFloor(dist: string): Record<string, string[]> {
	const below: Record<string, string[]> = {};
	const all = filesUnder(dist);
	const modules = new Set(all.filter((file) => file.startsWith('app/')));
	for (const page of all.filter((file) => file.endsWith('.html'))) {
		const folder = page.includes('/') ? page.slice(0, page.lastIndexOf('/') + 1) : '';
		for (const tag of readFileSync(join(dist, page), 'utf8').match(/<script\b[^>]*>/g) ?? []) {
			const src = /\ssrc=["']?([^"' >]+)/.exec(tag)?.[1];
			if (!src) continue;
			const script = src.startsWith('/') ? src.slice(1) : join(folder, src);
			if (/\stype=["']?module/.test(tag)) modules.add(script);
			else if (script.startsWith('app/'))
				(below[page] ??= []).push(`loads a module as a classic script: ${tag}`);
		}
	}
	for (const script of all.filter((file) => file.endsWith('.js'))) {
		const found = belowFloor(readFileSync(join(dist, script), 'utf8'), script, {
			module: modules.has(script)
		});
		if (found.length) below[script] = found;
	}
	return below;
}
