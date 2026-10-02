// Store screenshot 1: the wallet home in a tab (spec 094 B4).
//
// The DEVELOPMENT package (`pnpm build:extension`): the parallel space is how a
// script signs in without a person's passkey. The fixture multi-key Safe, its
// real balances on the real networks (read-only), renamed "Everyday"; the
// parallel-space badge hidden — a developer marker the store package does not
// carry. Usage: node scripts/store-art/wallet.mjs [light|dark]
import { chromium } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const APP = join(import.meta.dirname, '../..');
const OUT = join(APP, '../../docs/store-submission/chrome-web-store');
const DIST = join(APP, 'extension/dist');
const { key } = JSON.parse(readFileSync(join(APP, 'extension/manifest.json'), 'utf8'));
const digest = createHash('sha256').update(Buffer.from(key, 'base64')).digest('hex');
const id = [...digest.slice(0, 32)].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');
const theme = process.argv[2] ?? 'light';
const context = await chromium.launchPersistentContext('', {
	headless: false,
	args: ['--headless=new', `--disable-extensions-except=${DIST}`, `--load-extension=${DIST}`],
	viewport: { width: 1280, height: 800 },
	colorScheme: theme
});
await new Promise((r) => setTimeout(r, 1500));
for (const p of context.pages()) if (p.url().startsWith('chrome-extension://')) await p.close();
const page = await context.newPage();
await page.addInitScript(() => {
	localStorage.setItem('vela.intro.seen', String(Date.now()));
	localStorage.setItem('vela.dev.console', '1');
});
await page.goto(`chrome-extension://${id}/en/parallel.html`);
await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
await page.waitForURL(/wallet\.html$/, { timeout: 30_000 });
await page.evaluate(() => {
	localStorage.setItem('vela.activeAccountIndex', '3');
	const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]');
	for (const a of accounts) if (a.name === 'Parallel Multi') a.name = 'Everyday';
	localStorage.setItem('vela.accounts', JSON.stringify(accounts));
});
await page.goto(`chrome-extension://${id}/en/wallet.html`);
await page.waitForTimeout(Number(process.env.WAIT ?? 20_000));
await page.addStyleTag({ content: '[data-testid="parallel-space-badge"]{display:none!important}' });
await page.screenshot({ path: join(OUT, 'screenshot-1-wallet.png') });
console.log('ok', id);
await context.close();
