// Store screenshots 2 and 3 (spec 094 B4): the side panel's connect card and
// its signing sheet for a Permit2 permit, next to a dApp. The dApp is a
// neutral demo page (demo-dapp.html) served as https://swap.example — a
// reserved documentation domain — from this machine, with a throwaway
// self-signed certificate. The side panel is not a Playwright page: it is
// captured through Chrome's own CDP endpoint, the page by Playwright, and
// compose.py puts the two side by side at 1280 x 800. The DEVELOPMENT package
// (the parallel space signs in), as wallet.mjs.
// Usage: node scripts/store-art/panel.mjs
import { chromium } from '@playwright/test';
import { createHash } from 'node:crypto';
import { createServer } from 'node:https';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
const HERE = import.meta.dirname;
const APP = join(HERE, '../..');
const DIST = join(APP, 'extension/dist');
const OUT = join(APP, '../../docs/store-submission/chrome-web-store');
const TMP = mkdtempSync(join(tmpdir(), 'vela-store-art-'));
execFileSync(
	'openssl',
	[
		'req',
		'-x509',
		'-newkey',
		'rsa:2048',
		'-nodes',
		'-keyout',
		join(TMP, 'key.pem'),
		'-out',
		join(TMP, 'cert.pem'),
		'-subj',
		'/CN=swap.example',
		'-days',
		'1',
		'-addext',
		'subjectAltName=DNS:swap.example'
	],
	{ stdio: 'ignore' }
);
const { key } = JSON.parse(readFileSync(join(APP, 'extension/manifest.json'), 'utf8'));
const digest = createHash('sha256').update(Buffer.from(key, 'base64')).digest('hex');
const id = [...digest.slice(0, 32)].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');
const html = readFileSync(join(HERE, 'demo-dapp.html'));
const server = createServer(
	{ key: readFileSync(join(TMP, 'key.pem')), cert: readFileSync(join(TMP, 'cert.pem')) },
	(_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	}
);
await new Promise((r) => server.listen(8851, '127.0.0.1', r));
const CDP = 9335;
const context = await chromium.launchPersistentContext('', {
	headless: false,
	args: [
		'--headless=new',
		`--disable-extensions-except=${DIST}`,
		`--load-extension=${DIST}`,
		'--host-resolver-rules=MAP swap.example:443 127.0.0.1:8851',
		...(process.env.https_proxy ? [`--proxy-server=${process.env.https_proxy}`] : []),
		'--proxy-bypass-list=swap.example',
		'--ignore-certificate-errors',
		`--remote-debugging-port=${CDP}`,
		`--window-size=${process.env.WIN ?? '1305,895'}`
	],
	viewport: null,
	colorScheme: 'dark'
});
await new Promise((r) => setTimeout(r, 1500));
for (const p of context.pages()) if (p.url().startsWith('chrome-extension://')) await p.close();
const wallet = await context.newPage();
await wallet.addInitScript(() => {
	localStorage.setItem('vela.intro.seen', String(Date.now()));
	localStorage.setItem('vela.dev.console', '1');
});
await wallet.goto(`chrome-extension://${id}/en/parallel.html`);
await wallet.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
await wallet.waitForURL(/wallet\.html$/, { timeout: 30_000 });
await wallet.evaluate(() => {
	localStorage.setItem('vela.activeAccountIndex', '3');
	const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]');
	for (const a of accounts) if (a.name === 'Parallel Multi') a.name = 'Everyday';
	localStorage.setItem('vela.accounts', JSON.stringify(accounts));
});
await wallet.goto(`chrome-extension://${id}/en/wallet.html`);
await wallet.waitForTimeout(6000);

const page = await context.newPage();
await page.goto('https://swap.example/');
await page.bringToFront();

async function panelTarget() {
	for (let i = 0; i < 60; i += 1) {
		const list = await (await fetch(`http://127.0.0.1:${CDP}/json/list`)).json();
		// The panel drops `?panel` from its address (the mark lives in its
		// session storage); it is the wallet document Playwright has no page for.
		const candidates = list.filter((t) => t.url.includes('/wallet.html'));
		const panel =
			candidates.find((t) => t.type !== 'page') ?? (candidates.length > 1 ? candidates : null);
		if (panel && !Array.isArray(panel)) return panel;
		if (Array.isArray(panel)) {
			for (const t of panel) {
				const size = await cdp(t, 'Runtime.evaluate', {
					expression: 'innerWidth',
					returnByValue: true
				});
				if (size?.result?.value && size.result.value < 600) return t;
			}
		}
		await new Promise((r) => setTimeout(r, 500));
	}
	throw new Error('no side panel target');
}
async function cdp(target, method, params = {}) {
	const ws = new WebSocket(target.webSocketDebuggerUrl);
	await new Promise((r, j) => {
		ws.onopen = r;
		ws.onerror = j;
	});
	const reply = await new Promise((resolve) => {
		ws.onmessage = (m) => {
			const data = JSON.parse(m.data);
			if (data.id === 1) resolve(data);
		};
		ws.send(JSON.stringify({ id: 1, method, params }));
	});
	ws.close();
	return reply.result;
}
async function shoot(name) {
	const target = await panelTarget();
	const shot = await cdp(target, 'Page.captureScreenshot', { format: 'png' });
	writeFileSync(join(TMP, `${name}-panel.png`), Buffer.from(shot.data, 'base64'));
	await page.screenshot({ path: join(TMP, `${name}-page.png`) });
	execFileSync(
		'python3',
		[
			join(HERE, 'compose.py'),
			join(TMP, `${name}-page.png`),
			join(TMP, `${name}-panel.png`),
			join(OUT, name)
		],
		{ stdio: 'inherit' }
	);
}
const inPanel = (body) =>
	wallet.evaluate((source) => {
		const panel = chrome.extension.getViews().find((w) => {
			try {
				return (
					w.location.pathname.endsWith('/wallet.html') &&
					(new URLSearchParams(w.location.search).has('panel') ||
						w.sessionStorage.getItem('vela.surface.panel') === '1')
				);
			} catch {
				return false;
			}
		});
		if (!panel) return null;
		return new Function('panel', source)(panel);
	}, body);

// 2: the connect card.
await page.getByRole('button', { name: 'Connect wallet' }).click();
for (let i = 0; i < 60; i += 1) {
	if (await inPanel(`return !!panel.document.querySelector('[role="dialog"]');`)) break;
	await new Promise((r) => setTimeout(r, 500));
}
await new Promise((r) => setTimeout(r, Number(process.env.CARD_WAIT ?? 12000)));
await inPanel(
	`const s = panel.document.createElement('style'); s.textContent = '[data-testid="parallel-space-badge"]{display:none!important}'; panel.document.head.append(s); return true;`
);
await shoot('screenshot-2-connect.png');
await inPanel(
	`const b=[...panel.document.querySelectorAll('[role="dialog"] button')].find(x=>x.textContent.trim()==='Connect'); b.click(); return true;`
);
await page.waitForFunction(
	() => document.getElementById('connect').classList.contains('done'),
	null,
	{ timeout: 30_000 }
);

// 3: the signing sheet for a Permit2 permit.
await page.getByRole('button', { name: 'Swap', exact: true }).click();
for (let i = 0; i < 60; i += 1) {
	if (await inPanel(`return !!panel.document.querySelector('[role="dialog"]');`)) break;
	await new Promise((r) => setTimeout(r, 500));
}
await new Promise((r) => setTimeout(r, Number(process.env.SHEET_WAIT ?? 8000)));
await shoot('screenshot-3-signing.png');
console.log('done');
await context.close();
server.close();
