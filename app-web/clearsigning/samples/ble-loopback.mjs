// Runs the SHIPPED browser transport (lib/transport/ble.js) against the
// reference peripheral, with a stubbed GATT layer in between.
//
// Chrome's BluetoothEmulation is experimental and, in the build here, never
// completes the GATT phase — so this covers everything except the Web Bluetooth
// binding itself: framing, chunking, out-of-order reassembly, the ECDH
// handshake, the comparison code agreeing on both sides, AES-GCM in both
// directions, the replay guard, and the sheet the intent renders into.
//
//   node samples/ble-loopback.mjs
import { webcrypto } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');

const utf8 = (s) => new TextEncoder().encode(s);
const fromUtf8 = (b) => new TextDecoder().decode(b);
const b64url = (b) => Buffer.from(b).toString('base64url');
const unb64url = (s) => new Uint8Array(Buffer.from(s, 'base64url'));
const concat = (parts) => new Uint8Array(Buffer.concat(parts.map((p) => Buffer.from(p))));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const results = [];
function check(name, pass, detail) {
  results.push({ name, pass, detail });
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
}

// --- the fake radio ----------------------------------------------------------
//
// Two characteristics wired straight into the peripheral below. Writes go to
// the peripheral; the peripheral's notifications come back as
// `characteristicvaluechanged`, deliberately delivered OUT OF ORDER so the
// reassembler is actually exercised.

class Characteristic {
  constructor(uuid) {
    this.uuid = uuid;
    this.listeners = [];
    this.onWrite = null;
  }

  addEventListener(type, handler) {
    if (type === 'characteristicvaluechanged') this.listeners.push(handler);
  }

  startNotifications() {
    return Promise.resolve(this);
  }

  writeValueWithoutResponse(bytes) {
    return Promise.resolve(this.onWrite && this.onWrite(new Uint8Array(bytes)));
  }

  emit(bytes) {
    const value = new DataView(new Uint8Array(bytes).buffer);
    this.listeners.forEach((handler) => handler({ target: { value } }));
  }
}

// --- reference peripheral (same logic as samples/ble-peripheral.mjs) ---------

const HEADER = 6;

class Peripheral {
  constructor(out) {
    this.out = out;              // Characteristic the browser listens on
    this.chunk = 61;             // deliberately small: forces many frames
    this.pending = new Map();
    this.msgId = 0;
    this.counter = 0n;
    this.outgoing = 0;
    this.session = null;
    this.queue = [];
    this.waiters = [];
    this.shuffle = true;
  }

  deliver(message) {
    if (this.waiters.length) this.waiters.shift()(message);
    else this.queue.push(message);
  }

