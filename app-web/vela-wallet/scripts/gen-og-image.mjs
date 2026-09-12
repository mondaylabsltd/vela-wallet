// Render static/og-image.svg to static/og-image.png (1200×630) with the
// Playwright Chromium the e2e suite already installs. Run once when the
// mark or the ground changes; the PNG is committed (spec 038 #meta).
//
//   node scripts/gen-og-image.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright';

const root = join(import.meta.dirname, '..');
const svg = readFileSync(join(root, 'static', 'og-image.svg'), 'utf8');
const browser = await chromium.launch();
const page = await browser.newPage({
	viewport: { width: 1200, height: 630 },
	deviceScaleFactor: 1
});
await page.setContent(`<!doctype html><html><body style="margin:0">${svg}</body></html>`);
const png = await page.locator('svg').screenshot({ type: 'png' });
writeFileSync(join(root, 'static', 'og-image.png'), png);
await browser.close();
console.log(`og-image.png: ${png.length} bytes`);
