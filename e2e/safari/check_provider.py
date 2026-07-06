#!/usr/bin/env python3
# Phase A on-device flow: drive the REAL EIP-1193/6963 provider against the test
# dApp (packages/safari-extension/testdapp/index.html served at VELA_TEST_URL),
# through connect → chainId → getBalance → personal_sign, reading the dApp's own
# JSON-RPC result (window.__velaTestResult) rather than the old status pill.
#
# Prereqs (see README + PHASE-A-RUNBOOK.md):
#   - Release build installed; Safari extension enabled + granted on VELA_TEST_URL
#   - VELA_TEST_URL points at the served test dApp (NOT example.com)
#   - the app is logged in with at least one account (so vela.ext.account.json exists)
#   - VELA_WEB_YOFFSET calibrated (Safari top-toolbar height in points) for the
#     sign CTA coordinate tap
import time
from lib import (mk, L, webctx, nsrc, center, app_status, tap, provider_ready,
                 dapp_result, wait_dapp_result, sheet_shadow_click, sheet_cta_tap,
                 URL, SAFARI, VELA)


def step(name, ok, extra=''):
    L(f'  [{"PASS" if ok else "FAIL"}] {name}', extra)
    return ok


def main():
    d = mk(safari=True)
    results = []
    try:
        webctx(d); d.get(URL); time.sleep(2.0)
        results.append(step('provider injected (window.ethereum.isVela)', provider_ready(d)))

        # --- connect (in-Safari sheet, zero app hop) ---
        d.execute_script("document.getElementById('btn-connect').click();")
        time.sleep(1.2)
        # The connect sheet is an open shadow root; confirm is #cta (a button).
        clicked = sheet_shadow_click(d, 'cta')
        r = wait_dapp_result(d, 'eth_requestAccounts')
        ok = bool(r and r.get('ok') and isinstance(r.get('value'), list) and r['value'])
        results.append(step('eth_requestAccounts -> [address]', ok, str(r and r.get('value'))))

        # --- chainId (state, local) ---
        d.execute_script("document.getElementById('btn-chainid').click();")
        r = wait_dapp_result(d, 'eth_chainId')
        results.append(step('eth_chainId', bool(r and r.get('ok') and str(r.get('value', '')).startswith('0x')), str(r and r.get('value'))))

        # --- getBalance (read, proxied to RPC) ---
        d.execute_script("document.getElementById('btn-balance').click();")
        r = wait_dapp_result(d, 'eth_getBalance')
        results.append(step('eth_getBalance (proxied)', bool(r and r.get('ok')), str(r and (r.get('value') or r.get('message')))))

        # --- personal_sign (app hop; fake-signs in Phase A) ---
        d.execute_script("document.getElementById('btn-sign').click();")
        time.sleep(1.0)
        # Synthetic click on the shadow CTA anchor: its href=velawallet://sign?rid
        # navigates → the native "Open in Vela?" banner (the real gesture gate,
        # R1-proven). No coordinate calibration needed for the custom-scheme path.
        if not sheet_shadow_click(d, 'cta'):
            L('  !! sign hand-off sheet CTA not found (provider/content mismatch?)')
        time.sleep(2.0)
        xy = center(nsrc(d), '打开') or center(nsrc(d), 'Open')  # "Open in Vela?" banner
        if xy:
            tap(d, *xy)
        # native sign screen -> approve the fake sign
        for _ in range(20):
            s = app_status(nsrc(d))
            if s and 'request received' in s:
                break
            time.sleep(0.5)
        cxy = center(nsrc(d), 'Sign (fake)')
        if cxy:
            tap(d, *cxy)
        # Wait for the app to finish + write the result BEFORE returning to Safari,
        # else the single focus-poll races the App-Group write (would read CHECK_VELA).
        for _ in range(16):
            s = app_status(nsrc(d))
            if s and 'Signed' in s:
                break
            time.sleep(0.5)
        # return to Safari; the provider should resolve the dApp promise
        d.activate_app(SAFARI); time.sleep(3); webctx(d)
        r = wait_dapp_result(d, 'personal_sign')
        ok = bool(r and r.get('ok') and str(r.get('value', '')).startswith('0x'))
        # invariant (b): a returned 4001 here would be a false decline
        false_decline = bool(r and (not r.get('ok')) and r.get('code') == 4001)
        results.append(step('personal_sign resolves (no false-decline)', ok and not false_decline,
                            str(r and (r.get('value') or (r.get('code'), r.get('message'))))))
    finally:
        d.quit()

    L('==================== PHASE A SUMMARY ====================')
    passed = sum(1 for x in results if x)
    L(f'RESULT: {passed}/{len(results)} steps passed')


if __name__ == '__main__':
    main()
