// Store art (spec 094 B4): the 128 x 128 store icon (96 x 96 art, 16 px of
// transparent padding) and the 440 x 280 small promo tile, rendered by
// Chromium from the canonical mark (docs/design/icon) and the app's own font.
import { chromium } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const HERE = import.meta.dirname;
const REPO = join(HERE, '../../../..');
const OUT = join(REPO, 'docs/store-submission/chrome-web-store');
const icon = readFileSync(join(REPO, 'docs/design/icon/app-icon.svg'), 'utf8').replace(
	/^<\?xml[^>]*>\s*/,
	''
);
const mark = readFileSync(join(REPO, 'docs/design/icon/app-mark.svg'), 'utf8').replace(
	/^<\?xml[^>]*>\s*/,
	''
);
const fonts = join(HERE, '../../node_modules/@fontsource/plus-jakarta-sans/files');
const face = (w) =>
	`@font-face{font-family:Jakarta;font-weight:${w};src:url(data:font/woff2;base64,${readFileSync(join(fonts, `plus-jakarta-sans-latin-${w}-normal.woff2`)).toString('base64')}) format('woff2')}`;
const browser = await chromium.launch();
const page = await browser.newPage({ deviceScaleFactor: 1 });

await page.setViewportSize({ width: 128, height: 128 });
await page.setContent(`<html><body style="margin:0;background:transparent">
	<div style="position:absolute;left:16px;top:16px;width:96px;height:96px">${icon.replace('width="68" height="68"', 'width="96" height="96"')}</div>
</body></html>`);
await page.screenshot({ path: join(OUT, 'store-icon-128.png'), omitBackground: true });

await page.setViewportSize({ width: 440, height: 280 });
await page.setContent(`<html><head><style>${face(500)}${face(700)}
	body{margin:0;width:440px;height:280px;background:#f46d50;font-family:Jakarta,sans-serif;display:flex;align-items:center;gap:24px;padding:0 36px;box-sizing:border-box;color:#fff}
	.mark{flex:0 0 104px;height:104px}
	h1{margin:0 0 8px;font-size:34px;font-weight:700;letter-spacing:-0.02em;line-height:1.05}
	p{margin:0;font-size:17px;font-weight:500;line-height:1.35;color:#fff3ec}
	p.small{margin-top:12px;font-size:12.5px;color:#ffe2d6;white-space:nowrap}
	</style></head><body>
	<div class="mark">${mark.replace('width="68" height="68"', 'width="104" height="104"')}</div>
	<div><h1>Vela Wallet</h1><p>The passkey wallet,<br>in your browser.</p><p class="small">24 networks · no seed phrase</p></div>
</body></html>`);
await page.evaluate(() => document.fonts.ready);
await page.waitForTimeout(300);
await page.screenshot({ path: join(OUT, 'promo-small-440x280.png') });
await browser.close();
console.log('ok');
