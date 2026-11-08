// The page pins itself (spec 076 FR-008, §38 — review 0-5), in a real browser.
//
//   CHROME_BIN=… node samples/sw-pin-test.mjs
//
// A wallet's check fetches `b/<h>/sign` and hashes it; the browser then asks
// the host again. This drives the second half against a host that turns:
//
//   · the first visit registers `sw.js`, and the version it was opened as is
//     pinned (fetched once more, kept only because it hashes to its address);
//   · the host then serves other bytes at that address — the next visit is the
//     published page all the same, answered by the worker, and the host is not
//     even asked for it;
//   · a version the worker has not met, served wrong from the start, is not
//     run at all: a refusal page that names both hashes;
//   · what the worker serves still carries `frame-ancestors 'none'`.
//
// It is driven headlessly over CDP, with no dependencies, like the other
// harnesses here.

import { createServer } from 'node:http';
import { readFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
const { build } = await import('./build-single.mjs');
const HASH = build().hash;
const PAGE = join(ROOT, 'dist', 'b', HASH, 'sign.html');
const WORKER = join(ROOT, 'dist', 'sw.js');
// A version the worker has never met: any other address-shaped hash.
const OTHER = 'ab'.repeat(32);
const PORT = 8913;
const CDP = 9227;

const results = [];
const check = (what, ok, detail) => {
	results.push(!!ok);
	console.log(`${ok ? 'PASS' : 'FAIL'}  ${what}${detail ? '  — ' + detail : ''}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

for (const file of [PAGE, WORKER]) {
	if (!existsSync(file)) {
		console.log(`FAILED: ${file} is missing — run \`bun samples/build-single.mjs\``);
		process.exit(1);
	}
}
if (!process.env.CHROME_BIN) {
	console.log('FAILED: set CHROME_BIN to a Chrome for Testing binary');
	process.exit(1);
}

const html = readFileSync(PAGE);
const worker = readFileSync(WORKER);
const tampered = Buffer.from('<!doctype html><html><body><p id="evil">swapped after the check</p></body></html>');
let honest = true;
const asked = [];
const server = createServer((request, response) => {
	const path = request.url.split('?')[0];
	asked.push(path);
	if (path === '/sw.js') {
		response.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8' });
		response.end(worker);
		return;
	}
	if (path === `/b/${HASH}/sign`) {
		response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
		response.end(honest ? html : tampered);
		return;
	}
	if (path === `/b/${OTHER}/sign`) {
		response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
		response.end(tampered);
		return;
	}
	response.writeHead(404);
	response.end();
});
await new Promise((r) => server.listen(PORT, '127.0.0.1', r));

const profile = mkdtempSync(join(tmpdir(), 'sw-pin-'));
const chrome = spawn(
	process.env.CHROME_BIN,
	[
		'--headless=new',
		`--remote-debugging-port=${CDP}`,
		`--user-data-dir=${profile}`,
		'--no-first-run',
		'--no-default-browser-check',
		'about:blank'
	],
	{ stdio: 'ignore' }
);

try {
	let target = null;
	for (let i = 0; i < 40 && !target; i++) {
		await sleep(500);
		try {
			const list = await (await fetch(`http://127.0.0.1:${CDP}/json`)).json();
			target = list.find((t) => t.type === 'page');
		} catch {
			/* not up yet */
		}
	}
	if (!target) throw new Error('the browser never came up');
	const ws = new WebSocket(target.webSocketDebuggerUrl);
	await new Promise((r, j) => {
		ws.addEventListener('open', r, { once: true });
		ws.addEventListener('error', j, { once: true });
	});
	let id = 0;
	const waiting = new Map();
	const responses = new Map();
	ws.addEventListener('message', (event) => {
		const message = JSON.parse(event.data.toString());
		if (message.id && waiting.has(message.id)) {
			waiting.get(message.id)(message);
			waiting.delete(message.id);
		}
		if (message.method === 'Network.responseReceived' && message.params.type === 'Document') {
			responses.set(new URL(message.params.response.url).pathname, message.params.response);
		}
	});
	const send = (method, params = {}) =>
		new Promise((res) => {
			const i = ++id;
			waiting.set(i, res);
			ws.send(JSON.stringify({ id: i, method, params }));
		});
	const ev = async (expression) =>
		(await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }))
			.result?.result?.value;
	const visit = async (path) => {
		await send('Page.navigate', { url: `http://127.0.0.1:${PORT}${path}?ch=url&lang=en` });
		await sleep(2500);
	};
	for (const domain of ['Runtime', 'Network', 'Page']) await send(`${domain}.enable`);

	// 1 · First visit: the page registers its worker, which pins this version.
	await visit(`/b/${HASH}/sign`);
	check('the page runs', (await ev('typeof window.VelaCS === "object"')) === true);
	const ready = await ev(
		`navigator.serviceWorker.ready.then((r) => r.active ? r.scope : 'none', (e) => 'error ' + e)`
	);
	check('it registered the deployment worker for b/', ready === `http://127.0.0.1:${PORT}/b/`, ready);
	let pinned = 'none';
	for (let i = 0; i < 20 && pinned === 'none'; i++) {
		pinned = await ev(
			`caches.open('vela-signing-pages-v1').then((c) => c.match('http://127.0.0.1:${PORT}/b/${HASH}/sign')).then((r) => r ? 'pinned' : 'none')`
		);
		if (pinned === 'none') await sleep(250);
	}
	check('the version it was opened as is pinned', pinned === 'pinned', pinned);

	// 2 · The host turns. The next visit is still the published page.
	honest = false;
	const before = asked.filter((p) => p === `/b/${HASH}/sign`).length;
	await visit(`/b/${HASH}/sign`);
	check(
		'after the host swaps the bytes, the page that opens is the published one',
		(await ev('typeof window.VelaCS === "object" && !document.getElementById("evil")')) === true
	);
	check(
		'and the host was not asked for it',
		asked.filter((p) => p === `/b/${HASH}/sign`).length === before,
		`${asked.filter((p) => p === `/b/${HASH}/sign`).length - before} more request(s)`
	);
	const served = responses.get(`/b/${HASH}/sign`);
	check('it came from the worker', served?.fromServiceWorker === true, String(served?.fromServiceWorker));
	const frames = served?.headers?.['content-security-policy'] || served?.headers?.['Content-Security-Policy'] || '';
	check("what the worker serves cannot be framed", /frame-ancestors 'none'/.test(frames), frames);
	const csp = await ev(
		`document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content || ''`
	);
	check("the page's own CSP still begins default-src 'none'", csp.startsWith("default-src 'none'"), csp.slice(0, 40));

	// 3 · A version the worker never met, wrong from the start: nothing runs.
	await visit(`/b/${OTHER}/sign`);
	const refused = await ev('document.body ? document.body.textContent : ""');
	check(
		'a version whose bytes are not its address is refused, not run',
		/was not opened/.test(refused) && refused.includes(OTHER) && !/swapped after the check/.test(refused),
		refused.slice(0, 80)
	);
	check('the refusal is a 409', responses.get(`/b/${OTHER}/sign`)?.status === 409);
} catch (error) {
	console.log('FAILED: ' + error.message);
	process.exitCode = 1;
} finally {
	chrome.kill();
	server.close();
	try {
		rmSync(profile, { recursive: true, force: true });
	} catch {
		/* ignore */
	}
	const passed = results.filter(Boolean).length;
	console.log(`\n${passed}/${results.length} checks passed`);
	if (passed !== results.length) process.exitCode = 1;
}
