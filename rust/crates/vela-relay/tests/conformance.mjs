#!/usr/bin/env node
// Conformance for a vela-relay/1 host — specs/075-clear-signer-channel/contracts/relay.md.
//
//   node rust/crates/vela-relay/tests/conformance.mjs http://127.0.0.1:8787
//   node rust/crates/vela-relay/tests/conformance.mjs https://relay.example
//   node rust/crates/vela-relay/tests/conformance.mjs http://127.0.0.1:8787 --slow
//
// WebSocket clients against one base URL (http(s):// or ws(s)://), Node 22+ for the
// global WebSocket. Every check uses a fresh room. Exits 0 only if all pass.
//
// The time limits — 120 s idle, 10-minute room life — take minutes, so they run
// only with --slow (about ten and a half minutes). Their rules are unit-tested
// with an injected clock in the rules crate (`cargo test -p vela-relay`); --slow
// is what shows a host's timers actually fire them.

import http from 'node:http';
import https from 'node:https';
import { randomBytes } from 'node:crypto';

const MAX_FRAME = 256 * 1024;
const JOINED = '{"v":1,"relay":"joined"}';
const LEFT = '{"v":1,"relay":"left"}';
const WAIT = 5000;
const QUIET = 300;
const IDLE = 120_000;
const LIFE = 600_000;
const SLOW = process.argv.includes('--slow');

if (typeof WebSocket !== 'function') {
  console.error('conformance: needs a global WebSocket (Node 22 or later)');
  process.exit(2);
}

const given = new URL(process.argv.slice(2).find((arg) => !arg.startsWith('--')) ?? 'http://127.0.0.1:8787');
const secure = given.protocol === 'https:' || given.protocol === 'wss:';
const httpBase = new URL(given);
httpBase.protocol = secure ? 'https:' : 'http:';
const wsBase = new URL(given);
wsBase.protocol = secure ? 'wss:' : 'ws:';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const roomId = () => randomBytes(16).toString('base64url'); // 22 characters

function show(value) {
  if (value instanceof Uint8Array) return `<${value.length} bytes>`;
  if (typeof value === 'string') return value.length > 80 ? `${JSON.stringify(value.slice(0, 80))}… (${value.length})` : JSON.stringify(value);
  return String(value);
}

function eq(actual, expected, what = 'value') {
  if (actual !== expected) throw new Error(`${what}: expected ${show(expected)}, got ${show(actual)}`);
}

function sameBytes(actual, expected, what = 'frame') {
  if (!(actual instanceof Uint8Array)) throw new Error(`${what}: expected a binary frame, got ${show(actual)}`);
  if (Buffer.compare(Buffer.from(actual), Buffer.from(expected)) !== 0) {
    throw new Error(`${what}: ${actual.length} bytes differ from the ${expected.length} sent`);
  }
}

/** Every socket a check opened, closed once the check ends. */
const opened = new Set();

