import { chromium } from 'playwright';

const URL = 'http://localhost:8081/';
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 430, height: 932 } });

const errors = [];
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', (e) => errors.push('PAGEERROR: ' + e.message));

await page.goto(URL, { waitUntil: 'load', timeout: 90000 });

// RN-web first bundle can take a while; poll until the app paints real text.
let text = '';
for (let i = 0; i < 45; i++) {
  text = await page.evaluate(() => document.body?.innerText || '');
  if (text.trim().length > 15) break;
  await page.waitForTimeout(2000);
}

await page.screenshot({ path: '/tmp/01-landing.png', fullPage: true });
console.log('=== VISIBLE TEXT ===');
console.log(text.slice(0, 2500));
console.log('=== CONSOLE ERRORS (first 15) ===');
console.log(errors.slice(0, 15).join('\n') || '(none)');
await browser.close();
