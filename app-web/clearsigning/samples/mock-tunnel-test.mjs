// The mock tunnel against contracts/tunnel.md §1 — the rules the page's tunnel
// tests lean on. (The Rust tunnel has its own conformance script; this only
// makes sure the page is tested against the same room rules.)
//
//   node samples/mock-tunnel-test.mjs
import { startTunnel } from './mock-tunnel.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
function check(name, pass, detail) {
  results.push(!!pass);
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  — ' + detail : ''}`);
}

function client(url) {
  const ws = new WebSocket(url);
  ws.binaryType = 'arraybuffer';
  const got = [];
  const state = { ws, got, closed: null };
  ws.addEventListener('message', (e) => got.push(e.data));
  ws.addEventListener('close', (e) => { state.closed = { code: e.code, reason: e.reason }; });
  state.opened = new Promise((resolve) => {
    ws.addEventListener('open', () => resolve(true));
    ws.addEventListener('error', () => resolve(false));
  });
  return state;
}

async function until(fn, ms = 2000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { if (fn()) return true; await sleep(20); }
  return false;
}

const tunnel = await startTunnel({ limits: { idleMs: 600, roomLifetimeMs: 60000, maxFrame: 1024 } });
const room = 'AAAAAAAAAAAAAAAAAAAAAA';
try {
  const health = await fetch(`http://127.0.0.1:${tunnel.port}/healthz`);
  check('GET /healthz → 200 "ok"', health.status === 200 && (await health.text()) === 'ok');

  const bad = client(`${tunnel.url}/v1/rooms/short?role=signer`);
  check('a room id that is not 22 base64url characters is refused before the upgrade', !(await bad.opened));

  const badRole = client(`${tunnel.url}/v1/rooms/${room}?role=king`);
  await badRole.opened;
  check('a bad role is closed with 4400', await until(() => badRole.closed && badRole.closed.code === 4400));

  const signer = client(`${tunnel.url}/v1/rooms/${room}?role=signer`);
  await signer.opened;
  signer.ws.send('dropped: nobody is here');
  const requester = client(`${tunnel.url}/v1/rooms/${room}?role=requester`);
  await requester.opened;
  await until(() => signer.got.length && requester.got.length);
  check('both ends get {"v":1,"relay":"joined"}',
    signer.got[0] === '{"v":1,"relay":"joined"}' && requester.got[0] === '{"v":1,"relay":"joined"}');
  check('a frame sent while the peer was absent was dropped, not buffered',
    !requester.got.includes('dropped: nobody is here'));

  const second = client(`${tunnel.url}/v1/rooms/${room}?role=signer`);
  await second.opened;
  check('a second signer is closed with 4409, the incumbent untouched',
    await until(() => second.closed && second.closed.code === 4409) && signer.closed === null);

  signer.ws.send('{"t":"hello"}');
  signer.ws.send(new Uint8Array([1, 2, 3, 250]));
  requester.ws.send(new Uint8Array([9, 8, 7]));
  await until(() => requester.got.length >= 3 && signer.got.length >= 2);
  check('text is forwarded verbatim', requester.got[1] === '{"t":"hello"}');
  check('binary is forwarded byte for byte, in order',
    requester.got[2] instanceof ArrayBuffer && Buffer.from(requester.got[2]).equals(Buffer.from([1, 2, 3, 250])));
  check('both directions', signer.got[1] instanceof ArrayBuffer && Buffer.from(signer.got[1]).equals(Buffer.from([9, 8, 7])));

  requester.ws.close();
  await until(() => signer.got.includes('{"v":1,"relay":"left"}'));
  check('when one leaves the other gets {"v":1,"relay":"left"}', signer.got.includes('{"v":1,"relay":"left"}'));
  const back = client(`${tunnel.url}/v1/rooms/${room}?role=requester`);
  await back.opened;
  check('the room waits: a reconnect in that role is joined again',
    await until(() => back.got[0] === '{"v":1,"relay":"joined"}' && signer.got.filter((m) => m === '{"v":1,"relay":"joined"}').length === 2));

  signer.ws.send(new Uint8Array(2048));
  check('a frame over the limit is closed with 1009', await until(() => signer.closed && signer.closed.code === 1009));

  const quiet = client(`${tunnel.url}/v1/rooms/BBBBBBBBBBBBBBBBBBBBBB?role=signer`);
  await quiet.opened;
  check('an end silent past the idle limit is closed with 4408', await until(() => quiet.closed && quiet.closed.code === 4408, 2000));
  await sleep(100);
  check('an empty room is forgotten', !tunnel.rooms.has('BBBBBBBBBBBBBBBBBBBBBB'));
} finally {
  await tunnel.close();
  const passed = results.filter(Boolean).length;
  console.log(`\n${passed}/${results.length} checks passed`);
  process.exitCode = passed === results.length ? 0 : 1;
}