/** One end of a room: a WebSocket plus an inbox of its frames. */
class End {
  static open(room, role) {
    const ws = new WebSocket(new URL(`/v1/rooms/${room}?role=${role}`, wsBase));
    ws.binaryType = 'arraybuffer';
    const end = new End(ws, role);
    opened.add(end);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`${role}: no open within ${WAIT} ms`)), WAIT);
      ws.addEventListener('open', () => (clearTimeout(timer), resolve(end)), { once: true });
      ws.addEventListener('close', (ev) => (clearTimeout(timer), reject(new Error(`${role}: refused (${ev.code})`))), { once: true });
    });
  }

  constructor(ws, role) {
    this.ws = ws;
    this.role = role;
    this.inbox = [];
    this.waiting = null;
    this.close = null;
    this.closed = new Promise((resolve) => {
      ws.addEventListener('close', (ev) => {
        this.close = { code: ev.code, reason: ev.reason };
        resolve(this.close);
        this.waiting?.reject(new Error(`${role}: closed ${ev.code} ${ev.reason} while a frame was awaited`));
      });
    });
    ws.addEventListener('message', (ev) => {
      const data = typeof ev.data === 'string' ? ev.data : new Uint8Array(ev.data);
      if (this.waiting) this.waiting.resolve(data);
      else this.inbox.push(data);
    });
  }

  send(data) {
    this.ws.send(data);
  }

  /** The next frame: a string for text, a Uint8Array for binary. */
  next(ms = WAIT) {
    if (this.inbox.length) return Promise.resolve(this.inbox.shift());
    if (this.close) return Promise.reject(new Error(`${this.role}: already closed ${this.close.code}`));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiting = null;
        reject(new Error(`${this.role}: no frame within ${ms} ms`));
      }, ms);
      this.waiting = {
        resolve: (data) => (clearTimeout(timer), (this.waiting = null), resolve(data)),
        reject: (err) => (clearTimeout(timer), (this.waiting = null), reject(err)),
      };
    });
  }

  /** Nothing arrives for a little while. */
  async quiet(ms = QUIET) {
    await sleep(ms);
    if (this.inbox.length) throw new Error(`${this.role}: expected nothing, got ${show(this.inbox[0])}`);
  }

  /** The close the relay sent. */
  async closedWith(ms = WAIT) {
    const timeout = sleep(ms).then(() => {
      throw new Error(`${this.role}: not closed within ${ms} ms`);
    });
    return Promise.race([this.closed, timeout]);
  }

  shut() {
    if (this.ws.readyState <= WebSocket.OPEN) this.ws.close(1000);
  }
}

/** A fresh room with both ends in and told `joined`. */
async function pair() {
  const room = roomId();
  const requester = await End.open(room, 'requester');
  const signer = await End.open(room, 'signer');
  eq(await signer.next(), JOINED, 'signer, first frame');
  eq(await requester.next(), JOINED, 'requester, first frame');
  return { room, requester, signer };
}

/** The HTTP status of a WebSocket upgrade request (101 if it upgraded). */
function upgradeStatus(pathAndQuery) {
  return new Promise((resolve, reject) => {
    const lib = secure ? https : http;
    const req = lib.request(new URL(pathAndQuery, httpBase), {
      agent: false,
      headers: {
        Connection: 'Upgrade',
        Upgrade: 'websocket',
        'Sec-WebSocket-Version': '13',
        'Sec-WebSocket-Key': randomBytes(16).toString('base64'),
      },
    });
    req.setTimeout(WAIT, () => req.destroy(new Error(`no answer within ${WAIT} ms`)));
    req.on('response', (res) => {
      res.resume();
      resolve(res.statusCode);
    });
    req.on('upgrade', (_res, socket) => {
      socket.destroy();
      resolve(101);
    });
    req.on('error', reject);
    req.end();
  });
}

const checks = [];
const check = (name, fn, slow = false) => checks.push({ name, fn, slow });

check('GET /healthz answers 200 "ok"', async () => {
  const res = await fetch(new URL('/healthz', httpBase));
  eq(res.status, 200, 'status');
  eq(await res.text(), 'ok', 'body');
});

check('pairing: both ends get {"v":1,"relay":"joined"} as text; nothing is buffered', async () => {
  const room = roomId();
  const requester = await End.open(room, 'requester');
  requester.send('early: nobody is here yet');
  requester.send(randomBytes(32));
  await sleep(QUIET); // let the relay see (and drop) them before the signer arrives
  const signer = await End.open(room, 'signer');
  eq(await signer.next(), JOINED, 'signer, first frame');
  eq(await requester.next(), JOINED, 'requester, first frame');
  await signer.quiet();
  await requester.quiet();
});

