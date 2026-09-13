// Reference peripheral — and the test that proves the browser side works.
//
// It plays the role the native apps will play (desktop Rust, iOS CoreBluetooth,
// Android BluetoothGattServer): advertise the Vela service, answer the ECDH
// handshake, push a signing intent, read back the signature. Everything here is
// PROTOCOL.md sections 1–4 and nothing else, so the three native ports have a
// working thing to compare against.
//
// The radio is Chrome's own BluetoothEmulation, driven over CDP.
//
// STATUS: gets as far as the device chooser (DeviceAccess.selectPrompt works),
// then stalls — Chrome 151 emits no `gattOperationReceived` for a simulated
// preconnected peripheral, so the GATT connect never completes. Kept for when
// that experimental domain matures. **The transport is verified by
// samples/ble-loopback.mjs instead**, which drives the same shipped code
// through a stubbed GATT layer.
//
//   CHROME_BIN=… SB=… node samples/ble-peripheral.mjs
import { spawn } from 'node:child_process';
import { webcrypto as crypto } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const SB = process.env.SB;
const PORT = 8443;
const CDP = 9385;
const ADDRESS = '11:22:33:44:55:66';

const SERVICE = '76656c61-0001-4000-8000-00805f9b34fb';
const C2P = '76656c61-0002-4000-8000-00805f9b34fb';
const P2C = '76656c61-0003-4000-8000-00805f9b34fb';
const HEADER = 6;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const utf8 = (s) => new TextEncoder().encode(s);
const fromUtf8 = (b) => new TextDecoder().decode(b);
const b64 = (b) => Buffer.from(b).toString('base64');
const unb64 = (s) => new Uint8Array(Buffer.from(s, 'base64'));
const b64url = (b) => Buffer.from(b).toString('base64url');
const unb64url = (s) => new Uint8Array(Buffer.from(s, 'base64url'));

function concat(parts) {
  return new Uint8Array(Buffer.concat(parts.map((p) => Buffer.from(p))));
}

async function hkdf(secret, salt, info, length) {
  const key = await crypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt, info: utf8(info) }, key, length * 8,
  );
  return new Uint8Array(bits);
}

// --- CDP plumbing ------------------------------------------------------------

class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.listeners = [];
    ws.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        message.error ? reject(new Error(message.error.message)) : resolve(message.result);
      } else if (message.method) {
        this.listeners.forEach((l) => l(message));
      }
    });
  }

  static async open(url) {
    const ws = new WebSocket(url);
    await new Promise((resolve) => ws.addEventListener('open', resolve));
    return new Cdp(ws);
  }

  send(method, params = {}, sessionId) {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }

  on(handler) {
    this.listeners.push(handler);
  }
}

// --- the peripheral ----------------------------------------------------------

class Peripheral {
  constructor(cdp, ids) {
    this.cdp = cdp;
    this.ids = ids;            // { c2p, p2c }
    this.pending = new Map();  // msgId → frames being reassembled
    this.msgId = 0;
    this.counters = { c2p: 0n, p2c: 0n };
    this.outgoing = 0;
    this.session = null;
    this.chunk = 180;          // what a conservative peripheral would negotiate
    this.queue = [];
    this.waiters = [];
    this.notifySubscribed = false;
  }

  deliver(message) {
    if (this.waiters.length) this.waiters.shift()(message);
    else this.queue.push(message);
  }

