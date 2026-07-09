#!/usr/bin/env python3
# Device verification of the REWORKED sign hand-off:
#   A) the native SigningRequestModal still renders ABOVE the transparent /sign
#      route (no F5 regression from presentation:'transparentModal') — proven by
#      the approve control being FOUND after the deep-link launch.
#   B) after approve, the NEW settled bottom-sheet ("已发送 …") shows over the
#      dimmed wallet (not a full opaque debug page).
import os, time
import xml.etree.ElementTree as ET
from lib import (mk, L, webctx, nsrc, center, provider_ready, tap,
                 wait_dapp_result, sheet_shadow_click, URL, SAFARI, VELA)

APPROVE = ['签名', '授权', '确认', '批准', 'Sign', 'Approve', 'Confirm']
HERE = os.path.dirname(os.path.abspath(__file__))


def arm(d):
    try: d.terminate_app(VELA)
    except Exception: pass
    time.sleep(1.5); d.activate_app(VELA)
    for _ in range(15):
        time.sleep(2)
        try:
            if any((el.attrib.get('label') or '').startswith('账户 Parallel')
                   for el in ET.fromstring(nsrc(d)).iter()):
                L('parallel active'); break
        except Exception: pass
    d.activate_app(SAFARI); time.sleep(2)


def find_approve(d):
    best = None
    for el in ET.fromstring(nsrc(d)).iter():
        a = el.attrib
        if (a.get('label') or a.get('name')) in APPROVE:
            try:
                y = int(a['y']) + int(a['height']) // 2
                if best is None or y > best[0]: best = (y, (int(a['x']) + int(a['width']) // 2, y))
            except Exception: pass
    return best[1] if best else None


def shot(d, name):
    p = os.path.join(HERE, f'shot-{name}.png')
    d.switch_to.context('NATIVE_APP'); d.get_screenshot_as_file(p); L('->', p)


def main():
    d = mk(safari=True)
    try:
        arm(d)
        webctx(d); d.get(URL); time.sleep(2)
        L('provider?', provider_ready(d))
        d.execute_script("document.getElementById('btn-connect').click();")
        time.sleep(1.2); sheet_shadow_click(d, 'cta'); wait_dapp_result(d, 'eth_requestAccounts')
        # personal_sign hand-off → deep-link launch /sign
        d.execute_script("document.getElementById('btn-sign').click();")
        rid = None
        for _ in range(12):
            time.sleep(1)
            rid = d.execute_script("var e=document.getElementById('vela-r1-sign-status');return e?e.dataset.rid:null;")
            if rid: break
        sheet_shadow_click(d, 'cta'); time.sleep(1.5)
        if rid:
            d.execute_script('mobile: deepLink', {'url': 'velawallet://sign?rid=' + rid, 'bundleId': VELA})
        time.sleep(3)
        # (A) the SigningRequestModal must be present (renders above /sign)
        xy = None
        for _ in range(24):
            xy = find_approve(d)
            if xy: break
            time.sleep(0.6)
        L('A) SigningRequestModal approve control found:', bool(xy), '(True = no F5 regression)')
        shot(d, 'signflow-modal')
        if xy:
            tap(d, *xy); time.sleep(3)
            # (B) the settled sheet-over-wallet
            shot(d, 'signflow-settled')
            settled = [kw for kw in ('已发送', '返回 Safari', 'Signed') if center(nsrc(d), kw)]
            L('B) settled sheet shows:', settled or '(check screenshot)')
    finally:
        d.quit()


if __name__ == '__main__':
    main()
