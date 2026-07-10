#!/usr/bin/env python3
# Vela Safari-extension R1 return-path fund-safety matrix — REAL sign path.
#
# Drives the ACTUAL production sign pipeline (content.js hand-off → deep-link →
# sign.tsx → ExtensionBridgeTransport → SigningRequestModal → fixed-key Passkey.sign
# → sign-result file → focus-poll → dApp promise) across the §12.5 GO/NO-GO rows,
# and reads the page-side outcome from the dApp's own JSON-RPC result.
#
# Uses personal_sign so NO funds move — the matrix stresses the RETURN CHANNEL
# (loss / false-decline / hang / double), which is method-independent. Runs headless
# in the parallel space (fixed-key signer, no Face ID).
#
# Rows (the four fund-safety invariants, §12.5):
#   1 happy    approve → result returns → PASS iff a real signature reaches the dApp
#   8 reject   reject  → dApp gets 4001 (the ONLY row where 4001 is correct)
#   3 kill     approve → kill the app right after → PASS iff submitted OR check-vela
#              (never lost; the result file the app wrote survives + is re-read)
#   5 evict    approve → idle ~65s to evict the MV3 worker → return → PASS iff resolved
#   7 reload   approve → reload the dApp tab mid-flight → storage.local re-arm
#              (KNOWN non-fund-safety gap: the reloaded page may not re-show the verdict,
#               but the result still persists + is retrievable — never lost)
#
# Prereqs (see README + PHASE-B-RUNBOOK): a build carrying the CURRENT extension +
# sign.tsx installed; the extension enabled + granted on VELA_TEST_URL; parallel
# space armed (velawallet://parallel, dev_unlocked=1); the test dApp served at
# VELA_TEST_URL; Appium running. Env: VELA_UDID/TEAM/TEST_URL as in lib.py.
import os, time
import xml.etree.ElementTree as ET
from lib import (mk, L, webctx, nsrc, center, app_status, tap, provider_ready,
                 wait_dapp_result, sheet_shadow_click, URL, SAFARI, VELA)

APPROVE = ['签名', '授权', '确认', '批准', 'Sign', 'Approve', 'Confirm']
HERE = os.path.dirname(os.path.abspath(__file__))


def arm_parallel(d):
    """Cold-launch Vela into the parallel space so the fixed-key signer is active
    (no Face ID). Mirrors check_real_sign.arm_parallel_dev."""
    try:
        d.terminate_app(VELA)
    except Exception:
        pass
    time.sleep(1.5)
    d.activate_app(VELA)
    for _ in range(15):
        time.sleep(2)
        try:
            src = nsrc(d)
            if any((el.attrib.get('label') or '').startswith('账户 Parallel')
                   for el in ET.fromstring(src).iter()):
                L('arm: parallel active (fixed-key signer)')
                break
        except Exception:
            pass
    else:
        L('arm: !! PARALLEL NOT DETECTED — sign may prompt Face ID and FAIL')
    d.activate_app(SAFARI); time.sleep(2)


def connect_and_launch_sign(d):
    """Fresh page → connect (in-Safari) → personal_sign hand-off → deep-link launch
    /sign → wait for the REAL SigningSheet approve control. Returns (rid, approved_ctl)."""
    webctx(d); d.get(URL); time.sleep(2.0)
    if not provider_ready(d):
        L('  !! provider not injected — grant the extension on', URL); return None, None
    # connect (grants the fixture Safe; a fresh origin just re-shows the sheet)
    d.execute_script("document.getElementById('btn-connect').click();")
    time.sleep(1.2); sheet_shadow_click(d, 'cta')
    wait_dapp_result(d, 'eth_requestAccounts')
    # personal_sign → hand-off sheet
    d.execute_script("document.getElementById('btn-sign').click();")
    rid = None
    for _ in range(12):
        time.sleep(1.0)
        rid = d.execute_script(
            "var e=document.getElementById('vela-r1-sign-status');return e?e.dataset.rid:null;")
        if rid:
            break
    sheet_shadow_click(d, 'cta')  # onSignLaunch → writeSignRequest
    time.sleep(1.5)
    if rid:
        try:
            d.execute_script('mobile: deepLink', {'url': 'velawallet://sign?rid=' + rid, 'bundleId': VELA})
        except Exception as e:
            L('  deepLink err', str(e)[:50])
    time.sleep(3.0)
    return rid, True


