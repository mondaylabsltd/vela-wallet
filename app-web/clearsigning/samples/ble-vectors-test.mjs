// The BLE framing, from the page's side of it (PROTOCOL.md §2).
//
// `rust/crates/vela-core/tests/clear-signer/ble-frames.json` is written by the
// core, which is what the three peripherals frame with. This reads the same
// file with the page's own `lib/transport/ble.js` — the central side of every
// BLE session — so a header the two spell differently fails here rather than
// on somebody's phone with a signing sheet open.
//
//   node samples/ble-vectors-test.mjs
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { makeChecks, root } from './test-kit.mjs';

const check = makeChecks();
const vectors = JSON.parse(
  readFileSync(join(root, '../../rust/crates/vela-core/tests/clear-signer/ble-frames.json'), 'utf8'),
);

// The page's transport is a browser module (it reaches for navigator.bluetooth
// at call time, not at load time), so it loads here the way the other samples
// load page code.
globalThis.window = globalThis;
const source = readFileSync(join(root, 'lib/transport/ble.js'), 'utf8');
(0, eval)(source);
const ble = globalThis.VelaCS?.transport?.ble;
if (!ble?._Reassembler) {
  console.error('lib/transport/ble.js exposes no _Reassembler — nothing to check the core against');
  process.exit(2);
}

const unhex = (text) => new Uint8Array(Buffer.from(text, 'hex'));
const hex = (bytes) => Buffer.from(bytes).toString('hex');

for (const vector of vectors.cases) {
  const seen = [];
  const reassembler = new ble._Reassembler(
    (flags, payload, msgId) => seen.push({ flags, payload, msgId }),
    (error) => seen.push({ error }),
  );
  for (const frame of vector.frames) reassembler.accept(unhex(frame));
  const got = seen[0];
  check(
    `${vector.name}: the page reassembles what the core framed`,
    seen.length === 1 && got && !got.error &&
      hex(got.payload) === vector.payload && got.msgId === vector.msgId && got.flags === vector.flags,
    seen.length === 1 && got?.payload ? `${got.payload.length} bytes, msgId ${got.msgId}` : JSON.stringify(seen).slice(0, 80),
  );
  // Every frame but the last must leave the message incomplete: a header the
  // page read differently would finish early, and the check above would still
  // pass on the bytes.
  const partial = new ble._Reassembler(() => {}, () => {});
  const completions = vector.frames.slice(0, -1).filter((frame) => {
    let done = false;
    const watcher = new ble._Reassembler(() => { done = true; }, () => {});
    for (const f of vector.frames.slice(0, vector.frames.indexOf(frame) + 1)) watcher.accept(unhex(f));
    return done;
  });
  check(`${vector.name}: no frame but the last completes it`, completions.length === 0,
    `${vector.frames.length} frame(s)`);
  partial.accept(unhex(vector.frames[0]));
}

// Out of order is the case a peripheral actually hits: notifications arrive as
// the radio delivers them, not as they were queued.
{
  const biggest = vectors.cases.reduce((a, b) => (a.frames.length >= b.frames.length ? a : b));
  let got = null;
  const reassembler = new ble._Reassembler((flags, payload) => { got = payload; }, () => {});
  for (const frame of [...biggest.frames].reverse()) reassembler.accept(unhex(frame));
  check(`${biggest.name}: the same frames backwards are the same message`,
    !!got && hex(got) === biggest.payload, `${biggest.frames.length} frames reversed`);
}

process.exit(check.summary() ? 0 : 1);
