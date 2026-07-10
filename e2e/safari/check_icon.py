#!/usr/bin/env python3
# Capture the EIP-6963 icon the injected provider announces on-device and decode it,
# so we can confirm dApp wallet pickers get the REAL Vela sailboat logo (not the old
# placeholder). Writes the decoded PNG to shot-6963-icon.png for visual review.
import base64, os, time
from lib import mk, L, webctx, provider_ready, URL

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    d = mk(safari=True)
    try:
        webctx(d); d.get(URL); time.sleep(2)
        L('provider injected?', provider_ready(d))
        # Appium execute_script does NOT await Promises — capture into a window var,
        # dispatch requestProvider, wait, then read synchronously.
        d.execute_script(
            "window.__vi=null;"
            "window.addEventListener('eip6963:announceProvider', function(e){"
            "  window.__vi={name:e.detail.info.name, rdns:e.detail.info.rdns, icon:e.detail.info.icon};"
            "});"
            "window.dispatchEvent(new Event('eip6963:requestProvider'));")
        time.sleep(1.0)
        info = d.execute_script("return window.__vi;")
        if not info:
            L('[FAIL] no eip6963 announce captured'); return
        icon = info.get('icon') or ''
        L('announced:', info.get('name'), info.get('rdns'))
        L('icon prefix:', icon[:40], '| length:', len(icon))
        if icon.startswith('data:image/png;base64,'):
            b = base64.b64decode(icon.split(',', 1)[1])
            out = os.path.join(HERE, 'shot-6963-icon.png')
            open(out, 'wb').write(b)
            L('[PASS] real PNG icon announced on-device -> decoded to', out, f'({len(b)} bytes)')
        elif icon.startswith('data:image/svg'):
            L('[STALE] still announcing the SVG placeholder — appex not rebuilt with the fix')
        else:
            L('[FAIL] unexpected icon:', icon[:60])
    finally:
        d.quit()


if __name__ == '__main__':
    main()