  next(timeoutMs = 5000) {
    if (this.queue.length) return Promise.resolve(this.queue.shift());
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('peripheral timed out')), timeoutMs);
      this.waiters.push((m) => { clearTimeout(timer); resolve(m); });
    });
  }

  async accept(bytes) {
    const flags = bytes[0];
    const msgId = bytes[1];
    const seq = (bytes[2] << 8) | bytes[3];
    const total = (bytes[4] << 8) | bytes[5];
    let entry = this.pending.get(msgId);
    if (!entry || entry.total !== total) {
      entry = { total, flags, parts: new Array(total), got: 0 };
      this.pending.set(msgId, entry);
    }
    if (entry.parts[seq] === undefined) {
      entry.parts[seq] = bytes.subarray(HEADER);
      entry.got += 1;
    }
    if (entry.got !== entry.total) return;
    this.pending.delete(msgId);

    const whole = concat(entry.parts);
    if (!entry.flags) {
      this.deliver(JSON.parse(fromUtf8(whole)));
      return;
    }
    const plain = await webcrypto.subtle.decrypt(
      {
        name: 'AES-GCM',
        iv: whole.subarray(0, 12),
        additionalData: utf8(`vela-ble/1|c2p|${msgId}`),
      },
      this.session.key,
      whole.subarray(12),
    );
    this.deliver(JSON.parse(fromUtf8(new Uint8Array(plain))));
  }

  iv() {
    const counter = (this.counter += 1n);
    const out = new Uint8Array(12);
    out.set(utf8('P2C.'), 0);
    for (let i = 0; i < 8; i++) out[11 - i] = Number((counter >> BigInt(8 * i)) & 0xffn);
    return out;
  }

  async emit(payload, flags) {
    const msgId = (this.msgId = (this.msgId + 1) & 0xff);
    const total = Math.max(1, Math.ceil(payload.length / this.chunk));
    const frames = [];
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
      frames.push(frame);
    }
    // Real BLE does not promise ordering. Swap the first two frames of every
    // multi-frame message so the reassembler has to cope.
    if (this.shuffle && frames.length > 1) {
      const first = frames[0];
      frames[0] = frames[1];
      frames[1] = first;
    }
    for (const frame of frames) {
      this.out.emit(frame);
      await sleep(0);
    }
    return total;
  }

  sendPlain(object) {
    return this.emit(utf8(JSON.stringify(object)), 0);
  }

  async send(object) {
    object.v = 1;
    object.n = ++this.outgoing;
    const msgId = (this.msgId + 1) & 0xff;
    const iv = this.iv();
    const ct = await webcrypto.subtle.encrypt(
      { name: 'AES-GCM', iv, additionalData: utf8(`vela-ble/1|p2c|${msgId}`) },
      this.session.key,
      utf8(JSON.stringify(object)),
    );
    return this.emit(concat([iv, new Uint8Array(ct)]), 1);
  }

  /** Re-emit the last encrypted message verbatim — must be rejected. */
  async replayLast() {
    if (!this.lastSealed) return;
    for (const frame of this.lastSealed) {
      this.out.emit(frame);
      await sleep(0);
    }
  }
}

async function hkdf(secret, salt, info, length) {
  const key = await webcrypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveBits']);
  const bits = await webcrypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt, info: utf8(info) }, key, length * 8,
  );
  return new Uint8Array(bits);
}

// --- load the shipped browser code into Node --------------------------------

const c2p = new Characteristic('c2p');
const p2c = new Characteristic('p2c');
const peripheral = new Peripheral(p2c);
c2p.onWrite = (bytes) => peripheral.accept(bytes);

globalThis.window = globalThis;
// Node exposes `crypto` as a getter-only global; the browser code wants the
// WebCrypto object, so define it rather than assign it.
Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
globalThis.btoa = (s) => Buffer.from(s, 'binary').toString('base64');
globalThis.atob = (s) => Buffer.from(s, 'base64').toString('binary');
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: {
  bluetooth: {
    requestDevice: async () => ({
      name: 'Vela · reference peripheral',
      gatt: {
        connect: async () => ({
          connected: true,
          disconnect() { this.connected = false; },
          getPrimaryService: async () => ({
            getCharacteristic: async (uuid) => (uuid.endsWith('0002-4000-8000-00805f9b34fb') ? c2p : p2c),
          }),
        }),
      },
    }),
  },
} });

