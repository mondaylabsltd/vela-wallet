/**
 * `bun run i18n:stamp:doc <locale>/<file>.md …` — stamp ONE translated doc.
 *
 * `i18n:stamp` records agreement for every doc in every locale at once, which
 * is right after a full translation round and wrong at every other moment: a
 * translator who has finished one file would, by running it, silently certify
 * thirteen they have not touched. That is not a hypothetical — spec 081's
 * release-provenance pass updated one English doc, and stamping would have
 * marked thirteen untranslated copies fresh.
 *
 * So this stamps exactly the files named, and refuses one whose English source
 * is missing. It makes no judgement about whether the translation is finished;
 * naming the file is that judgement.
 *
 *   bun run i18n:stamp:doc zh/whitepaper.md ja/whitepaper.md
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { docFingerprint, withSource } from './doc-fingerprint.ts';

const here = (p: string) => fileURLToPath(new URL(`../${p}`, import.meta.url));
const DOCS = 'src/content/docs';

const targets = process.argv.slice(2);
if (targets.length === 0) {
	console.error('usage: bun run i18n:stamp:doc <locale>/<file>.md [...]');
	process.exit(2);
}

let failed = false;
for (const target of targets) {
	const relative = target.replace(/^.*src\/content\/docs\//, '');
	const parts = relative.split('/');
	if (parts.length !== 2 || !parts[1].endsWith('.md')) {
		console.error(`${target}: expected <locale>/<file>.md`);
		failed = true;
		continue;
	}
	const [locale, file] = parts;
	const translated = here(`${DOCS}/${locale}/${file}`);
	const english = here(`${DOCS}/${file}`);
	if (!existsSync(translated)) {
		console.error(`${target}: no such translated doc`);
		failed = true;
		continue;
	}
	if (!existsSync(english)) {
		console.error(`${target}: no English ${file} to be a translation of`);
		failed = true;
		continue;
	}
	const before = readFileSync(translated, 'utf8');
	const fingerprint = docFingerprint(english);
	const after = withSource(before, fingerprint);
	if (after === before) {
		console.log(`${locale}/${file}: already ${fingerprint}`);
		continue;
	}
	writeFileSync(translated, after);
	console.log(`${locale}/${file}: stamped ${fingerprint}`);
}

process.exit(failed ? 1 : 0);
