/**
 * The e2e suites' server: the built worker and its assets on `workerd`, with
 * nothing in front of them.
 *
 *   node e2e/preview.mjs [port]        (after `npm run build`; default 4173)
 *
 * WHY NOT `wrangler dev` (the `preview` script, which the suites used to run
 * against). `wrangler dev` does not serve the worker: it serves a ProxyWorker
 * that forwards each request to it, and that proxy reports ONE forward that
 * fails — "Network connection lost.", which a busy machine is enough to
 * produce — as the end of the dev session. Caught with `--log-level debug`:
 *
 *     Error in ProxyController: Error inside ProxyWorker
 *       cause: { message: 'Network connection lost.' }
 *
 * What a person sees is an empty "✘ ERROR" and wrangler EXITING 0; every test
 * still to run then fails on a refused connection. That is the preview
 * "dying mid-run" the config pinned its workers for (specs 026, 027): in one
 * run of the whole suite on a loaded machine it exited three times, in the
 * next eleven. Nothing in the app had failed.
 *
 * WHAT RUNS HERE is what ships. `wrangler deploy --dry-run` writes the deploy
 * bundle — in a project directory of its own, as `welcome-ssr.e2e.ts` does, so
 * nothing is written beside another running server — and Miniflare, the
 * `workerd` host wrangler itself uses, serves that bundle and the adapter's
 * assets with the settings `wrangler.jsonc` gives them. Checked against
 * `wrangler dev` — status, content type and cache headers — on a page, the
 * locale redirect, `/_app/version.json`, an immutable chunk (`Cache-Control:
 * public, immutable, max-age=31536000`, from the adapter's `_headers`), the
 * files `.assetsignore` hides (`/_worker.js` is the worker's 404) and an
 * unknown locale: the same answers, less the proxy's `Transfer-Encoding`.
 *
 * No proxy, no inspector port, no file watcher.
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = join(dirname(fileURLToPath(import.meta.url)), '..');
const port = Number(process.argv[2] ?? 4173);

const config = JSON.parse(
	readFileSync(join(APP, 'wrangler.jsonc'), 'utf8').replace(/^\s*\/\/.*$/gm, '')
);
delete config.$schema;
const worker = join(APP, config.main);
const assets = join(APP, config.assets.directory);

// The bundle that ships, written where no other wrangler is working: one
// directory per port, made again on every start.
const project = join(APP, 'node_modules', '.cache', `vela-e2e-preview-${port}`);
const bundle = join(project, 'out');
rmSync(project, { recursive: true, force: true });
mkdirSync(bundle, { recursive: true });
writeFileSync(
	join(project, 'wrangler.json'),
	JSON.stringify({ ...config, main: worker, assets: { ...config.assets, directory: assets } })
);
execFileSync(
	join(APP, 'node_modules', '.bin', 'wrangler'),
	['deploy', '--dry-run', '--config', join(project, 'wrangler.json'), '--outdir', bundle],
	{ cwd: project, stdio: ['ignore', 'ignore', 'inherit'] }
);

// Miniflare is wrangler's dependency, not the app's: it is asked for where
// wrangler finds it, so the two cannot drift apart.
const require = createRequire(realpathSync(join(APP, 'node_modules', 'wrangler', 'package.json')));
const { Miniflare } = require('miniflare');
const server = new Miniflare({
	name: config.name,
	host: 'localhost',
	port,
	compatibilityDate: config.compatibility_date,
	compatibilityFlags: config.compatibility_flags,
	modulesRoot: bundle,
	modules: [{ type: 'ESModule', path: join(bundle, '_worker.js') }],
	assets: {
		directory: assets,
		binding: config.assets.binding,
		// What wrangler derives from a config that has a `main` and names no
		// `html_handling` / `not_found_handling` of its own.
		routerConfig: { has_user_worker: true },
		assetConfig: { html_handling: 'auto-trailing-slash', not_found_handling: 'none' }
	}
});
await server.ready;
console.log(`e2e preview: http://localhost:${port} — the deploy bundle on workerd`);

let closing = false;
async function close() {
	if (closing) return;
	closing = true;
	await server.dispose().catch(() => undefined);
	process.exit(0);
}
process.on('SIGTERM', close);
process.on('SIGINT', close);