for (const file of ['lib/i18n.js', 'lib/locales/en.js', 'lib/locales/zh.js', 'lib/keccak.js',
  'lib/identicon-features.js', 'lib/identicon.js',
  'lib/abi.js', 'lib/logos.js', 'lib/registry.js', 'lib/safeop.js', 'lib/resolve.js', 'lib/transport/ble.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const ns = globalThis.VelaCS;
ns.i18n.setLocale = ((original) => (locale) => { ns.i18n.__locale = locale; return original(locale); })(ns.i18n.setLocale);

// The locale setter touches document; give it the smallest possible stand-in.
globalThis.document = { documentElement: { setAttribute() {} } };
globalThis.localStorage = { getItem: () => null, setItem() {} };
ns.i18n.setLocale('zh');

// --- run ---------------------------------------------------------------------

let browserCode = null;
const connecting = ns.transport.ble.connect({ onCode: (code) => { browserCode = code; } });

// Peripheral half of the handshake.
const hello = await peripheral.next();
check('handshake: browser sends hello first', hello.t === 'hello' && !!hello.pk, `role=${hello.role}`);

const ours = await webcrypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits']);
const ourPk = new Uint8Array(await webcrypto.subtle.exportKey('raw', ours.publicKey));
const ourNonce = webcrypto.getRandomValues(new Uint8Array(16));
const peerKey = await webcrypto.subtle.importKey('raw', unb64url(hello.pk), { name: 'ECDH', namedCurve: 'P-256' }, false, []);
const shared = new Uint8Array(await webcrypto.subtle.deriveBits({ name: 'ECDH', public: peerKey }, ours.privateKey, 256));
const salt = concat([unb64url(hello.nonce), ourNonce]);
const keyBytes = await hkdf(shared, salt, 'vela-ble/1 key', 32);
const codeBytes = await hkdf(shared, salt, 'vela-ble/1 code', 4);
peripheral.session = { key: await webcrypto.subtle.importKey('raw', keyBytes, 'AES-GCM', false, ['encrypt', 'decrypt']) };
const peripheralCode = String(
  ((((codeBytes[0] << 24) >>> 0) + (codeBytes[1] << 16) + (codeBytes[2] << 8) + codeBytes[3]) % 1000000),
).padStart(6, '0');

await peripheral.sendPlain({
  v: 1, t: 'hello', role: 'requester', app: 'vela-reference-peripheral/0.1',
  pk: b64url(ourPk), nonce: b64url(ourNonce),
});

const channel = await connecting;
check('handshake: both sides derive the same comparison code', browserCode === peripheralCode,
  `${browserCode} / ${peripheralCode}`);
check('handshake: code is six digits', /^\d{6}$/.test(browserCode || ''), browserCode);

// --- a real intent, chunked --------------------------------------------------

const fixtures = JSON.parse(readFileSync(join(root, 'samples/intents.json'), 'utf8'));
const batch = fixtures.cases.find((c) => c.code === 'CS26'); // the biggest payload

let received = null;
channel.onMessage((message) => { if (message.t === 'intent') received = message; });

const frames = await peripheral.send({
  t: 'intent', id: 'e6f3a1', intent: batch.intent, context: batch.context,
  expires: Math.floor(Date.now() / 1000) + 120,
});
await sleep(80);

check('transfer: multi-frame message reassembled', !!received, `${frames} frames of ≤${peripheral.chunk} bytes`);
check('transfer: out-of-order frames survive', !!received && received.id === 'e6f3a1');
check('transfer: intent arrives byte-identical',
  !!received && JSON.stringify(received.intent) === JSON.stringify(batch.intent));

// --- the sheet ---------------------------------------------------------------

const view = ns.resolve(received.intent, received.context || {});
check('render: the received intent produces a sheet', !!view.intentKey,
  `${ns.i18n.t(view.intentKey)} / risk=${view.risk} / legs=${view.legs.length}`);
check('render: the requester origin is NOT treated as verified',
  view.warnings.some((w) => w.key === 'warn.claimedOrigin'));

// --- answer, replay, teardown -----------------------------------------------

await channel.send({ t: 'result', id: 'e6f3a1', signature: '0x' + 'ab'.repeat(64) });
const answer = await peripheral.next();
check('answer: signature reaches the peripheral, encrypted',
  answer.t === 'result' && answer.signature.length === 130, `n=${answer.n}`);

let replayRejected = false;
channel.onError((error) => { if (/replay/i.test(error.message)) replayRejected = true; });
const sealed = [];
const originalEmit = p2c.emit.bind(p2c);
p2c.emit = (bytes) => { sealed.push(bytes); originalEmit(bytes); };
await peripheral.send({ t: 'bye', reason: 'test' });
await sleep(40);
peripheral.outgoing -= 1; // rewind the counter so the next message replays an old n
await peripheral.send({ t: 'bye', reason: 'test' });
await sleep(60);
check('replay: a message whose counter does not advance is dropped', replayRejected);

console.log('');
const failed = results.filter((r) => !r.pass);
console.log(`${results.length - failed.length}/${results.length} checks passed`);
process.exitCode = failed.length ? 1 : 0;