  next(timeoutMs = 15000) {
    if (this.queue.length) return Promise.resolve(this.queue.shift());
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('peripheral timed out waiting')), timeoutMs);
      this.waiters.push((message) => { clearTimeout(timer); resolve(message); });
    });
  }

  // Frames arriving from the browser on c2p.
  async accept(bytes) {
    const flags = bytes[0];
    const msgId = bytes[1];
    const seq = (bytes[2] << 8) | bytes[3];
    const total = (bytes[4] << 8) | bytes[5];
    const payload = bytes.subarray(HEADER);

    let entry = this.pending.get(msgId);
    if (!entry || entry.total !== total) {
      entry = { total, flags, parts: new Array(total), got: 0 };
      this.pending.set(msgId, entry);
    }
    if (entry.parts[seq] === undefined) {
      entry.parts[seq] = payload;
      entry.got += 1;
    }
    if (entry.got !== entry.total) return;
    this.pending.delete(msgId);

    const whole = concat(entry.parts);
    if (!entry.flags) {
      this.deliver(JSON.parse(fromUtf8(whole)));
      return;
    }
    const iv = whole.subarray(0, 12);
    const body = whole.subarray(12);
    const aad = utf8(`vela-ble/1|c2p|${msgId}`);
    const plain = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv, additionalData: aad }, this.session.key, body,
    );
    this.deliver(JSON.parse(fromUtf8(new Uint8Array(plain))));
  }

  iv(direction) {
    const counter = (this.counters[direction] += 1n);
    const out = new Uint8Array(12);
    out.set(utf8(direction === 'c2p' ? 'C2P.' : 'P2C.'), 0);
    for (let i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
    return out;
  }

  // Push one message to the browser as notifications on p2c.
  async notify(payload, flags) {
    const msgId = (this.msgId = (this.msgId + 1) & 0xff);
    const total = Math.max(1, Math.ceil(payload.length / this.chunk));
    for (let seq = 0; seq < total; seq++) {
      const slice = payload.subarray(seq * this.chunk, (seq + 1) * this.chunk);
      const frame = new Uint8Array(HEADER + slice.length);
      frame[0] = flags;
      frame[1] = msgId;
      frame[2] = (seq >> 8) & 0xff;
      frame[3] = seq & 0xff;
      frame[4] = (total >> 8) & 0xff;
      frame[5] = total & 0xff;
      frame.set(slice, HEADER);
      await this.cdp.send('BluetoothEmulation.simulateCharacteristicOperationResponse', {
        characteristicId: this.ids.p2c,
        type: 'read',
        code: 0,
        data: b64(frame),
      });
      await sleep(5);
    }
    return total;
  }

  sendPlain(object) {
    return this.notify(utf8(JSON.stringify(object)), 0);
  }

  async send(object) {
    object.v = 1;
    object.n = ++this.outgoing;
    const msgId = (this.msgId + 1) & 0xff;
    const iv = this.iv('p2c');
    const aad = utf8(`vela-ble/1|p2c|${msgId}`);
    const ct = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: aad }, this.session.key, utf8(JSON.stringify(object)),
    );
    return this.notify(concat([iv, new Uint8Array(ct)]), 1);
  }
}

// --- run ---------------------------------------------------------------------

