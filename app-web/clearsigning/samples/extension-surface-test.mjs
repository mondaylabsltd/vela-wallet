// Does the sheet behave the same INSIDE the extension?
//
// MV3 pages have their own CSP, and remote images are exactly the kind of thing
// it can quietly refuse — which would leave the extension showing drawn letters
// where the hosted page shows logos, and nobody would notice.
//
//   CHROME_BIN=… node samples/extension-surface-test.mjs
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const EXT = '/Volumes/data/production/vela-wallet-native/app-web/clearsigning';
const CDP = 9399;
const profile = mkdtempSync(join(tmpdir(), 'surface-'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hash = createHash('sha256').update(EXT).digest('hex').slice(0, 32);
const id = [...hash].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join('');

const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`, '--enable-unsafe-extension-debugging',
  `--user-data-dir=${profile}`, `--disable-extensions-except=${EXT}`, `--load-extension=${EXT}`,
  '--no-proxy-server', '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });

try {
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  await sleep(1500);
  const url = `chrome-extension://${id}/gallery.html?lang=en`;
  const tab = await (await fetch(`http://127.0.0.1:${CDP}/json/new?${encodeURIComponent(url)}`, { method: 'PUT' })).json();
  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener('open', r));
  let n = 0;
  const pending = new Map();
  const errors = [];
  ws.addEventListener('message', (e) => {
    const m = JSON.parse(e.data);
    if (pending.has(m.id)) { pending.get(m.id)(m.result); pending.delete(m.id); }
    if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') errors.push(m.params.entry.text);
  });
  const send = (method, params = {}) => new Promise((res) => {
    const i = ++n; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params }));
  });
  await send('Runtime.enable');
  await send('Log.enable');
  await sleep(3500);
  const ev = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true })).result.value;

  console.log('surface        :', await ev('location.origin'));
  console.log('cases rendered :', await ev("document.querySelectorAll('.sheet').length"));
  console.log('render failures:', await ev("[...document.querySelectorAll('.case')].filter(c => !c.querySelector('.sheet')).length"));
  console.log('identicons     :', await ev("document.querySelectorAll('.identicon svg').length"));
  console.log('remote logos   :', await ev("document.querySelectorAll('.token-chip.has-logo, .chain-dot.has-logo').length"));
  console.log('selector check :', await ev("document.getElementById('selfcheck').firstChild.textContent"));
  console.log('console errors :', errors.length ? errors.slice(0, 3) : 'none');
} finally {
  chrome.kill();
  try { rmSync(profile, { recursive: true, force: true }); } catch {}
}
