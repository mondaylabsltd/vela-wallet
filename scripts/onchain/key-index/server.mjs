#!/usr/bin/env node
// A local passkey key index (p256-index v2), in memory — the stand-in a test
// run creates its throwaway accounts in, so none of them lands in the
// production index (§28, docs/multisig-machines.md: a test build never writes
// to production; Round 17 wrote one there).
//
//   node scripts/onchain/key-index/server.mjs [--port 8793] [--chain 100]
//        [--registry 0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf]
//
// `KEY_INDEX_PORT` / `KEY_INDEX_CHAIN` / `KEY_INDEX_REGISTRY` work too. The
// multisig fork harness (`../multisig-fork/up.sh`) starts one.
//
// The protocol as the clients use it (`RegistryClient` on Android, iOS and the
// desktop; `app-web/vela-wallet/src/lib/onboarding/core/registry.ts`):
//
//   GET  /api/health      the service, `ok`, its chain and `domainRegistry`
//   POST /api/challenge   member mode `{rpId, groupPublicKey, publicKey,
//                         attestation?}` — the HONEST challenge the registry
//                         contract computes, keccak256(abi.encode(uint256
//                         chainId, address registry, string rpId, bytes
//                         publicKey, keccak256(abi.encode(bytes groupPublicKey,
//                         bytes attestation)))), so a wallet that checks it
//                         against its own derivation agrees. Group mode
//                         `{members: [...]}` — fresh random challenges.
//   POST /api/register    kept, and landed at once (`status: done`)
//   GET  /api/task/:id    done
//   GET  /api/query?publicKey=…            the units the key founded (404: none)
//   GET  /api/query?unitId=…[&pageSize&order]  a unit's metadata and members
//   GET  /api/query?credentialId=…         the v1 index's names: none here (404)
//
// It checks no proof: it is the index's PLACE in a flow, not its rules — those
// are the registry contract's and the service's, tested in their repositories.
// Nothing is written to disk; a restart is an empty index.
import { createServer } from 'node:http';
import { randomBytes } from 'node:crypto';
import { keccak_256 } from '@noble/hashes/sha3';

const arg = (name, env, fallback) => {
  const at = process.argv.indexOf(`--${name}`);
  return at > 0 ? process.argv[at + 1] : (process.env[env] ?? fallback);
};
const port = Number(arg('port', 'KEY_INDEX_PORT', '8793'));
const chainId = BigInt(arg('chain', 'KEY_INDEX_CHAIN', '100'));
const domainRegistry = arg('registry', 'KEY_INDEX_REGISTRY', '0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf');

// --- ABI, as much as the challenge needs ------------------------------------

const strip = (h) => String(h ?? '').replace(/^0x/i, '');
const fromHex = (h) => Buffer.from(strip(h).length % 2 ? '0' + strip(h) : strip(h), 'hex');
const word = (n) => Buffer.from(BigInt(n).toString(16).padStart(64, '0'), 'hex');
const padded = (bytes) => Buffer.concat([bytes, Buffer.alloc((32 - (bytes.length % 32)) % 32)]);

/** abi.encode for uint256 / address / bytes32 (static) and bytes / string (dynamic). */
function encode(types, values) {
  const heads = [];
  const tails = [];
  let offset = 32 * types.length;
  types.forEach((type, i) => {
    const value = values[i];
    if (type === 'uint256') heads.push(word(value));
    else if (type === 'address') heads.push(Buffer.concat([Buffer.alloc(12), fromHex(value)]));
    else if (type === 'bytes32') heads.push(padded(fromHex(value)).subarray(0, 32));
    else {
      const data = type === 'string' ? Buffer.from(String(value), 'utf8') : fromHex(value);
      heads.push(word(offset));
      const tail = Buffer.concat([word(data.length), padded(data)]);
      tails.push(tail);
      offset += tail.length;
    }
  });
  return Buffer.concat([...heads, ...tails]);
}