check('text frames are forwarded byte-for-byte, both ways', async () => {
  const { requester, signer } = await pair();
  const texts = [
    '{"v":1,"t":"hello","role":"signer","pk":"BHr0…","nonce":"q5x0VbXk3mFqL1a2zY-_Aw"}',
    'héllo, 世界 🚀 — ünïcødé',
    '\u0000\u0001\u007f control \t\r\n',
    '',
    'x'.repeat(70_000),
  ];
  for (const text of texts) {
    signer.send(text);
    eq(await requester.next(), text, 'signer → requester');
    requester.send(text);
    eq(await signer.next(), text, 'requester → signer');
  }
});

check('binary frames are forwarded byte-for-byte, both ways, up to 256 KiB', async () => {
  const { requester, signer } = await pair();
  for (const size of [0, 1, 28, 1000, 65_536, MAX_FRAME]) {
    const bytes = randomBytes(size);
    signer.send(bytes);
    sameBytes(await requester.next(), bytes, `signer → requester (${size} bytes)`);
    requester.send(bytes);
    sameBytes(await signer.next(), bytes, `requester → signer (${size} bytes)`);
  }
});

check('frames arrive in the order sent, text and binary interleaved', async () => {
  const { requester, signer } = await pair();
  const n = 200;
  for (let i = 0; i < n; i++) signer.send(i % 2 ? `#${i}` : Uint8Array.of(i & 255, i >> 8));
  for (let i = 0; i < n; i++) {
    const got = await requester.next();
    if (i % 2) eq(got, `#${i}`, `frame ${i}`);
    else sameBytes(got, Uint8Array.of(i & 255, i >> 8), `frame ${i}`);
  }
});

check('a third connection in a taken role is closed 4409; the incumbents are untouched', async () => {
  const { room, requester, signer } = await pair();
  for (const role of ['signer', 'requester']) {
    const third = await End.open(room, role);
    eq((await third.closedWith()).code, 4409, `third ${role}, close code`);
  }
  await requester.quiet();
  await signer.quiet();
  signer.send('still paired');
  eq(await requester.next(), 'still paired', 'signer → requester');
  requester.send(Uint8Array.of(1, 2, 3));
  sameBytes(await signer.next(), Uint8Array.of(1, 2, 3), 'requester → signer');
});

check('an over-size frame (256 KiB + 1) closes its sender 1009; the peer hears left', async () => {
  for (const [kind, frame] of [
    ['binary', randomBytes(MAX_FRAME + 1)],
    ['text', 'x'.repeat(MAX_FRAME + 1)],
  ]) {
    const { requester, signer } = await pair();
    signer.send(frame);
    eq((await signer.closedWith()).code, 1009, `${kind}: close code`);
    eq(await requester.next(), LEFT, `${kind}: requester`);
  }
});

check('a leave is announced with left; a reconnect in that role pairs again', async () => {
  const { room, requester, signer } = await pair();
  signer.shut();
  eq(await requester.next(), LEFT, 'requester after the signer left');
  requester.send('into the void'); // no peer: dropped
  await sleep(QUIET);
  const signer2 = await End.open(room, 'signer');
  eq(await signer2.next(), JOINED, 'new signer');
  eq(await requester.next(), JOINED, 'requester, again');
  await signer2.quiet();
  signer2.send('back');
  eq(await requester.next(), 'back', 'new signer → requester');

  requester.shut();
  eq(await signer2.next(), LEFT, 'signer after the requester left');
  const requester2 = await End.open(room, 'requester');
  eq(await requester2.next(), JOINED, 'new requester');
  eq(await signer2.next(), JOINED, 'signer, again');
  requester2.send(Uint8Array.of(9));
  sameBytes(await signer2.next(), Uint8Array.of(9), 'new requester → signer');
});

