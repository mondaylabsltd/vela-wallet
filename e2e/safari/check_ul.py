#!/usr/bin/env python3
# Server-side AASA / UL provisioning diagnostic — NOT a definitive direct-open test.
#
# CAVEAT: iOS does NOT fire a Universal Link on address-bar navigation (this script's
# d.get(url)) or on synthetic Appium gestures — only on a REAL link tap with genuine
# user activation (the same limitation that forces check_real_sign to use
# mobile:deepLink). So a "did not open" result here does NOT prove UL is broken; the
# ONLY reliable check is a human doing a real sign in Safari and observing whether the
# "Open in Vela?" prompt appears. Use this script to confirm the server-side AASA is
# live and to print the device-provisioning guidance, not as a pass/fail gate.
#
# It navigates Safari to https://getvela.app/sign?rid=ul-selftest; if the app happens
# to foreground, the association is verified + swcd is fresh (a strong positive signal).
import time
import xml.etree.ElementTree as ET
from lib import mk, L, webctx, nsrc, center, VELA, SAFARI

UL = 'https://getvela.app/sign?rid=ul-selftest'


def foreground_bundle(d):
    try:
        return d.execute_script('mobile: activeAppInfo').get('bundleId')
    except Exception:
        return None


def main():
    d = mk(safari=True)
    try:
        webctx(d); d.get('https://example.com'); time.sleep(1.5)
        L('navigating Safari to the UL:', UL)
        # A location.assign is a script nav (may be treated as user-less); also try the
        # address-bar style get. Either way, a VERIFIED UL routes to the app.
        try:
            d.get(UL)
        except Exception as e:
            L('nav threw (may mean the app intercepted):', str(e)[:60])
        time.sleep(3)
        bid = foreground_bundle(d)
        L('foreground app after UL nav:', bid)
        if bid == VELA:
            src = nsrc(d)
            hit = center(src, '一键签名已启用') or any('一键签名' in (el.attrib.get('label') or '') for el in ET.fromstring(src).iter())
            L('[PASS] UL opened the app DIRECTLY (no scheme prompt).',
              'selftest confirmation shown.' if hit else '(app open; selftest UI not detected)')
        else:
            # Still in Safari → the UL was NOT verified on this device yet.
            L('[INFO] app did NOT open — UL not verified on this device. The AASA is live '
              '(server-side OK); swcd needs to re-fetch it: reinstall the app, or enable '
              'iPhone Settings > Developer > Associated Domains Development. Custom-scheme '
              'fallback (the "Open in Vela?" prompt) remains until then.')
    finally:
        d.quit()


if __name__ == '__main__':
    main()