const profile = mkdtempSync(join(tmpdir(), 'ble-'));
const server = spawn('python3', [join(SB, 'tls-serve.py'), root, String(PORT), join(SB, 'cert.pem'), join(SB, 'key.pem')], { stdio: 'ignore' });
const chrome = spawn(process.env.CHROME_BIN, [
  '--headless=new', `--remote-debugging-port=${CDP}`,
  `--user-data-dir=${profile}`,
  `--host-resolver-rules=MAP getvela.app:443 127.0.0.1:${PORT}`,
  '--ignore-certificate-errors', '--no-proxy-server',
  '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });

const report = [];
function say(line) {
  report.push(line);
  console.log(line);
}

try {
  for (let i = 0; i < 80; i++) {
    try { await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json(); break; } catch { await sleep(250); }
  }
  await sleep(1200);

  // BluetoothEmulation and DeviceAccess live on the BROWSER endpoint, not on a
  // page session — sending them to a tab answers "wasn't found".
  const version = await (await fetch(`http://127.0.0.1:${CDP}/json/version`)).json();
  const browserCdp = await Cdp.open(version.webSocketDebuggerUrl);

  const tab = await (await fetch(
    `http://127.0.0.1:${CDP}/json/new?https://getvela.app/samples/ble-harness.html`, { method: 'PUT' },
  )).json();
  const cdp = await Cdp.open(tab.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  await cdp.send('Page.enable');
  await sleep(900);

  // --- stand up a fake phone ------------------------------------------------
  await browserCdp.send('BluetoothEmulation.enable', { state: 'powered-on', leSupported: true });
  await browserCdp.send('BluetoothEmulation.simulatePreconnectedPeripheral', {
    address: ADDRESS,
    name: 'Vela · iPhone',
    manufacturerData: [],
    knownServiceUuids: [SERVICE],
  });
  const { serviceId } = await browserCdp.send('BluetoothEmulation.addService', {
    address: ADDRESS, serviceUuid: SERVICE,
  });
  const c2p = await browserCdp.send('BluetoothEmulation.addCharacteristic', {
    serviceId, characteristicUuid: C2P,
    properties: { write: true, writeWithoutResponse: true },
  });
  const p2c = await browserCdp.send('BluetoothEmulation.addCharacteristic', {
    serviceId, characteristicUuid: P2C,
    properties: { notify: true, read: true },
  });
  say(`peripheral up: service ${serviceId.slice(0, 8)}… c2p ${c2p.characteristicId.slice(0, 8)}… p2c ${p2c.characteristicId.slice(0, 8)}…`);

  const peripheral = new Peripheral(browserCdp, { c2p: c2p.characteristicId, p2c: p2c.characteristicId });

  await cdp.send('DeviceAccess.enable');
  cdp.on(async (message) => {
    if (message.method !== 'DeviceAccess.deviceRequestPrompted') return;
    const device = message.params.devices[0];
    say(`chooser offered: ${JSON.stringify(message.params.devices.map((d) => d.name))}`);
    await cdp.send('DeviceAccess.selectPrompt', { id: message.params.id, deviceId: device.id });
  });

  browserCdp.on(async (message) => {
    try {
      if (message.method.startsWith('Bluetooth')) {
        say('· event ' + message.method + ' ' + JSON.stringify(message.params).slice(0, 140));
      }
      if (message.method === 'BluetoothEmulation.gattOperationReceived') {
        await browserCdp.send('BluetoothEmulation.simulateGATTOperationResponse', {
          address: message.params.address, type: message.params.type, code: 0,
        });
      } else if (message.method === 'BluetoothEmulation.characteristicOperationReceived') {
        const { characteristicId, type, data } = message.params;
        if (type === 'subscribe-to-notifications') {
          peripheral.notifySubscribed = true;
          await browserCdp.send('BluetoothEmulation.simulateCharacteristicOperationResponse', {
            characteristicId, type, code: 0,
          });
          say('browser subscribed to notifications');
        } else if (type === 'write') {
          await browserCdp.send('BluetoothEmulation.simulateCharacteristicOperationResponse', {
            characteristicId, type, code: 0,
          });
          await peripheral.accept(unb64(data));
        }
      }
    } catch (error) {
      say('peripheral handler error: ' + error.message);
    }
  });

  // --- the browser starts the ceremony (needs a user gesture) ---------------
  cdp.send('Runtime.evaluate', { expression: 'window.__run()', userGesture: true, awaitPromise: false });

  // --- handshake ------------------------------------------------------------
  let hello;
  try {
    hello = await peripheral.next(20000);
  } catch (error) {
    const dump = await cdp.send('Runtime.evaluate', {
      expression: 'JSON.stringify({ trace: window.__trace, state: window.__state, hasBt: !!navigator.bluetooth, loaded: !!(window.VelaCS && window.VelaCS.transport) })',
      returnByValue: true,
    });
    say('browser side says: ' + dump.result.value);
    throw error;
  }
  if (hello.t !== 'hello') throw new Error('expected hello, got ' + hello.t);
  say(`hello from browser: role=${hello.role}, pk ${hello.pk.length} chars`);

  const ours = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits']);
  const ourPk = new Uint8Array(await crypto.subtle.exportKey('raw', ours.publicKey));
  const ourNonce = crypto.getRandomValues(new Uint8Array(16));
  const peerKey = await crypto.subtle.importKey('raw', unb64url(hello.pk), { name: 'ECDH', namedCurve: 'P-256' }, false, []);
  const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, ours.privateKey, 256));
  const salt = concat([unb64url(hello.nonce), ourNonce]);
  const keyBytes = await hkdf(shared, salt, 'vela-ble/1 key', 32);
  const codeBytes = await hkdf(shared, salt, 'vela-ble/1 code', 4);
  peripheral.session = { key: await crypto.subtle.importKey('raw', keyBytes, 'AES-GCM', false, ['encrypt', 'decrypt']) };
  const code = String((((codeBytes[0] << 24) >>> 0) + (codeBytes[1] << 16) + (codeBytes[2] << 8) + codeBytes[3]) % 1000000).padStart(6, '0');
  say(`peripheral comparison code: ${code}`);

  await peripheral.sendPlain({
    v: 1, t: 'hello', role: 'requester', app: 'vela-reference-peripheral/0.1',
    pk: b64url(ourPk), nonce: b64url(ourNonce),
  });

  // --- push a real intent ---------------------------------------------------
  const fixtures = JSON.parse(readFileSync(join(root, 'samples/intents.json'), 'utf8'));
  const chosen = fixtures.cases.find((c) => c.code === 'CS1');
  await sleep(600);
  const frames = await peripheral.send({
    t: 'intent', id: 'e6f3a1', intent: chosen.intent, context: chosen.context,
    expires: Math.floor(Date.now() / 1000) + 120,
  });
  say(`intent pushed as ${frames} frames of ≤${peripheral.chunk} bytes`);

  // --- the signature comes back --------------------------------------------
  const result = await peripheral.next(20000);
  say(`browser answered: ${result.t} n=${result.n} sig=${(result.signature || '').slice(0, 18)}…`);

  const state = await cdp.send('Runtime.evaluate', {
    expression: 'JSON.stringify({ code: window.__state.code, rendered: window.__state.rendered, error: window.__state.error, trace: window.__trace })',
    returnByValue: true,
  });
  const browser = JSON.parse(state.result.value);

  say('');
  say('=== verdict ===');
  say(`codes match            : ${browser.code === code ? 'YES (' + code + ')' : 'NO (' + browser.code + ' vs ' + code + ')'}`);
  say(`sheet rendered         : ${browser.rendered ? browser.rendered.intentKey + ' / ' + browser.rendered.risk : 'NO'}`);
  say(`sentence               : ${browser.rendered ? browser.rendered.sentence : '—'}`);
  say(`signature returned     : ${result.t === 'result' && result.signature ? 'YES' : 'NO'}`);
  say(`browser-side error     : ${browser.error || 'none'}`);
} catch (error) {
  say('FAILED: ' + error.message);
  process.exitCode = 1;
} finally {
  chrome.kill();
  server.kill();
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* ignore */ }
}
