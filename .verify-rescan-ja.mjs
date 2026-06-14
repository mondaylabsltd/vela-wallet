import { chromium } from 'playwright';
const URL = 'http://localhost:8081/';
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 430, height: 932 } });
const errors = [];
page.on('pageerror', (e) => errors.push('PAGEERROR: ' + e.message));
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
async function waitText(s, ms = 90000) {
  try { await page.waitForFunction((x) => (document.body?.innerText || '').includes(x), s, { timeout: ms }); }
  catch (e) { await page.screenshot({ path: '/tmp/05-ja-FAIL.png', fullPage: true });
    console.log('TIMEOUT on:', s, '\n', (await page.evaluate(() => (document.body?.innerText||'').slice(0,400)))); throw e; }
}

await page.goto(URL, { waitUntil: 'load', timeout: 120000 });
await waitText('Create Wallet', 120000);                 // en welcome → bundle ready
await page.evaluate(() => {
  const acct = { id: 'verify-cred-0001', name: 'Verify', address: '0x1111111111111111111111111111111111111111', createdAt: '2026-06-14T00:00:00.000Z', publicKeyHex: '04' + '11'.repeat(64) };
  localStorage.setItem('vela.accounts', JSON.stringify([acct]));
  localStorage.setItem('vela.activeAccountIndex', '0');
  localStorage.setItem('vela.language', 'ja');           // drive i18n via app's own pref
});
await page.reload({ waitUntil: 'load', timeout: 120000 });
await waitText('合計残高', 120000);                        // ja Home (Total balance) → home + ja confirmed
await page.evaluate(() => window.vela?.failRpc('all'));

await page.getByText('再スキャン', { exact: true }).first().click();   // Re-scan (ja)
await waitText('最近のアクティビティを再スキャン', 20000);                 // sheet title (ja)
await page.getByText('過去10分', { exact: true }).click();             // Last 10 min (ja)
await waitText('検出できないことがあります', 90000);                       // native note (ja)

const body = await page.evaluate(() => document.body?.innerText || '');
const checks = {
  homeJa: body.length > 0,
  noteJa: body.includes('検出できないことがあります') && body.includes('ブロックエクスプローラーでご確認'),
  explorerTitleJa: body.includes('ブロックエクスプローラーで直接確認'),
  noneJa: body.includes('新しい入金は見つかりませんでした'),
  failedJa: body.includes('接続できませんでした'),
  noEnglishLeak: !body.includes('Check directly on a block explorer') && !body.includes('No new payments') && !body.includes("Couldn't reach"),
};
await page.screenshot({ path: '/tmp/05-rescan-ja.png', fullPage: true });
console.log('JA CHECKS:', JSON.stringify(checks, null, 2));
console.log('ERRORS:', errors.slice(0, 8).join('\n') || '(none)');
await browser.close();
process.exit(Object.values(checks).every(Boolean) ? 0 : 1);
