#!/usr/bin/env python3
# COMPREHENSIVE on-device test for the in-app dApp browser (WalletWebView):
# functionality + injection + interaction + visuals, end-to-end on a real device.
#
# A Release WKWebView is not inspectable, so the test dApp BEACONS each stage back
# to a local server the device reaches. The dApp chains a real dApp session:
#   eth_chainId  -> eth_requestAccounts (connect sheet) -> personal_sign (sign sheet)
# and Appium taps Connect + Approve (the parallel-space mock passkey signs without
# biometrics). Screenshots are saved per stage for the visual/interaction record.
#
# Requires a build WITH the WalletWebView native module (remember: after editing
# native code, cp modules/*/ios/* -> ios/VelaWallet/ before `expo run:ios` — the
# config-plugin copy is skipped when ios/ exists) + an onboarded account (the
# parallel space provides the fixture). Env: VELA_UDID / VELA_APP_BID (lib.py).
import os, time, socket, threading, urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from lib import mk, L, VELA, nsrc, center, tap

REPORTS = {}

DAPP_HTML = b"""<!doctype html><html><head><meta name=viewport content="width=device-width,initial-scale=1"></head>
<body style="font-family:-apple-system;padding:16px"><h1>vela walletwebview test</h1><pre id=out>booting</pre>
<script>
function beacon(k,v){ try{ new Image().src='/report?'+k+'='+encodeURIComponent(v)+'&t='+Date.now(); }catch(e){} }
function setout(s){ var o=document.getElementById('out'); if(o) o.textContent=s; }
beacon('loaded','1');
window.__6963=[];
addEventListener('eip6963:announceProvider', function(e){ if(e&&e.detail&&e.detail.info){ window.__6963.push(e.detail.info.rdns); beacon('eip6963', e.detail.info.rdns); } });
dispatchEvent(new Event('eip6963:requestProvider'));
function go(){
  if(!window.ethereum){ beacon('provider','none'); setout('no window.ethereum'); return; }
  beacon('provider', window.ethereum.isVela ? 'vela' : 'other'); beacon('sent','1');
  var acct='';
  window.ethereum.request({method:'eth_chainId'}).then(function(cid){
    beacon('chainId', cid); setout('chainId='+cid);
    beacon('connectcalled','1');
    return window.ethereum.request({method:'eth_requestAccounts'});   // -> connect sheet
  }).then(function(a){
    acct=(a&&a[0])||''; beacon('accounts', acct||'[]'); setout('accounts='+a);
    beacon('signcalled','1');
    return window.ethereum.request({method:'personal_sign', params:['0x48656c6c6f2056656c61', acct]}); // -> sign sheet
  }).then(function(sig){
    beacon('signature', String(sig).slice(0,24)); setout('signed='+String(sig).slice(0,18));
  }).catch(function(e){ beacon('flowerr', (e&&e.code)+':'+String((e&&e.message)||'').slice(0,40)); setout('err '+(e&&e.message)); });
}
if(window.ethereum) go(); else { setout('waiting for provider'); addEventListener('ethereum#initialized', go); setTimeout(go, 2500); }
</script></body></html>"""


def lan_ip():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(('8.8.8.8', 80)); return s.getsockname()[0]
    except Exception:
        return '127.0.0.1'
    finally:
        s.close()


