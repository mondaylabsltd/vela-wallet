// WalletPair dApp peer for the concurrent-session device proof (F2/F3 two-slot).
//
// Stands in for a live dApp holding a WalletPair session while the Safari extension
// runs a concurrent sign. It:
//   - creates a WalletPair pairing (real relay) and prints the URI + fingerprint,
//   - auto-accepts when the Vela wallet joins,
//   - RECORDS every response/event it receives — if the concurrent extension
//     signature ever lands here, that is the F2 leak we are proving CANNOT happen,
//   - answers stdin commands: `status` (JSON: phase + everything received), `close`.
//
// Run from the repo root so `walletpair-sdk` resolves:
//   node e2e/safari/wp_peer.mjs
// stdout = newline-delimited JSON events (the Python harness reads these);
// stderr = human logs.
import { DAppSession, WebSocketTransport, setDisconnectLogSink, setWalletpairDebugLogging } from 'walletpair-sdk';

const RELAY = process.env.WP_RELAY || 'wss://relay.walletpair.org/v1';
if (process.env.WP_DEBUG) {
  try { setWalletpairDebugLogging(true); } catch { /* older sdk */ }
  try { setDisconnectLogSink((e) => console.error('[wp_peer][disconnect]', JSON.stringify(e))); } catch { /* older sdk */ }
}
const log = (...a) => console.error('[wp_peer]', ...a);
const out = (obj) => process.stdout.write(JSON.stringify(obj) + '\n');

// Everything the peer receives from the wallet. MUST stay empty of the extension's
// signature — a non-empty leak here (the ext sig delivered over the WP socket) is
// the exact F2 fund-safety violation the two-slot design prevents.
const received = [];
let phase = 'idle';

const transport = new WebSocketTransport(RELAY);
const session = new DAppSession({
  transport,
  meta: {
    name: 'Vela Concurrency Peer',
    description: 'e2e concurrent-session proof dApp',
    url: 'https://wp-peer.test',
    icon: 'https://wp-peer.test/icon.png',
  },
  // Declare WalletPair-native method names (the app's wallet declares wallet_* per
  // buildCapabilities, NOT the EVM aliases personal_sign/eth_sendTransaction) — a dApp
  // that declares a method the wallet doesn't list is rejected 'unsupported_capability'.
  // Keep this a SUBSET of what the app declares.
  methods: ['wallet_signMessage', 'wallet_signTypedData', 'wallet_sendTransaction'],
  chains: ['eip155:1', 'eip155:100', 'eip155:8453'],
});

session.on('phase', (p) => { phase = p; log('phase →', p); out({ type: 'phase', phase: p }); });
session.on('sessionFingerprint', (fp) => { log('fingerprint', fp); out({ type: 'fingerprint', fingerprint: fp }); });
session.on('walletJoined', (info) => {
  log('walletJoined:', info && info.meta && info.meta.name);
  try { session.acceptWallet(); } catch (e) { log('acceptWallet err', e && e.message); }
  out({ type: 'walletJoined', wallet: (info && info.meta && info.meta.name) || null });
});
session.on('response', (r) => { received.push({ kind: 'response', data: r }); log('*** received response', JSON.stringify(r).slice(0, 160)); });
session.on('event', (e) => { received.push({ kind: 'event', data: e }); log('*** received event', JSON.stringify(e).slice(0, 160)); });
session.on('reconnectExhausted', () => { log('reconnectExhausted'); out({ type: 'reconnectExhausted' }); });

try {
  const uri = await session.createPairing();
  log('pairing created; fingerprint', session.sessionFingerprint);
  out({ type: 'uri', uri, fingerprint: session.sessionFingerprint });
} catch (e) {
  log('createPairing FAILED', e && e.message);
  out({ type: 'error', error: String(e && e.message ? e.message : e) });
  process.exit(1);
}

// stdin command loop (newline-delimited).
process.stdin.setEncoding('utf8');
let buf = '';
process.stdin.on('data', (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i).trim();
    buf = buf.slice(i + 1);
    if (!line) continue;
    if (line === 'status') {
      out({ type: 'status', phase, receivedCount: received.length, received });
    } else if (line === 'close') {
      try { session.close('test-done'); } catch { /* ignore */ }
      out({ type: 'closed' });
      process.exit(0);
    } else {
      log('unknown cmd', line);
    }
  }
});

// Keep alive.
setInterval(() => {}, 1 << 30);
