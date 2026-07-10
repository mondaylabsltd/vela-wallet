#!/usr/bin/env python3
# One-time re-arm of the parallel-space test env after a fresh (data-wiped) install.
# The clean reinstall cleared AsyncStorage → K_FLAG + dev_unlocked gone → parallel not
# armed. But this is a DEV build (__DEV__=true), and /parallel/_layout sets
# access=__DEV__?'allow':'checking' → entering /parallel runs enterParallelSpace
# regardless of dev_unlocked. So a COLD `velawallet://parallel` deep link arms it
# (installs the fixed-key signer + fixture wallet + sets K_FLAG so subsequent cold
# launches re-arm via applyParallelSpaceOnBoot). deepLink (not a Safari tap) reliably
# routes the scheme without the "Open in Vela?" banner.
import time
import xml.etree.ElementTree as ET
from lib import mk, L, nsrc, center, VELA


def armed(d):
    try:
        src = nsrc(d)
        if center(src, '账户 Parallel One。切换账户'):
            return True
        for el in ET.fromstring(src).iter():
            lab = el.attrib.get('label') or el.attrib.get('name') or ''
            if lab.startswith('账户 Parallel') or 'PARALLEL' in lab.upper() or 'Parallel One' in lab:
                return True
    except Exception:
        pass
    return False


def main():
    d = mk(safari=False)  # bind to the Vela app
    try:
        try:
            d.terminate_app(VELA); L('terminated Vela (for a cold launch)')
        except Exception as e:
            L('terminate err', str(e)[:40])
        time.sleep(1.2)
        # Cold deep link → getInitialURL routes /parallel → __DEV__ allow → enterParallelSpace.
        d.execute_script('mobile: deepLink', {'url': 'velawallet://parallel', 'bundleId': VELA})
        L('deepLink velawallet://parallel sent; waiting for the fixture wallet to load…')
        ok = False
        for i in range(20):
            time.sleep(2)
            if armed(d):
                ok = True
                L('[PASS] parallel armed — fixture wallet "Parallel One" loaded (fixed-key signer active)')
                break
            if i == 6:
                L('… still loading (Metro bundle on a fresh DEV build can take ~30s)')
        if not ok:
            L('[FAIL] parallel did not arm — dumping a hint of the current screen')
            try:
                src = nsrc(d)
                labels = [el.attrib.get('label') for el in ET.fromstring(src).iter() if el.attrib.get('label')]
                L('visible labels:', [x for x in labels[:14]])
            except Exception:
                pass
    finally:
        d.quit()


if __name__ == '__main__':
    main()
