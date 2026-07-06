// Transparent WalletPair relay PROXY for BUG-5 debugging.
//
// Sits between the clients (the Node dApp peer AND the app's RN wallet) and the REAL
// relay, forwarding every frame verbatim while LOGGING both directions + all closes.
// This shows exactly what the app's RN WebSocket sends/receives around the moment the
// relay closes the peer — the last-hop difference vs the clean Node WalletSession.
//
// Usage:  node e2e/safari/wp_proxy.mjs        (listens ws://0.0.0.0:9999)
// Point the peer at it:  WP_RELAY=ws://<mac-lan-ip>:9999
// (the peer bakes that into the pairing URI, so the app connects here too).
//
// Plain ws:// to the clients (the DEV build already talks to http://<lan> for Metro +
// testdapp, so ATS allows it); TLS wss:// upstream to the real relay.
import WebSocket from 'ws';

const REAL = process.env.WP_UPSTREAM || 'wss://relay.walletpair.org/v1';
const PORT = Number(process.env.WP_PROXY_PORT || 9999);
const start = Date.now();
const ts = () => ((Date.now() - start) / 1000).toFixed(2) + 's';
const FULL = process.env.WP_FULL === '1';
const short = (s) => (FULL || s.length <= 260 ? s : s.slice(0, 260) + '…(' + s.length + ')');
// tag a connection by the first frame it sends: create=dApp/peer, join=wallet/app
const roleOf = (s) => (s.includes('"t":"create"') ? 'PEER' : s.includes('"t":"join"') ? 'APP ' : '????');

const wss = new WebSocket.Server({ host: '0.0.0.0', port: PORT });
let cid = 0;

wss.on('connection', (client, req) => {
  const id = ++cid;
  const path = req.url || '';
  // Forward only the QUERY (?ch=…) onto REAL's own path — the client hits us at
  // "/?ch=…", and REAL already ends in "/v1", so REAL+path would be ".../v1/?ch=…"
  // (extra slash → the relay 404s). Correct is ".../v1?ch=…".
  const q = new URL(path, 'ws://x').search;
  const upstreamUrl = REAL + q;
  console.log(`${ts()} [${id}] CLIENT connected  url=${path}  hdrs.origin=${req.headers.origin || '-'} ua=${(req.headers['user-agent'] || '-').slice(0, 40)}`);
  const upstream = new WebSocket(upstreamUrl);
  const queue = [];
  let role = '????';

  upstream.on('open', () => {
    console.log(`${ts()} [${id}] UPSTREAM open → ${REAL}${path}`);
    for (const m of queue) upstream.send(m);
    queue.length = 0;
  });
  client.on('message', (d) => {
    const s = typeof d === 'string' ? d : d.toString();
    if (role === '????') role = roleOf(s);
    console.log(`${ts()} [${id}:${role}] C→R ${short(s)}`);
    if (upstream.readyState === WebSocket.OPEN) upstream.send(d);
    else queue.push(d);
  });
  upstream.on('message', (d) => {
    const s = typeof d === 'string' ? d : d.toString();
    console.log(`${ts()} [${id}:${role}] R→C ${short(s)}`);
    if (client.readyState === WebSocket.OPEN) client.send(d);
  });
  client.on('close', (code, reason) => {
    console.log(`${ts()} [${id}] CLIENT close code=${code} reason=${(reason || '').toString().slice(0, 60)}`);
    try { if (upstream.readyState <= WebSocket.OPEN) upstream.close(); } catch {}
  });
  upstream.on('close', (code, reason) => {
    console.log(`${ts()} [${id}] *** UPSTREAM(relay) close code=${code} reason=${(reason || '').toString().slice(0, 60)}`);
    try {
      const safe = code >= 1000 && code <= 4999 ? code : 1000;
      if (client.readyState <= WebSocket.OPEN) client.close(safe, reason);
    } catch {}
  });
  upstream.on('error', (e) => console.log(`${ts()} [${id}] upstream ERR ${e.message}`));
  client.on('error', (e) => console.log(`${ts()} [${id}] client ERR ${e.message}`));
});

wss.on('listening', () => console.log(`${ts()} WP proxy listening ws://0.0.0.0:${PORT} → ${REAL}`));
wss.on('error', (e) => console.log(`${ts()} proxy server ERR ${e.message}`));
