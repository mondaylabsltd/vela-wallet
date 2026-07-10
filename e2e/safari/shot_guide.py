#!/usr/bin/env python3
# Navigate the app to the new "Use Vela in Safari" guide screen and screenshot it.
# The parallel-space app runs in Chinese, so we match the zh row label.
import os, time
import xml.etree.ElementTree as ET
from lib import mk, L, nsrc, center, tap, VELA

HERE = os.path.dirname(os.path.abspath(__file__))
ROW = '在 Safari 中使用 Vela'        # safariExt.settingsTitle (zh)
NAV = '在 Safari 里用 Vela'           # safariExt.heroTitle (zh) — confirms we're on the guide


def find_gear(src):
    # Settings gear: top-right. Try common labels, else the top-right-most tappable.
    for lab in ('设置', 'Settings', '账户设置'):
        xy = center(src, lab)
        if xy:
            return xy
    return None


def scroll_find(d, label, tries=6):
    for _ in range(tries):
        src = nsrc(d)
        xy = center(src, label)
        if xy:
            return xy
        d.execute_script('mobile: scroll', {'direction': 'down'})
        time.sleep(0.8)
    return None


def main():
    d = mk(safari=False)  # drive the native app directly
    try:
        d.activate_app(VELA); time.sleep(3)
        # Open Settings — tap the gear (top-right ~ x=0.9*w, y=0.09*h) as a fallback.
        src = nsrc(d)
        gear = find_gear(src)
        if not gear:
            # tap top-right area
            root = ET.fromstring(src)
            w = int(root.attrib.get('width', 390)); h = int(root.attrib.get('height', 844))
            gear = (int(w * 0.92), int(h * 0.10))
            L('gear not found by label — tapping top-right', gear)
        tap(d, *gear); time.sleep(1.5)
        # Scroll to the guide row and open it.
        xy = scroll_find(d, ROW)
        if not xy:
            L('[FAIL] guide row not found in Settings');
            d.get_screenshot_as_file(os.path.join(HERE, 'shot-guide-settings.png'))
            return
        L('found guide row, opening...')
        tap(d, *xy); time.sleep(1.5)
        p = os.path.join(HERE, 'shot-guide.png')
        d.get_screenshot_as_file(p); L('screenshot ->', p)
        on = center(nsrc(d), NAV) is not None or any(NAV in (e.attrib.get('label') or '') for e in ET.fromstring(nsrc(d)).iter())
        L('[PASS] guide screen shown' if on else '[?] captured, verify screenshot')
    finally:
        d.quit()


if __name__ == '__main__':
    main()
