#!/usr/bin/env python3
# Capture the IN-SAFARI extension UI (the founder's "first impression in the
# browser"): the connect sheet and the sign hand-off sheet (State A intent +
# State C waiting). These are shadow-DOM sheets injected by content.js — we
# screenshot Safari itself (native context) so the real rendered pixels are
# captured, dark + light where possible.
import os, time
from lib import mk, L, webctx, provider_ready, sheet_shadow_click, URL, SAFARI

HERE = os.path.dirname(os.path.abspath(__file__))


def shot(d, name):
    p = os.path.join(HERE, f'shot-{name}.png')
    d.switch_to.context('NATIVE_APP')
    d.get_screenshot_as_file(p)
    L('screenshot ->', p)
    return p


def main():
    d = mk(safari=True)
    try:
        webctx(d); d.get(URL); time.sleep(2.0)
        L('provider injected?', provider_ready(d))

        # --- connect sheet (State: connect consent) ---
        d.execute_script("document.getElementById('btn-connect').click();")
        time.sleep(1.5)
        shot(d, 'connect-sheet')

        # confirm connect so a subsequent sign is authorized
        sheet_shadow_click(d, 'cta')
        time.sleep(1.5)

        # --- sign hand-off sheet: State A (intent) ---
        webctx(d)
        d.execute_script("document.getElementById('btn-sign').click();")
        time.sleep(1.8)
        shot(d, 'sign-intent')

        # --- State C (waiting / breathing ring) — synthetic click swaps the sheet
        #     to the waiting state without a real app launch under Appium ---
        sheet_shadow_click(d, 'cta')
        time.sleep(1.2)
        shot(d, 'sign-waiting')
    finally:
        d.quit()


if __name__ == '__main__':
    main()
