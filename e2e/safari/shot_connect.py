#!/usr/bin/env python3
# Capture the CONNECT sheet on a FRESH origin (no stored grant → the sheet shows).
import os, time
from lib import mk, L, webctx, provider_ready

HERE = os.path.dirname(os.path.abspath(__file__))
FRESH = os.environ.get('VELA_FRESH_URL', 'http://192.168.50.40:8795/index.html')


def main():
    d = mk(safari=True)
    try:
        # Foreground Vela first so AccountFileWriter re-writes the cache (with the new
        # chain icon data) before the extension reads it for the connect sheet.
        try:
            from lib import VELA
            d.activate_app(VELA); time.sleep(3); d.activate_app('com.apple.mobilesafari'); time.sleep(1.5)
        except Exception as e:
            L('vela foreground skipped:', str(e)[:40])
        webctx(d); d.get(FRESH); time.sleep(2.0)
        L('provider injected?', provider_ready(d))
        d.execute_script("document.getElementById('btn-connect').click();")
        time.sleep(1.8)
        d.switch_to.context('NATIVE_APP')
        p = os.path.join(HERE, 'shot-connect-fresh.png')
        d.get_screenshot_as_file(p); L('screenshot ->', p)
    finally:
        d.quit()


if __name__ == '__main__':
    main()
