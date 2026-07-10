#!/usr/bin/env python3
# Quick check: is the Phase A EIP-1193/6963 provider injected on VELA_TEST_URL?
# Run this first — if markers are False, re-enable the extension in
# Settings > Safari > Extensions > Vela Wallet > All Websites > Allow.
import time
from lib import mk, L, webctx, URL

d = mk(safari=True)
try:
    webctx(d); d.get(URL); time.sleep(2.5)
    # EIP-6963 discovery: dispatch requestProvider and capture the announce.
    d.execute_script(
        "window.__v6963=[];"
        "window.addEventListener('eip6963:announceProvider',function(e){"
        "window.__v6963.push(e.detail.info.rdns);});"
        "window.dispatchEvent(new Event('eip6963:requestProvider'));")
    time.sleep(0.6)
    m = d.execute_script(
        "return {eth:(typeof window.ethereum!=='undefined'),"
        "isVela:!!(window.ethereum&&window.ethereum.isVela),"
        "chainId:(window.ethereum&&window.ethereum.chainId)||null,"
        "e6963:(window.__v6963||[]),"
        "host:(typeof document.getElementById('vela-sheet-host')!=='undefined')};")
    L('provider markers on', URL, ':', m)
    ok = m.get('eth') and m.get('isVela') and ('app.getvela' in (m.get('e6963') or []))
    L('OK' if ok else 'NOT INJECTING (or provider missing) — enable+grant the extension, rebuild the bundle')
finally:
    d.quit()
