/**
 * Local RemoteInject relay + test dApp.
 *
 * A ~one-file, zero-dependency stand-in for the remote-inject bridge. It speaks the
 * EXACT wire protocol the wallet's real `RemoteInjectTransport` expects (SSE for reads,
 * POST for writes — see src/services/dapp-transport.ts), so the wallet drives the whole
 * connect → request → approve → respond loop over its real transport. The relay also
 * serves a tiny, self-contained test dApp at `/` that fires each request type.
 *
 * There is no cryptography here: the relay is plaintext JSON bridging two roles
 * ("mobile" = wallet, "web" = dApp), which is exactly what makes it stable to test.
 *
 * CommonJS on purpose: runnable directly (`node e2e/support/relay.js [port]`) AND
 * importable by the Playwright specs (esbuild handles the CJS interop; an .mjs would
 * clash with the spec transform).
 */
const http = require('node:http');
const crypto = require('node:crypto');

const DEFAULT_PORT = 8788;

function makeState() {
  return { streams: { mobile: null, web: null }, buf: { mobile: [], web: [] } };
}

function startRelay({ port = DEFAULT_PORT } = {}) {
  /** sessionId -> { nonce, secret, metadata, ...state } */
  const sessions = new Map();
  const baseUrl = `http://localhost:${port}`;

  function newSession() {
    const id = crypto.randomBytes(6).toString('hex');
    const nonce = crypto.randomBytes(6).toString('hex');
    const secret = crypto.randomBytes(9).toString('hex');
    sessions.set(id, {
      nonce,
      secret,
      metadata: { name: 'Vela Test dApp', url: baseUrl, icon: `${baseUrl}/icon.svg` },
      ...makeState(),
    });
    return {
      sessionId: id, nonce, secret,
      connectUrl: `${baseUrl}/s/${id}?n=${nonce}&k=${secret}`,
      dappUrl: `${baseUrl}/?s=${id}&n=${nonce}&k=${secret}`,
    };
  }

  const auth = (s, q) => s && s.nonce === q.get('n') && s.secret === q.get('k');

  function cors(res) {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET,POST,OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  }

  function sseSend(res, obj) {
    try { res.write(`data: ${JSON.stringify(obj)}\n\n`); } catch { /* stream closed */ }
  }

  /** Deliver a message to the peer role, buffering it if the peer isn't connected yet. */
  function deliver(s, targetRole, msg) {
    const stream = s.streams[targetRole];
    if (stream) sseSend(stream, msg);
    else s.buf[targetRole].push(msg);
  }

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, baseUrl);
    const q = url.searchParams;

    if (req.method === 'OPTIONS') { cors(res); res.writeHead(204); res.end(); return; }

    // --- Test dApp page + assets ---
    if (req.method === 'GET' && (url.pathname === '/' || url.pathname === '/index.html')) {
      cors(res); res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(DAPP_HTML.split('__BASE__').join(baseUrl));
      return;
    }
    if (req.method === 'GET' && url.pathname === '/icon.svg') {
      cors(res); res.writeHead(200, { 'Content-Type': 'image/svg+xml' });
      res.end('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" rx="14" fill="#7c3aed"/><text x="32" y="42" font-size="30" text-anchor="middle" fill="#fff">🧪</text></svg>');
      return;
    }

    // --- Mint a fresh session (dApp side) ---
    if (req.method === 'POST' && url.pathname === '/session/new') {
      cors(res); res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(newSession()));
      return;
    }

    // --- Session metadata (wallet: fetchDAppInfo) ---
    const metaMatch = url.pathname.match(/^\/session\/([^/]+)$/);
    if (req.method === 'GET' && metaMatch) {
      const s = sessions.get(metaMatch[1]);
      cors(res);
      if (!auth(s, q)) { res.writeHead(403); res.end(); return; }
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ metadata: s.metadata }));
      return;
    }

    // --- SSE stream (both roles subscribe here) ---
    if (req.method === 'GET' && url.pathname === '/sse') {
      const s = sessions.get(q.get('session'));
      const role = q.get('role') === 'web' ? 'web' : 'mobile';
      if (!auth(s, q)) { cors(res); res.writeHead(403); res.end(); return; }
      res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        Connection: 'keep-alive',
        'Access-Control-Allow-Origin': '*',
      });
      res.write(':ok\n\n');
      s.streams[role] = res;
      sseSend(res, { type: 'ready' }); // wallet transitions to "connected" on first ready
      for (const m of s.buf[role].splice(0)) sseSend(res, m); // flush pre-connect messages
      req.on('close', () => { if (s.streams[role] === res) s.streams[role] = null; });
      return;
    }

    // --- POST message (wallet responses/info, or dApp requests) ---
    if (req.method === 'POST' && url.pathname === '/message') {
      const s = sessions.get(q.get('session'));
      const role = q.get('role') === 'web' ? 'web' : 'mobile';
      cors(res);
      if (!auth(s, q)) { res.writeHead(403); res.end(); return; }
      let body = '';
      req.on('data', (c) => { body += c; if (body.length > 1e6) req.destroy(); });
      req.on('end', () => {
        let msg; try { msg = JSON.parse(body); } catch { res.writeHead(400); res.end(); return; }
        deliver(s, role === 'mobile' ? 'web' : 'mobile', msg);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ ok: true }));
      });
      return;
    }

    cors(res); res.writeHead(404); res.end();
  });

  return new Promise((resolve) => {
    server.listen(port, '127.0.0.1', () => {
      resolve({
        port,
        baseUrl,
        newSession,
        sessions,
        // Force-close first: the SSE streams are keep-alive and never end on their
        // own, so a bare server.close() would hang waiting for them to drain.
        stop: () => new Promise((r) => {
          try { server.closeAllConnections?.(); } catch { /* older node */ }
          server.close(() => r());
        }),
      });
    });
  });
}

