// world:"MAIN" injection guard. inpage.js must install the provider ONLY when it
// runs in the page's MAIN world (window.ethereum visible to the dApp). If an older
// Safari honored the new inpage content_scripts entry but ignored world:"MAIN", it
// would run inpage as an ISOLATED content script — where extension APIs exist and
// window.ethereum is invisible. inpage must then bail WITHOUT setting the shared-DOM
// marker, so content.js still fires its MAIN-world <script> fallback.
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ASSETS = path.resolve(HERE, '..', '..', '..', 'targets', 'safari', 'assets');
const INPAGE = readFileSync(path.join(ASSETS, 'inpage.js'), 'utf8');

test('MAIN world (no extension APIs): provider installs + sets marker', async ({ page }) => {
  await page.goto('about:blank');
  await page.addScriptTag({ content: INPAGE });
  expect(await page.evaluate(() => !!(window as any).ethereum?.isVela)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.hasAttribute('data-vela-inpage'))).toBe(true);
});

test('ISOLATED world (browser.runtime.id present): provider BAILS, no marker', async ({ page }) => {
  await page.goto('about:blank');
  // Simulate the isolated content-script world where extension APIs are present.
  await page.addInitScript({ content: `window.browser = { runtime: { id: 'vela-ext-id' } };` });
  await page.reload();
  await page.addScriptTag({ content: INPAGE });
  expect(await page.evaluate(() => !!(window as any).ethereum)).toBe(false);
  expect(await page.evaluate(() => document.documentElement.hasAttribute('data-vela-inpage'))).toBe(false);
});
