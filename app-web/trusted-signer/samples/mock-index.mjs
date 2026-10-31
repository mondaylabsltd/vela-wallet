// A local passkey index for a device run that CREATES a wallet (the Safari
// route's simulator e2e, spec 102 R8): the registry calls the wallet makes to
// create one, answered on this machine, so no run writes to the real index.
//
//   bun samples/mock-index.mjs --port 10195
//
// What it answers, as p256-index v2 does (`app-web/vela-wallet/src/lib/
// onboarding/core/registry.ts`):
//
//   GET  /api/health      the service, `ok`, its chain and `domainRegistry`
//   POST /api/challenge   member mode — the HONEST challenge,
//                         keccak256(abi.encode(chainId, domainRegistry, rpId,
//                         publicKey, keccak256(abi.encode(groupPublicKey,
//                         attestation)))), with the page's own keccak and ABI
//                         encoder (mock-registry.mjs, pinned against the live
//                         registry by ceremony-test.mjs). The page computes
//                         the same bytes itself and the wallet's core demands
//                         the two be equal, so this cannot be faked to pass.
//                         Group mode — fresh random challenges per member and
//                         for the group (signed by the wallet's software group
//                         key and the replayed member proofs).
//   POST /api/register    accepted and landed at once (`status: done`); the
//                         group is kept, so a lookup by key finds it
//   GET  /api/task/:id    done
//   GET  /api/query?publicKey=… | ?unitId=…   what it was told
//
// It checks no proof: it is the index's PLACE in the flow, not its rules —
// which are the registry contract's and the service's, tested in their repos.
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { loadPageLibs } from './test-kit.mjs';

const ns = loadPageLibs(['src/lib/keccak.js', 'src/lib/abi.js', 'src/lib/encode.js', 'src/lib/signer.js',
  'src/lib/ceremony.js']);
const arg = (name, fallback) => {
  const at = process.argv.indexOf(`--${name}`);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const port = Number(arg('port', '10195'));
const chainId = Number(arg('chain', '100'));
const domainRegistry = arg('registry', '0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf');

const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');
const value = (bytes) => ({ challenge: hex(bytes), challengeBase64url: Buffer.from(bytes).toString('base64url') });
const units = [];

function memberChallenge(body) {
  const binding = ns.ceremony.memberBinding(body.groupPublicKey, body.attestation || '');
  const challenge = ns.ceremony.memberChallenge({
    chainId, registry: domainRegistry, rpId: body.rpId, publicKey: body.publicKey, binding,
  });
  return { ...value(challenge), binding: hex(binding) };
}

const headers = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET, POST, OPTIONS',
  'access-control-allow-headers': 'content-type',
  'content-type': 'application/json',
};

createServer((req, res) => {
  if (req.method === 'OPTIONS') { res.writeHead(204, headers).end(); return; }
  let raw = '';
  req.on('data', (chunk) => { raw += chunk; });
  req.on('end', () => {
    const url = new URL(req.url, 'http://index');
    let body = null;
    try { body = raw ? JSON.parse(raw) : null; } catch { body = null; }
    const answer = (status, json) => {
      console.log(`${req.method} ${url.pathname}${url.search} → ${status}`);
      res.writeHead(status, headers).end(JSON.stringify(json));
    };
    if (req.method === 'GET' && url.pathname === '/api/health') {
      return answer(200, { service: 'webauthn-p256-publickey-registry', status: 'ok', chainId, domainRegistry });
    }
    if (req.method === 'POST' && url.pathname === '/api/challenge' && body) {
      if (Array.isArray(body.members)) {
        return answer(200, {
          contentHash: hex(randomBytes(32)),
          groupChallenge: value(randomBytes(32)),
          members: body.members.map((m) => ({ publicKey: m.publicKey, ...value(randomBytes(32)) })),
        });
      }
      if (body.publicKey && body.groupPublicKey) return answer(200, memberChallenge(body));
    }
    if (req.method === 'POST' && url.pathname === '/api/register' && body) {
      units.push(body);
      return answer(200, { id: `task-${units.length}`, status: 'done', contentHash: hex(randomBytes(32)) });
    }
    if (req.method === 'GET' && url.pathname.startsWith('/api/task/')) {
      return answer(200, { id: url.pathname.slice('/api/task/'.length), status: 'done' });
    }
    if (req.method === 'GET' && url.pathname === '/api/query') {
      const key = (url.searchParams.get('publicKey') || '').toLowerCase();
      if (key) {
        const unitIds = units
          .map((u, i) => (u.members.some((m) => m.publicKey.toLowerCase() === key) ? i : -1))
          .filter((i) => i >= 0);
        if (!unitIds.length) return answer(404, { error: 'not found' });
        return answer(200, { entry: { publicKey: key }, groups: { total: unitIds.length, unitIds } });
      }
      const unit = units[Number(url.searchParams.get('unitId'))];
      if (!unit) return answer(404, { error: 'not found' });
      return answer(200, {
        unit: { metadata: unit.metadata || '0x' },
        members: { total: unit.members.length, items: unit.members },
      });
    }
    answer(400, { error: 'bad request' });
  });
}).listen(port, '127.0.0.1', () => console.log(`mock index http://127.0.0.1:${port} (chain ${chainId}, ${domainRegistry})`));
