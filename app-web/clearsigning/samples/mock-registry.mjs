// A mock of the public-key registry's two calls the page makes for a member
// proof (p256-index: GET /api/health, POST /api/challenge in member mode).
//
// Honest by default: it computes the member-mode challenge the way the real
// registry does — keccak256(abi.encode(chainId, domainRegistry, rpId,
// publicKey, keccak256(abi.encode(groupPublicKey, attestation)))) — with the
// page's own keccak and ABI encoder, which samples/ceremony-test.mjs pins
// against answers from the live registry. `lie` makes it dishonest, for the
// hostile tests:
//
//   'otherGroup'  the challenge for another group key (a different wallet)
//   'otherKey'    the challenge for another member key
//   'safeOp'      32 bytes that are not a registry challenge at all
//   'binding'     the right challenge, a wrong `binding` beside it
//   'down'        500 on every call
//
// CORS is open, as the real registry's is.
import { createServer } from 'node:http';

export function startRegistry({ ns, chainId = 100, domainRegistry = '0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf', lie = null } = {}) {
  const requests = [];
  const state = { lie };
  const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');
  const b64url = (bytes) => Buffer.from(bytes).toString('base64url');

  function challengeFor(body) {
    let group = body.groupPublicKey;
    let key = body.publicKey;
    if (state.lie === 'otherGroup') group = '04' + 'ab'.repeat(64);
    if (state.lie === 'otherKey') key = '04' + 'cd'.repeat(64);
    const binding = ns.ceremony.memberBinding(group, body.attestation || '');
    const challenge = state.lie === 'safeOp'
      ? ns.keccak.hash(new Uint8Array(Buffer.from('1901' + '11'.repeat(64), 'hex')))
      : ns.ceremony.memberChallenge({ chainId, registry: domainRegistry, rpId: body.rpId, publicKey: key, binding });
    const answer = { challenge: hex(challenge), challengeBase64url: b64url(challenge), binding: hex(binding) };
    if (state.lie === 'binding') answer.binding = '0x' + '00'.repeat(32);
    return answer;
  }

  const server = createServer((req, res) => {
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-methods': 'GET, POST, OPTIONS',
      'access-control-allow-headers': 'content-type',
      'content-type': 'application/json',
    };
    if (req.method === 'OPTIONS') { res.writeHead(204, headers).end(); return; }
    let raw = '';
    req.on('data', (chunk) => { raw += chunk; });
    req.on('end', () => {
      const url = new URL(req.url, 'http://registry');
      let body = null;
      try { body = raw ? JSON.parse(raw) : null; } catch { body = null; }
      requests.push({ method: req.method, path: url.pathname, body, origin: req.headers.origin || null });
      if (state.lie === 'down') { res.writeHead(500, headers).end('{"error":"down"}'); return; }
      if (req.method === 'GET' && url.pathname === '/api/health') {
        res.writeHead(200, headers).end(JSON.stringify({
          service: 'webauthn-p256-publickey-registry', status: 'ok', chainId, domainRegistry,
        }));
        return;
      }
      if (req.method === 'POST' && url.pathname === '/api/challenge' && body && body.publicKey && body.groupPublicKey) {
        res.writeHead(200, headers).end(JSON.stringify(challengeFor(body)));
        return;
      }
      res.writeHead(400, headers).end('{"error":"bad request"}');
    });
  });

  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({
        url: `http://127.0.0.1:${server.address().port}`,
        requests,
        state,
        honestChallenge: (body) => {
          const saved = state.lie;
          state.lie = null;
          try { return challengeFor(body); } finally { state.lie = saved; }
        },
        close: () => new Promise((done) => { server.close(() => done()); server.closeAllConnections?.(); }),
      });
    });
  });
}
