// Serve `dist/` the way the signer host does, on this machine — for trying a
// build before the owner deploys it, and for the end-to-end checks.
//
//   bun samples/serve-dist.mjs                       http://localhost:8099
//   bun samples/serve-dist.mjs --port 8443 --tls <cert.pem> <key.pem>
//
// What makes it the host and not just a file server: the header rules
// `build-single.mjs` writes to `dist/_headers` (frame-ancestors, no-transform,
// the year-long immutable cache under `/b/`, and the CORS line the web wallet's
// check needs for `/b/*` and `/index.json`), applied as Cloudflare Pages
// applies them — every matching block, in order; and the pretty URL the
// official host answers, `/b/<sha256>/sign` for `sign.html`.
//
// Pointing a wallet at it: a page on this machine's loopback has its own
// relying party (`localhost`), so it reaches a wallet's keys only when the
// wallet runs on the loopback too, and only an account whose signing domain
// is that page's. To try it with `getvela.app` keys, serve it over TLS and map
// `sign.getvela.app` to it in the browser (`--host-resolver-rules`), as the
// page's own browser suites do (HANDOVER.md).
//
// Zero dependencies; Node or Bun. `handler` is exported for a harness that
// serves the page beside something else.
import { createServer as createHttp } from 'node:http';
import { createServer as createHttps } from 'node:https';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
export const DIST = resolve(here, '..', 'dist');

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
};

/** `dist/_headers` as `[pattern, [[name, value]...]]`, in file order. */
function readRules(dist) {
  const path = join(dist, '_headers');
  if (!existsSync(path)) return [];
  const rules = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    if (!/^\s/.test(line)) {
      rules.push([line.trim(), []]);
    } else if (rules.length) {
      const at = line.indexOf(':');
      rules[rules.length - 1][1].push([line.slice(0, at).trim(), line.slice(at + 1).trim()]);
    }
  }
  return rules;
}

function matches(pattern, path) {
  if (pattern === path) return true;
  if (pattern.endsWith('*')) return path.startsWith(pattern.slice(0, -1));
  return false;
}

/** Every matching block's headers; same-name values joined, as Pages joins them. */
export function headersFor(path, dist = DIST) {
  const out = {};
  for (const [pattern, headers] of readRules(dist)) {
    if (!matches(pattern, path)) continue;
    for (const [name, value] of headers) {
      const key = name.toLowerCase();
      out[key] = out[key] ? `${out[key]}, ${value}` : value;
    }
  }
  return out;
}

/** A request handler serving `dist`. */
export function handler(dist = DIST) {
  return (req, res) => {
    const url = new URL(req.url, 'http://host');
    let path = decodeURIComponent(url.pathname);
    if (path.endsWith('/')) path += 'sign.html';
    // The official host's pretty URL: `/b/<sha256>/sign` is `sign.html`.
    const file = /\/sign$/.test(path) ? `${path}.html` : path;
    const full = normalize(join(dist, file));
    if (!full.startsWith(dist) || !existsSync(full) || !statSync(full).isFile() || file.endsWith('_headers')) {
      res.writeHead(404, { 'content-type': 'text/plain' }).end('not found');
      return;
    }
    res.writeHead(200, {
      'content-type': TYPES[extname(full)] || 'application/octet-stream',
      ...headersFor(url.pathname, dist),
    });
    res.end(req.method === 'HEAD' ? undefined : readFileSync(full));
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const at = args.indexOf('--port');
  const port = at >= 0 ? Number(args[at + 1]) : 8099;
  const tls = args.indexOf('--tls');
  const server = tls >= 0
    ? createHttps({ cert: readFileSync(args[tls + 1]), key: readFileSync(args[tls + 2]) }, handler())
    : createHttp(handler());
  server.listen(port, '127.0.0.1', () => {
    const scheme = tls >= 0 ? 'https' : 'http';
    const index = JSON.parse(readFileSync(join(DIST, 'index.json'), 'utf8'));
    console.log(`serve-dist: ${scheme}://localhost:${port}/ — ${index.versions.length} version(s) under /b/`);
  });
}