const keccak = (bytes) => Buffer.from(keccak_256(bytes));
const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');
const value = (bytes) => ({ challenge: hex(bytes), challengeBase64url: Buffer.from(bytes).toString('base64url') });

function memberChallenge(body) {
  const binding = keccak(encode(['bytes', 'bytes'], [body.groupPublicKey, body.attestation || '0x']));
  const challenge = keccak(
    encode(['uint256', 'address', 'string', 'bytes', 'bytes32'], [chainId, domainRegistry, body.rpId, body.publicKey, hex(binding)])
  );
  return { ...value(challenge), binding: hex(binding) };
}

// --- the index ---------------------------------------------------------------

/** Registered units, in order: a unit's id is its place here. */
const units = [];
const tasks = new Map();

const headers = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET, POST, OPTIONS',
  'access-control-allow-headers': 'content-type',
  'content-type': 'application/json',
};

const server = createServer((req, res) => {
  if (req.method === 'OPTIONS') return void res.writeHead(204, headers).end();
  let raw = '';
  req.on('data', (chunk) => (raw += chunk));
  req.on('end', () => {
    const url = new URL(req.url, 'http://index');
    let body = null;
    try {
      body = raw ? JSON.parse(raw) : null;
    } catch {
      body = null;
    }
    const answer = (status, json) => {
      console.log(`${req.method} ${url.pathname}${url.search} → ${status}`);
      res.writeHead(status, headers).end(JSON.stringify(json));
    };
    const at = url.pathname;
    if (req.method === 'GET' && at === '/api/health') {
      return answer(200, {
        service: 'webauthn-p256-publickey-registry',
        status: 'ok',
        chainId: Number(chainId),
        domainRegistry,
        local: true,
      });
    }
    if (req.method === 'POST' && at === '/api/challenge' && body) {
      if (Array.isArray(body.members)) {
        return answer(200, {
          contentHash: hex(randomBytes(32)),
          groupChallenge: value(randomBytes(32)),
          members: body.members.map((m) => ({ publicKey: m.publicKey, ...value(randomBytes(32)) })),
        });
      }
      if (body.publicKey && body.groupPublicKey) return answer(200, memberChallenge(body));
      return answer(400, { error: 'a challenge names its members, or a key and its group' });
    }
    if (req.method === 'POST' && at === '/api/register' && body && Array.isArray(body.members)) {
      units.push(body);
      const id = `task-${units.length}`;
      tasks.set(id, { id, status: 'done', unitId: units.length - 1 });
      return answer(200, { id, status: 'done', contentHash: hex(randomBytes(32)) });
    }
    if (req.method === 'GET' && at.startsWith('/api/task/')) {
      const id = decodeURIComponent(at.slice('/api/task/'.length));
      return tasks.has(id) ? answer(200, tasks.get(id)) : answer(404, { error: 'no such task' });
    }
    if (req.method === 'GET' && at === '/api/query') {
      const q = url.searchParams;
      if (q.has('walletRef')) return answer(400, { error: 'walletRef is not a v2 query' });
      if (q.has('credentialId')) return answer(404, { error: 'not found' });
      const key = (q.get('publicKey') || '').toLowerCase();
      if (key) {
        const unitIds = units
          .map((u, i) => (u.members.some((m) => String(m.publicKey).toLowerCase() === key) ? i : -1))
          .filter((i) => i >= 0);
        if (!unitIds.length) return answer(404, { error: 'not found' });
        return answer(200, { entry: { publicKey: key }, groups: { total: unitIds.length, unitIds } });
      }
      if (q.has('unitId')) {
        const unit = units[Number(q.get('unitId'))];
        if (!unit) return answer(404, { error: 'not found' });
        return answer(200, {
          unit: { metadata: unit.metadata || '0x' },
          members: { total: unit.members.length, items: unit.members },
        });
      }
    }
    answer(400, { error: 'bad request' });
  });
});

server.listen(port, '127.0.0.1', () => {
  console.log(`key index (local, in memory) http://127.0.0.1:${port} · chain ${chainId} · registry ${domainRegistry}`);
});
