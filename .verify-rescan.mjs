import { chromium } from 'playwright';

const URL = 'http://localhost:8081/';
const NOTE_SUBSTR = 'open a block explorer for the full record';
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 430, height: 932 } });

const errors = [];
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', (e) => errors.push('PAGEERROR: ' + e.message));

const log = (...a) => console.log(...a);
async function waitText(sub, ms = 30000) {
  await page.waitForFunction(
    (s) => (document.body?.innerText || '').includes(s), sub, { timeout: ms },
  );
}

// 1) Boot once so localStorage exists on this origin.
await page.goto(URL, { waitUntil: 'load', timeout: 90000 });
await waitText('Create Wallet', 60000);

// 2) Seed a wallet account so the router lands on Home (skips passkey onboarding).
await page.evaluate(() => {
  const acct = {
    id: 'verify-cred-0001',
    name: 'Verify',
    address: '0x1111111111111111111111111111111111111111',
    createdAt: '2026-06-14T00:00:00.000Z',
    publicKeyHex: '04' + '11'.repeat(64),
  };
  localStorage.setItem('vela.accounts', JSON.stringify([acct]));
  localStorage.setItem('vela.activeAccountIndex', '0');
});

// 3) Reload → Home.
await page.reload({ waitUntil: 'load', timeout: 90000 });
await waitText('Total balance', 60000);
log('✅ reached Home (seeded account)');
await page.screenshot({ path: '/tmp/02-home.png', fullPage: true });

// 4) Force every RPC to fail fast so the scan finishes immediately (dev fault harness).
const faultOk = await page.evaluate(() => {
  if (window.vela && typeof window.vela.failRpc === 'function') { window.vela.failRpc('all'); return true; }
  return false;
});
log(faultOk ? '✅ vela.failRpc("all") set' : '⚠️ vela console not present — scan will hit real RPC');

// 5) Open the Re-scan sheet.
await page.getByText('Re-scan', { exact: true }).first().click();
await waitText('Re-scan recent activity', 15000);
log('✅ Re-scan sheet open');
await page.screenshot({ path: '/tmp/03-rescan-chooser.png', fullPage: true });

// 6) Pick a window → scanning → done.
await page.getByText('Last 10 minutes', { exact: true }).click();
log('… scanning');

// 7) Wait for the done-state note.
let pass = false;
try {
  await waitText(NOTE_SUBSTR, 90000);
  pass = true;
} catch { /* fall through to capture */ }

await page.screenshot({ path: '/tmp/04-rescan-done.png', fullPage: true });
const body = await page.evaluate(() => document.body?.innerText || '');
log('=== DONE-STATE VISIBLE TEXT (excerpt) ===');
log(body.split('\n').filter((l) => l.trim()).slice(-12).join('\n'));
log('=== NOTE PRESENT? ===', pass);
log('=== CONSOLE ERRORS ===', errors.slice(0, 10).join('\n') || '(none)');

await browser.close();
process.exit(pass ? 0 : 1);
