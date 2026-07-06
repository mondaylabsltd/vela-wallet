#!/usr/bin/env python3
# Concurrent-session DEVICE proof (the two-slot raison d'être — F2/F3).
#
# Establishes a REAL WalletPair session (a Node dApp peer + the app's Connect UI,
# over the real relay), THEN runs a REAL Safari-extension sign while that session is
# live — WITHOUT killing the app — and proves on hardware that:
#   • the WalletPair session SURVIVES the extension sign (durable slot untouched by
#     beginExtensionSign), and
#   • the extension signature NEVER reaches the WalletPair peer (no F2 leak).
#
# Companion to the headless routing proof (src/__tests__/concurrent-session.test.ts).
#
# Prereq: a DEV build on the device + Appium (see README). The app is armed into the
# parallel space by a COLD `velawallet://parallel/connect` deep link (fixed-key signer,
# no Face ID), which ALSO lands on the Connect screen ready for the pairing URI.
import os, time, json, subprocess, threading
import xml.etree.ElementTree as ET
from appium.webdriver.common.appiumby import AppiumBy
from lib import (mk, L, webctx, nsrc, center, tap, provider_ready,
                 wait_dapp_result, sheet_shadow_click, URL, SAFARI, VELA)

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
CONFIRM = ['确认', 'Confirm']
APPROVE = ['签名', 'Sign', '批准', 'Approve']