check('bad room ids and roles are refused: 400 before the upgrade, or 4400 after it', async () => {
  const good = roomId();
  const cases = [
    `/v1/rooms/?role=signer`,
    `/v1/rooms/${good.slice(0, 21)}?role=signer`, // 21 characters
    `/v1/rooms/${good}A?role=signer`, // 23 characters
    `/v1/rooms/${good.slice(0, 20)}+/?role=signer`, // standard base64
    `/v1/rooms/${good.slice(0, 21)}=?role=signer`, // padding
    `/v1/rooms/${good.slice(0, 21)}.?role=signer`,
    `/v1/rooms/${good}?role=admin`,
    `/v1/rooms/${good}?role=Signer`,
    `/v1/rooms/${good}?role=`,
    `/v1/rooms/${good}`,
    `/v1/rooms/${good}?role=signer&role=requester`,
  ];
  for (const url of cases) {
    const status = await upgradeStatus(url);
    if (status === 400) continue;
    if (status !== 101) throw new Error(`${url}: expected 400 (or 101 then 4400), got ${status}`);
    // A host that can only refuse after the upgrade must close 4400.
    const ws = new WebSocket(new URL(url, wsBase));
    const code = await new Promise((resolve) => ws.addEventListener('close', (ev) => resolve(ev.code)));
    eq(code, 4400, `${url}: close code`);
  }
});

/** Closed with `code`/`reason`, `at` ms after `start` (give or take `slack`). */
async function closedAt(end, start, at, code, reason, slack = 15_000) {
  const close = await end.closedWith(at - (Date.now() - start) + slack);
  eq(close.code, code, `${end.role}, close code`);
  eq(close.reason, reason, `${end.role}, close reason`);
  const after = Date.now() - start;
  if (after < at - 2000) throw new Error(`${end.role}: closed after ${after} ms, before ${at} ms`);
  return after;
}

if (SLOW) {
  check('an end silent for 120 s is closed 4408 "idle"; its peer hears left (slow)', async () => {
    const start = Date.now();
    const { requester, signer } = await pair();
    // The signer keeps talking; the requester only listens.
    const talk = setInterval(() => signer.send('tick'), 30_000);
    try {
      const after = await closedAt(requester, start, IDLE, 4408, 'idle');
      eq(await signer.next(), LEFT, 'signer after the requester idled');
      return `requester closed after ${(after / 1000).toFixed(1)} s`;
    } finally {
      clearInterval(talk);
    }
  }, true);

  check('a room is closed 4408 "expired" for both ends 10 minutes after creation (slow)', async () => {
    const start = Date.now();
    const { requester, signer } = await pair();
    // Both ends stay busy, so only the room's age can end it.
    const talk = setInterval(() => {
      signer.send('tick');
      requester.send('tock');
    }, 50_000);
    try {
      const [a, b] = await Promise.all([
        closedAt(requester, start, LIFE, 4408, 'expired'),
        closedAt(signer, start, LIFE, 4408, 'expired'),
      ]);
      return `closed after ${(a / 1000).toFixed(1)} s and ${(b / 1000).toFixed(1)} s`;
    } finally {
      clearInterval(talk);
    }
  }, true);
}

let failed = 0;
async function run({ name, fn }) {
  const started = Date.now();
  try {
    const note = await fn();
    console.log(`  ok    ${name}  (${note ?? `${Date.now() - started} ms`})`);
  } catch (err) {
    failed++;
    console.log(`  FAIL  ${name}\n          ${err.message}`);
  }
}
const closeAll = () => {
  for (const end of opened) end.shut();
  opened.clear();
};

console.log(`vela-relay/1 conformance against ${given.origin}\n`);
for (const c of checks.filter((c) => !c.slow)) {
  await run(c);
  closeAll();
}
const slow = checks.filter((c) => c.slow);
if (slow.length) {
  console.log(`\n  … ${slow.length} slow checks, side by side (about ${Math.ceil((LIFE + 30_000) / 60_000)} minutes)`);
  await Promise.all(slow.map(run));
  closeAll();
}
console.log(`\n${checks.length - failed}/${checks.length} checks passed`);
process.exit(failed ? 1 : 0);