def serve():
    class H(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path.startswith('/report'):
                q = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
                for k, v in q.items():
                    if k != 't':
                        REPORTS[k] = v[0]; L('  beacon:', k, '=', v[0])
                self.send_response(204); self.end_headers(); return
            self.send_response(200)
            self.send_header('content-type', 'text/html; charset=utf-8')
            self.end_headers(); self.wfile.write(DAPP_HTML)
        def log_message(self, *a):
            pass
    srv = ThreadingHTTPServer(('0.0.0.0', 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def wait_for(keys, secs):
    end = time.time() + secs
    while time.time() < end:
        if set(REPORTS) & set(keys):
            return True
        time.sleep(0.5)
    return False


def shot(d, path):
    try:
        d.get_screenshot_as_file(path); L('screenshot', path)
    except Exception as e:
        L('screenshot failed', repr(e)[:100])


def tap_label(d, label):
    try:
        c = center(nsrc(d), label)
        if c:
            tap(d, *c); time.sleep(1.4); return True
    except Exception as e:
        L('tap', label, 'failed', repr(e)[:100])
    return False


def main():
    srv, port = serve()
    url = f'http://{lan_ip()}:{port}/'
    L('test dApp URL:', url)
    d = mk(safari=False)
    try:
        deeplink = 'velawallet://browser?url=' + urllib.parse.quote(url, safe='')
        L('deep link:', deeplink)
        try:
            d.execute_script('mobile: deepLink', {'url': deeplink, 'bundleId': VELA})
        except Exception as e:
            L('deepLink error:', repr(e)[:160])
        time.sleep(2)
        try:
            d.execute_script('mobile: activateApp', {'bundleId': VELA})
        except Exception:
            pass

        # Phase 1 — injection + chainId round-trip + the connect-consent sheet.
        wait_for(['connectcalled', 'flowerr'], 25)
        time.sleep(1.2)
        shot(d, '/tmp/wv_1_connect.png')
        # The Connect button label follows the app language — match every shipped
        # locale (连接 = zh, 連接 = zh-Hant, 接続 = ja, plus English).
        L('tap Connect:', tap_label(d, 'Connect') or tap_label(d, '连接')
          or tap_label(d, '連接') or tap_label(d, '接続'))

        # Phase 2 — accounts granted + personal_sign brings up the signing sheet.
        wait_for(['signcalled', 'accounts', 'flowerr'], 15)
        time.sleep(1.8)
        shot(d, '/tmp/wv_2_sign.png')
        # The Sign button label follows the DEVICE language — match every locale we
        # ship (署名 = ja, 簽署/簽名 = zh-Hant, 签名 = zh, plus English).
        approved = (tap_label(d, '签名') or tap_label(d, '署名') or tap_label(d, '簽署')
                    or tap_label(d, '簽名') or tap_label(d, 'Sign') or tap_label(d, 'Approve'))
        L('tap Approve:', approved)

        # Phase 3 — signature returns to the dApp.
        wait_for(['signature', 'flowerr'], 25)
        time.sleep(1.0)
        shot(d, '/tmp/wv_3_done.png')

        L('reports:', dict(REPORTS))
        R = REPORTS
        stages = [
            ('loaded (renders + loads page)', R.get('loaded') == '1'),
            ('injected (window.ethereum isVela)', R.get('provider') == 'vela'),
            ('eip6963 (app.getvela)', 'app.getvela' in R.get('eip6963', '')),
            ('eth_chainId round-trip', str(R.get('chainId', '')).startswith('0x')),
            ('connect -> accounts granted', bool(R.get('accounts') and R.get('accounts') != '[]')),
            ('personal_sign signature', bool(R.get('signature'))),
        ]
        L('=== on-device results ===')
        for name, ok in stages:
            L(('  PASS ' if ok else '  ---- '), name, R.get(name, ''))
        # The exit gate covers the WHOLE claimed flow — inject + round-trip AND the
        # two interactions this PR is actually about (connect sheet → accounts, and
        # personal_sign → signature). The parallel-space mock passkey signs without
        # biometrics, so both are deterministic here. Set VELA_SKIP_SIGN=1 to relax
        # the signature stage when running against a build without a registered key.
        require_sign = os.environ.get('VELA_SKIP_SIGN') != '1'
        gate = stages if require_sign else stages[:5]
        passed = all(ok for _name, ok in gate)
        L('GATE', '(inject/6963/round-trip/connect' + ('/sign)' if require_sign else ')'),
          ':', 'PASS ✅' if passed else 'FAIL ❌',
          '| connect:', 'ok' if stages[4][1] else 'FAIL',
          '| sign:', ('ok' if stages[5][1] else ('skip' if not require_sign else 'FAIL')),
          '| flowerr:', R.get('flowerr', '-'))
        return 0 if passed else 1
    finally:
        try: d.quit()
        except Exception: pass
        srv.shutdown()


if __name__ == '__main__':
    raise SystemExit(main())
