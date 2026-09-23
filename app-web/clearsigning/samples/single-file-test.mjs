// The single-file signing page, under its own CSP, in a real browser.
//
//   CHROME_BIN=… node samples/single-file-test.mjs
//
// `build-single.mjs` can only check its own arithmetic. This checks the thing
// that actually matters (spec 076, FR-003 and SC-004):
//
//   · the page's scripts RUN — a CSP hash that is one byte off produces a
//     signing page that loads and then does nothing, which is worse than one
//     that fails loudly;
//   · the page asks the network for NOTHING but its own document;
//   · a script inside it cannot reach the network at all.
//
// The third is the property the whole single-file constraint exists for. The
// page renders attacker-supplied data — a dApp's transaction, a token's name,
// a contract's own strings — so the one thing it must not be able to do is
// send what it sees anywhere.
//
// It is driven headlessly over CDP, with no dependencies, like the other
// harnesses here.

import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
// The page this build produces, found where it is published. `dist/` holds
// every version ever published, so the test must name the one under test
// rather than assume there is only one.
const { build } = await import('./build-single.mjs');
const PAGE = join(ROOT, 'dist', 'b', build().hash, 'sign.html');
const PORT = 8911;
const CDP = 9225;

const results = [];
const check = (what, ok, detail) => {
	results.push(!!ok);
	console.log(`${ok ? 'PASS' : 'FAIL'}  ${what}${detail ? '  — ' + detail : ''}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

if (!existsSync(PAGE)) {
	console.log(`FAILED: ${PAGE} is missing — run \`bun samples/build-single.mjs\``);
	process.exit(1);
}
if (!process.env.CHROME_BIN) {
	console.log('FAILED: set CHROME_BIN to a Chrome for Testing binary');
	process.exit(1);
}

// Serves the page, and a `probe` the page will try (and fail) to reach.
const html = readFileSync(PAGE);
const asked = [];
const server = createServer((request, response) => {
	asked.push(request.url);
	if (request.url === '/sign.html') {
		response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
		response.end(html);
		return;
	}
	response.writeHead(200, { 'content-type': 'text/plain' });
	response.end('probe');
});
await new Promise((r) => server.listen(PORT, '127.0.0.1', r));

// A fresh profile every run: `Log.enable` replays a target's earlier entries,
// so a reused browser reports violations from a previous page as if they were
// this one's. (Measured — it is how a passing build first looked like a
// failing one.)
const profile = mkdtempSync(join(tmpdir(), 'single-'));
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
	const violations = [];
	const requested = [];
	ws.addEventListener('message', (event) => {
		const message = JSON.parse(event.data.toString());
		if (message.id && waiting.has(message.id)) {
			waiting.get(message.id)(message);
			waiting.delete(message.id);
		}
		if (message.method === 'Log.entryAdded') {
			const text = message.params.entry.text || '';
			if (/Content Security Policy|Refused to/i.test(text)) violations.push(text.slice(0, 120));
		}
		if (message.method === 'Network.requestWillBeSent') requested.push(message.params.request.url);
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

	for (const domain of ['Log', 'Runtime', 'Network', 'Page']) await send(`${domain}.enable`);
	await send('Page.navigate', { url: `http://127.0.0.1:${PORT}/sign.html` });
	await sleep(4000);

	check('the page loads', (await ev('document.title')).length > 0, await ev('document.title'));

	const modules = await ev(
		'typeof window.VelaCS === "object" ? Object.keys(window.VelaCS).length : 0'
	);
	check('its scripts RUN under its own CSP hash', modules > 10, `${modules} modules on VelaCS`);

	const csp = await ev(
		`document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content || ''`
	);
	check("the CSP is default-src 'none'", csp.startsWith("default-src 'none'"), csp.slice(0, 40));

	check('no CSP violation on load', violations.length === 0, violations[0] || '');

	const ownDocumentOnly = requested.every((url) => url.endsWith('/sign.html'));
	check('the page asks the network for nothing but itself', ownDocumentOnly, requested.join(' '));

	// The property the constraint exists for: a script in this page cannot send
	// what the page sees anywhere. Every way out is tried, and then the SERVER
	// is asked what arrived — that is the ground truth.
	//
	// An API's own return value is not. `navigator.sendBeacon` returns `true`
	// for "queued", not for "sent": the CSP refusal happens afterwards, so it
	// reports success on a request that never leaves the browser. Believing it
	// would have reported a leak that is not there. (Measured.)
	const tried = {
		fetch: await ev(
			`fetch('http://127.0.0.1:${PORT}/probe?f').then(()=>'REACHED').catch(()=>'refused')`
		),
		image: await ev(
			`new Promise(r=>{const i=new Image();i.onload=()=>r('REACHED');i.onerror=()=>r('refused');i.src='http://127.0.0.1:${PORT}/probe.png?i';})`
		),
		beacon: await ev(
			`(() => { try { return navigator.sendBeacon('http://127.0.0.1:${PORT}/probe?b', 'x') ? 'queued' : 'refused'; } catch { return 'refused'; } })()`
		),
		websocket: await ev(
			`new Promise(r=>{try{const s=new WebSocket('ws://127.0.0.1:${PORT}/probe?w');s.onopen=()=>r('REACHED');s.onerror=()=>r('refused');setTimeout(()=>r('refused'),1500);}catch{r('refused');}})`
		)
	};
	await sleep(1500);

	for (const [how, said] of Object.entries(tried)) {
		check(`${how}: the page's own API says it did not go`, said !== 'REACHED', said);
	}
	// The one that settles it.
	const escaped = asked.filter((url) => url !== '/sign.html');
	check(
		'NOTHING but the document ever reached the server',
		escaped.length === 0,
		escaped.length ? escaped.join(' ') : 'only /sign.html'
	);
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