// ---------------------------------------------------------------------------
// The test dApp page (served at `/`)
// ---------------------------------------------------------------------------

const DAPP_HTML = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Vela Test dApp</title>
<style>
  :root { color-scheme: light dark; }
  body { font: 14px/1.5 -apple-system, system-ui, sans-serif; margin: 0; padding: 24px; max-width: 720px; }
  h1 { font-size: 18px; margin: 0 0 4px; }
  .muted { color: #888; }
  code { font-family: ui-monospace, Menlo, monospace; word-break: break-all; }
  .card { border: 1px solid #8883; border-radius: 12px; padding: 14px; margin: 14px 0; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; }
  button { font: inherit; padding: 8px 12px; border-radius: 8px; border: 1px solid #8886; background: #7c3aed; color: #fff; cursor: pointer; }
  button.sec { background: transparent; color: inherit; }
  #log { white-space: pre-wrap; font-family: ui-monospace, Menlo, monospace; font-size: 12px; max-height: 320px; overflow: auto; }
  .pill { display: inline-block; padding: 2px 8px; border-radius: 999px; font-size: 12px; font-weight: 700; }
  .on { background: #16a34a22; color: #16a34a; } .off { background: #ef444422; color: #ef4444; }
</style></head>
<body>
  <h1>🧪 Vela Test dApp</h1>
  <div class="muted">Talks to the wallet over the real RemoteInject relay. Parallel space only.</div>

  <div class="card">
    <div>Connect URL (paste into the wallet's Connect screen):</div>
    <code id="connect-url" data-testid="dapp-connect-url">…</code>
    <div style="margin-top:8px">
      Relay: <span id="relay" class="pill off" data-testid="dapp-relay-status">connecting…</span>
      Wallet: <span id="wallet" class="pill off" data-testid="dapp-wallet-status">not connected</span>
    </div>
    <div style="margin-top:8px">Account: <code id="addr" data-testid="dapp-wallet-address">—</code> · chain <code id="chain" data-testid="dapp-wallet-chain">—</code></div>
  </div>

  <div class="card">
    <div class="row" id="actions">
      <button data-testid="dapp-btn-accounts" onclick="fire('eth_requestAccounts', [])">eth_requestAccounts</button>
      <button data-testid="dapp-btn-chainid" class="sec" onclick="fire('eth_chainId', [])">eth_chainId</button>
      <button data-testid="dapp-btn-balance" class="sec" onclick="fire('eth_getBalance', [ADDR(), 'latest'])">eth_getBalance</button>
      <button data-testid="dapp-btn-personalsign" onclick="fire('personal_sign', [HEX('Hello from the Vela test dApp — sign to prove control.'), ADDR()])">personal_sign</button>
      <button data-testid="dapp-btn-typeddata" onclick="fire('eth_signTypedData_v4', [ADDR(), TYPED()])">signTypedData_v4</button>
      <button data-testid="dapp-btn-sendnative" onclick="fire('eth_sendTransaction', [{ from: ADDR(), to: '0x031d7D57c99CAF891e1C250554691Fd12D84772b', value: '0x5af3107a4000' }])">send 0.0001 xDAI</button>
      <button data-testid="dapp-btn-approve-limited" onclick="fire('eth_sendTransaction', [{ from: ADDR(), to: WXDAI, data: APPROVE('0x000000000022d473030f116ddee9f6b43ac78ba3', '0x0de0b6b3a7640000') }])">approve 1 WXDAI</button>
      <button data-testid="dapp-btn-approve-unlimited" onclick="fire('eth_sendTransaction', [{ from: ADDR(), to: WXDAI, data: APPROVE('0x000000000022d473030f116ddee9f6b43ac78ba3', 'f'.repeat(64)) }])">approve ∞ (should cap)</button>
      <button data-testid="dapp-btn-batch" onclick="fire('wallet_sendCalls', [{ version: '2.0.0', chainId: '0x64', from: ADDR(), calls: [{ to: '0x031d7D57c99CAF891e1C250554691Fd12D84772b', value: '0x2386f26fc10000' }, { to: '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d', value: '0x2386f26fc10000' }] }])">wallet_sendCalls (batch)</button>
      <button data-testid="dapp-btn-switch" class="sec" onclick="fire('wallet_switchEthereumChain', [{ chainId: '0x64' }])">switch → Gnosis</button>
      <button data-testid="dapp-btn-switch-bad" class="sec" onclick="fire('wallet_switchEthereumChain', [{ chainId: '0x270f' }])">switch → unsupported</button>
    </div>
  </div>

  <div class="card"><div id="log" data-testid="dapp-log"></div></div>

<script>
  const BASE = '__BASE__';
  const WXDAI = '0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d';
  const params = new URLSearchParams(location.search);
  let sess = null;
  let es = null;
  let counter = 0;
  const pending = new Map();
  window.__walletInfo = null;
  window.__log = [];

  function logline(s) { window.__log.push(s); const el = document.getElementById('log'); el.textContent = window.__log.join('\\n'); el.scrollTop = el.scrollHeight; }
  function setPill(id, on, text) { const el = document.getElementById(id); el.className = 'pill ' + (on ? 'on' : 'off'); el.textContent = text; }
  function ADDR() { return (window.__walletInfo && window.__walletInfo.address) || '0x0000000000000000000000000000000000000000'; }
  function HEX(s) { return '0x' + Array.from(new TextEncoder().encode(s)).map(b => b.toString(16).padStart(2, '0')).join(''); }
  function APPROVE(spender, amountHex) { return '0x095ea7b3' + spender.replace(/^0x/, '').toLowerCase().padStart(64, '0') + amountHex.replace(/^0x/, '').padStart(64, '0'); }
  function TYPED() {
    return JSON.stringify({
      types: { EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }], Mail: [{ name: 'contents', type: 'string' }] },
      primaryType: 'Mail', domain: { name: 'Vela Test dApp', chainId: 100 }, message: { contents: 'gm from the test dApp' },
    });
  }

  window.fire = function fire(method, prms) {
    const id = 'req-' + (++counter);
    const p = new Promise((resolve) => pending.set(id, resolve));
    logline('→ ' + method + '  ' + JSON.stringify(prms));
    fetch(BASE + '/message?session=' + sess.sessionId + '&role=web&n=' + sess.nonce + '&k=' + sess.secret, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ type: 'request', id, method, params: prms, origin: location.origin }),
    });
    return p;
  };
  window.getLog = () => window.__log.slice();

  function openStream() {
    es = new EventSource(BASE + '/sse?session=' + sess.sessionId + '&role=web&n=' + sess.nonce + '&k=' + sess.secret);
    es.onmessage = (ev) => {
      let m; try { m = JSON.parse(ev.data); } catch { return; }
      if (m.type === 'ready') { setPill('relay', true, 'connected'); return; }
      if (m.type === 'connect') {
        window.__walletInfo = { address: m.address, chainId: m.chainId };
        setPill('wallet', true, 'connected');
        document.getElementById('addr').textContent = m.address;
        document.getElementById('chain').textContent = m.chainId;
        logline('● wallet connected: ' + m.address + ' chain ' + m.chainId);
        return;
      }
      if (m.type === 'response') {
        const out = m.error ? ('ERROR ' + m.error.code + ' ' + m.error.message) : JSON.stringify(m.result);
        logline('← ' + m.id + '  ' + out);
        const r = pending.get(m.id); if (r) { pending.delete(m.id); r(m); }
        window.__lastResponse = m;
        return;
      }
      if (m.type === 'disconnect') { setPill('wallet', false, 'disconnected'); window.__walletInfo = null; }
    };
    es.onerror = () => setPill('relay', false, 'relay error');
  }

  async function boot() {
    if (params.get('s') && params.get('n') && params.get('k')) {
      sess = { sessionId: params.get('s'), nonce: params.get('n'), secret: params.get('k') };
      sess.connectUrl = BASE + '/s/' + sess.sessionId + '?n=' + sess.nonce + '&k=' + sess.secret;
    } else {
      sess = await (await fetch(BASE + '/session/new', { method: 'POST' })).json();
    }
    window.__connectUrl = sess.connectUrl;
    document.getElementById('connect-url').textContent = sess.connectUrl;
    openStream();
  }
  boot();
</script>
</body></html>`;

module.exports = { startRelay };

// Run standalone: `node e2e/support/relay.js [port]`
if (require.main === module) {
  const port = Number(process.argv[2]) || DEFAULT_PORT;
  startRelay({ port }).then((r) => {
    const s = r.newSession();
    console.log(`\n  Vela test relay + dApp on ${r.baseUrl}`);
    console.log(`  ─ test dApp:   ${r.baseUrl}/`);
    console.log(`  ─ connect URL: ${s.connectUrl}`);
    console.log(`    (paste it into the wallet's /parallel/connect screen)\n`);
  });
}