class Peer:
    """The Node WalletPair dApp peer subprocess. Parses its newline-JSON events and
    sends it commands. `received` staying empty is the F2 no-leak assertion."""
    def __init__(self):
        self.proc = subprocess.Popen(
            ['node', 'e2e/safari/wp_peer.mjs'], cwd=REPO_ROOT,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1)
        self.events = []
        self.uri = None
        self.fingerprint = None
        threading.Thread(target=self._read_out, daemon=True).start()
        threading.Thread(target=self._drain_err, daemon=True).start()

    def _read_out(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except Exception:
                continue
            self.events.append(ev)
            if ev.get('type') == 'uri':
                self.uri = ev['uri']
                self.fingerprint = ev.get('fingerprint')

    def _drain_err(self):
        for line in self.proc.stderr:
            L('[peer]', line.rstrip())  # surface the peer's own view (join/phase/errors)

    def wait_uri(self, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            if self.uri:
                return self.uri
            time.sleep(0.2)
        return None

    def wait_event(self, typ, timeout=30):
        end = time.time() + timeout
        while time.time() < end:
            if any(e.get('type') == typ for e in self.events):
                return True
            time.sleep(0.3)
        return False

    def phase(self):
        ph = [e['phase'] for e in self.events if e.get('type') == 'phase']
        return ph[-1] if ph else None

    def _cmd(self, c):
        try:
            self.proc.stdin.write(c + '\n')
            self.proc.stdin.flush()
        except Exception:
            pass

    def status(self, timeout=6):
        n = len(self.events)
        self._cmd('status')
        end = time.time() + timeout
        while time.time() < end:
            for e in self.events[n:]:
                if e.get('type') == 'status':
                    return e
            time.sleep(0.2)
        return None

    def close(self):
        self._cmd('close')
        time.sleep(0.4)
        try:
            self.proc.terminate()
        except Exception:
            pass


def navigate_to_connect(d):
    """Launch the app into the parallel space and open the Connect screen (WITHOUT a
    peer/URI yet). Launch pattern: terminate + activate_app (the dev-client launcher
    auto-loads the last Metro bundle → parallel home; a cold `deepLink` lands on the
    launcher UI and hangs). THEN a WARM `deepLink velawallet://parallel/connect` routes
    to the Connect screen. Returns True once its TextField is present."""
    try:
        d.terminate_app(VELA)
    except Exception:
        pass
    time.sleep(1.5)
    d.activate_app(VELA)
    L('activate_app → waiting for the parallel home (fixed-key signer)…')
    for _ in range(20):
        time.sleep(2)
        src = nsrc(d)
        if center(src, '账户 Parallel One。切换账户') or any(
            (el.attrib.get('label') or '').startswith('账户 Parallel')
            for el in ET.fromstring(src).iter()
        ):
            L('parallel home loaded')
            break
    else:
        L('[WARN] parallel home not detected — the sign may hit Face ID')
    d.execute_script('mobile: deepLink', {'url': 'velawallet://parallel/connect', 'bundleId': VELA})
    L('warm deepLink → /parallel/connect')
    for _ in range(12):
        time.sleep(1.5)
        d.switch_to.context('NATIVE_APP')
        if d.find_elements(AppiumBy.IOS_PREDICATE,
                           "type == 'XCUIElementTypeTextField' OR type == 'XCUIElementTypeSecureTextField'"):
            L('Connect screen ready')
            return True
    L('[FAIL] Connect screen TextField never appeared')
    return False


def type_uri(d, uri):
    """Type the pairing URI into the Connect screen's TextField and submit. Called
    IMMEDIATELY after the peer creates the pairing — the CF-Worker relay drops an idle
    channel WS within ~tens of seconds, so the wallet must join promptly or the relay
    reports `peer_closed` and the session never reaches 'connected'."""
    d.switch_to.context('NATIVE_APP')
    fields = d.find_elements(AppiumBy.IOS_PREDICATE,
                             "type == 'XCUIElementTypeTextField' OR type == 'XCUIElementTypeSecureTextField'")
    if not fields:
        L('[FAIL] TextField vanished before typing')
        return False
    f = fields[0]
    f.click()
    time.sleep(0.5)
    f.send_keys(uri)
    time.sleep(0.6)
    f.send_keys('\n')  # returnKeyType=go → onSubmitEditing → handlePasteConnect
    L('entered pairing URI + submitted')
    return True


def confirm_fingerprint(d, peer):
    """Wait for the fingerprint-verify card, sanity-check it matches the peer's, tap Confirm."""
    for _ in range(15):
        time.sleep(1)
        src = nsrc(d)
        # the 4 fingerprint digits render as separate boxes; the peer's fingerprint
        # should appear somewhere on screen (visual match the user would make).
        if peer.fingerprint and peer.fingerprint in src.replace(' ', ''):
            L('fingerprint on screen matches peer:', peer.fingerprint)
        xy = None
        for lab in CONFIRM:
            xy = center(src, lab)
            if xy:
                break
        if xy:
            tap(d, *xy)
            L('tapped Confirm')
            return True
    L('[FAIL] fingerprint Confirm button never appeared')
    return False


def ext_sign(d):
    """Run ONE real Safari-extension personal_sign, WARM (the app + its WalletPair
    session stay alive). Returns the dApp's real signature hex or None."""
    d.activate_app(SAFARI)
    time.sleep(2)
    loaded = False
    for _ in range(6):
        try:
            webctx(d)
            d.get(URL)
            time.sleep(2)
            if d.execute_script("return !!document.getElementById('btn-connect');"):
                loaded = True
                break
        except Exception:
            d.activate_app(SAFARI)
            time.sleep(1.5)
    if not loaded or not provider_ready(d):
        L('ext_sign: provider/page not ready')
        return None
    # connect (in-Safari)
    d.execute_script("document.getElementById('btn-connect').click();")
    time.sleep(1.2)
    sheet_shadow_click(d, 'cta')
    wait_dapp_result(d, 'eth_requestAccounts')
    # personal_sign → write sign-req → WARM deepLink /sign (app already running)
    d.execute_script("document.getElementById('btn-sign').click();")
    rid = None
    for _ in range(12):
        time.sleep(1)
        rid = d.execute_script(
            "var e=document.getElementById('vela-r1-sign-status');return e?e.dataset.rid:null;")
        if rid:
            break
    L('ext_sign: rid =', rid)
    sheet_shadow_click(d, 'cta')  # onSignLaunch → writeSignRequest
    time.sleep(1.5)
    if rid:
        d.execute_script('mobile: deepLink', {'url': 'velawallet://sign?rid=' + rid, 'bundleId': VELA})
        L('ext_sign: WARM deepLink /sign (WalletPair session stays alive)')
    time.sleep(3)
    # tap the real SigningSheet approve
    xy = None
    for _ in range(24):
        src = nsrc(d)
        best = None
        for el in ET.fromstring(src).iter():
            a = el.attrib
            if (a.get('label') or a.get('name')) in APPROVE:
                try:
                    y = int(a['y']) + int(a['height']) // 2
                    x = int(a['x']) + int(a['width']) // 2
                    if best is None or y > best[0]:
                        best = (y, (x, y))
                except Exception:
                    pass
        if best:
            xy = best[1]
            break
        time.sleep(0.6)
    if not xy:
        L('ext_sign: SigningSheet approve not found')
        return None
    tap(d, *xy)
    L('ext_sign: tapped approve (fixed-key sign)')
    time.sleep(2.5)
    d.activate_app(SAFARI)
    time.sleep(3)
    webctx(d)
    rr = wait_dapp_result(d, 'personal_sign', tries=25)
    val = (rr or {}).get('value')
    return val if (val and isinstance(val, str) and val.startswith('0x') and not val.startswith('0xFA') and len(val) > 20) else None


def main():
    d = mk(safari=True)
    peer = None
    steps = {'wp_connected': False, 'ext_real_sig': False, 'wp_survived': False, 'no_leak': False}
    try:
        # 1. get the app to the Connect screen FIRST (no peer yet) so the pairing WS
        #    isn't left idle on the relay during the ~40s app cold-launch (the CF-Worker
        #    relay drops an idle channel → the wallet's join lands on a dead peer).
        if not navigate_to_connect(d):
            raise SystemExit
        # 2. NOW create the peer + enter the URI immediately (minimal relay idle).
        peer = Peer()
        uri = peer.wait_uri()
        if not uri:
            L('[FAIL] WalletPair peer produced no pairing URI (relay unreachable?)')
            raise SystemExit
        L('peer pairing URI ready; fingerprint', peer.fingerprint)
        if not type_uri(d, uri):
            raise SystemExit
        if not confirm_fingerprint(d, peer):
            raise SystemExit
        # 3. wait for the session to go live on BOTH ends.
        joined = peer.wait_event('walletJoined', timeout=40)
        time.sleep(2)
        ph = peer.phase()
        steps['wp_connected'] = bool(joined and ph == 'connected')
        L('WalletPair: walletJoined=%s peer.phase=%s → connected? %s' % (joined, ph, steps['wp_connected']))

        # 3. run a REAL extension sign while the WP session is live (warm — no app kill).
        sig = ext_sign(d)
        steps['ext_real_sig'] = bool(sig)
        L('extension sign → real EIP-1271 signature?', bool(sig), (str(sig)[:24] + '…') if sig else '')

        # 4. the verdicts: WP survived + never received the ext signature.
        st = peer.status()
        if st:
            steps['wp_survived'] = (st.get('phase') == 'connected')
            steps['no_leak'] = (st.get('receivedCount', 1) == 0)
            L('peer AFTER the ext sign: phase=%s receivedCount=%s' % (st.get('phase'), st.get('receivedCount')))
            if st.get('receivedCount'):
                L('!! LEAK — the WP peer received:', json.dumps(st.get('received'))[:300])
    finally:
        try:
            d.quit()
        except Exception:
            pass
        if peer:
            peer.close()

    L('==================== CONCURRENT-SESSION DEVICE PROOF ====================')
    for k in ('wp_connected', 'ext_real_sig', 'wp_survived', 'no_leak'):
        L(('  [PASS] ' if steps[k] else '  [FAIL] ') + k)
    passed = sum(1 for v in steps.values() if v)
    L('RESULT: %d/4 (WalletPair live + extension real-sig + WP survived + no cross-answer leak)' % passed)


if __name__ == '__main__':
    main()
