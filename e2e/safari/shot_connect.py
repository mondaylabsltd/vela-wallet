#!/usr/bin/env python3
# Capture the CONNECT sheet on a FRESH origin (no stored grant → the sheet shows).
import os, time
from lib import mk, L, webctx, provider_ready

HERE = os.path.dirname(os.path.abspath(__file__))
FRESH = os.environ.get('VELA_FRESH_URL', 'http://192.168.50.40:8795/index.html')


def main():
    d = mk(safari=True)
    try:
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