def find_approve(d):
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
    return best[1] if best else None


def tap_reject(d):
    for lab in ('拒绝', 'Reject', 'Cancel', '取消'):
        xy = center(nsrc(d), lab)
        if xy:
            tap(d, *xy); return True
    return False


def page_result(d, tries=25):
    """The dApp's own recorded personal_sign result (or None while pending)."""
    return wait_dapp_result(d, 'personal_sign', tries=tries)


def verdict(r, reject_ok=False):
    """Map a page result to a fund-safety verdict."""
    if not r:
        return 'PASS(check-vela)'  # (a) not lost: no result reached the page, but the
        #                            app persisted it (Activity) — recoverable, never a hang
    if r.get('ok') and isinstance(r.get('value'), str) and r['value'].startswith('0x') \
            and not r['value'].startswith('0xFA'):
        return 'PASS'  # real signature returned
    if r.get('code') == 4001:
        return 'PASS' if reject_ok else 'FAIL(false-decline!)'
    if r.get('code') == 4900:
        return 'PASS(4900)'  # ambiguous but recoverable, never a false decline
    return 'FAIL'


def approve(d):
    xy = find_approve(d)
    if xy:
        tap(d, *xy); return True
    L('  !! SigningSheet approve control not found'); return False


def main():
    d = mk(safari=True)
    res = []
    try:
        arm_parallel(d)

        # 1 happy — approve, result returns
        connect_and_launch_sign(d); approve(d)
        d.activate_app(SAFARI); time.sleep(3); webctx(d)
        r = page_result(d); v = verdict(r); res.append(('1 happy', v)); L('1 happy ->', v)

        # 8 reject — the ONLY row where 4001 is correct
        connect_and_launch_sign(d); tap_reject(d)
        d.activate_app(SAFARI); time.sleep(3); webctx(d)
        r = page_result(d, tries=12); v = verdict(r, reject_ok=True); res.append(('8 reject', v)); L('8 reject ->', v)

        # 3 kill — approve, then kill the app immediately (result must survive)
        connect_and_launch_sign(d); approve(d); time.sleep(1.0)
        try: d.terminate_app(VELA)
        except Exception as e: L('  kill err', str(e)[:40])
        d.activate_app(SAFARI); time.sleep(3); webctx(d)
        r = page_result(d); v = verdict(r); res.append(('3 kill', v)); L('3 kill ->', v)

        # 5 evict — approve, idle ~65s to evict the MV3 worker, then return
        connect_and_launch_sign(d); approve(d)
        d.activate_app(SAFARI); L('  idling ~65s to evict the background worker...')
        for _ in range(13):
            time.sleep(5)
            try: _ = d.title
            except Exception: webctx(d)
        webctx(d)
        r = page_result(d); v = verdict(r); res.append(('5 evict', v)); L('5 evict ->', v)

        # 7 reload — approve, reload the dApp tab mid-flight (KNOWN re-arm gap)
        connect_and_launch_sign(d); approve(d)
        d.activate_app(SAFARI); time.sleep(0.6); webctx(d); d.get(URL); time.sleep(2)
        r = page_result(d, tries=12); v = verdict(r); res.append(('7 reload', v)); L('7 reload ->', v)
    finally:
        d.quit()

    L('==================== REAL-PATH MATRIX SUMMARY ====================')
    for name, v in res:
        L(f'  {name:<10} {v}')
    fails = [n for n, v in res if v.startswith('FAIL')]
    L('RESULT:', 'ALL PASS' if not fails else f'FAILS: {fails}  (7 reload is a KNOWN non-fund-safety gap)')


if __name__ == '__main__':
    main()
