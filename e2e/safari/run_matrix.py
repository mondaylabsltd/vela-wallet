#!/usr/bin/env python3
# Vela Safari-extension R1 return-path regression matrix (fully automated).
# Drives Safari + the native app via Appium/WDA and reads the page-side verdict.
# Requires: Appium server running, a Release build of Vela installed on the
# device (dev-launcher breaks deep-link automation), the Safari extension
# ENABLED + granted on VELA_TEST_URL. See README.md.
#
# Verdict source = the injected #vela-r1-sign-status element:
#   "✓ submitted 0x…"           -> RESOLVED         (a) satisfied
#   "✕ rejected (4001)"          -> rejected         (b): ONLY OK on the reject row
#   "… 请在 Vela 中查看 (4900)"  -> CHECK_VELA/4900   (a) satisfied (recoverable)
#   None                          -> no verdict shown (investigate)
import time
from lib import mk, L, webctx, nsrc, center, app_status, tap, page_status, wait_btn, URL, SAFARI, VELA


def flow(d):
    """Fresh page -> tap injected R1 sign -> confirm scheme banner -> wait sign screen."""
    webctx(d); d.get(URL)
    if not wait_btn(d):
        L('!! R1 sign button never injected (grant the extension on', URL, ')'); return False
    d.execute_script("document.getElementById('vela-r1-sign-btn').click();")
    time.sleep(2.5)
    xy = center(nsrc(d), '打开') or center(nsrc(d), 'Open')  # "Open in Vela?" banner
    if xy:
        tap(d, *xy)
    for _ in range(24):
        s = app_status(nsrc(d))
        if s and 'request received' in s:
            return True
        time.sleep(0.5)
    return False


def taplabel(d, lab):
    xy = center(nsrc(d), lab)
    if xy:
        tap(d, *xy); return True
    return False


def wait_signed(d, t=10):
    for _ in range(t * 2):
        s = app_status(nsrc(d))
        if s and 'Signed' in s:
            return True
        time.sleep(0.5)
    return False


def read_page(d, tries=18):
    txt = None
    for _ in range(tries):
        try:
            txt = page_status(d)
        except Exception:
            webctx(d)
        if txt and ('submitted' in txt or 'rejected' in txt):
            break
        time.sleep(1)
    return txt


def ret(d):
    d.activate_app(SAFARI); time.sleep(3); webctx(d)


def verdict(txt, reject_ok=False):
    if not txt:
        return 'FAIL(None)'
    if 'submitted' in txt:
        return 'PASS'
    if ('rejected' in txt or '4001' in txt):
        return 'PASS' if reject_ok else 'FAIL(false-decline!)'
    if 'Vela' in txt or '4900' in txt:
        return 'PASS(check-vela)'   # (a) satisfied: not lost, recoverable
    return 'FAIL'


def main():
    d = mk(safari=True)
    res = []
    try:
        # 1 happy
        if flow(d): taplabel(d, 'Sign (fake)')
        ret(d); t = read_page(d); r = verdict(t); res.append(('1 happy', r, t)); L('1 happy ->', r, '|', t)
        # 8 reject  (the ONLY row where 4001 is correct)
        if flow(d): taplabel(d, 'dev: reject')
        ret(d); t = read_page(d); r = verdict(t, reject_ok=True); res.append(('8 reject', r, t)); L('8 reject ->', r, '|', t)
        # 3 kill-after-sign
        if flow(d):
            taplabel(d, 'Sign (fake)'); wait_signed(d)
            try: d.terminate_app(VELA)
            except Exception as e: L('kill err', str(e)[:50])
        ret(d); t = read_page(d); r = verdict(t); res.append(('3 kill', r, t)); L('3 kill ->', r, '|', t)
        # 5 evict (idle ~65s -> MV3 worker dies; keep the Appium session alive)
        if flow(d):
            taplabel(d, 'Sign (fake)'); wait_signed(d)
            d.activate_app(SAFARI); L('  idling ~65s to evict the background worker...')
            for _ in range(13):
                time.sleep(5)
                try: _ = d.title
                except Exception: webctx(d)
            d.activate_app(VELA); time.sleep(1.5); d.activate_app(SAFARI); time.sleep(3)
        t = read_page(d); r = verdict(t); res.append(('5 evict', r, t)); L('5 evict ->', r, '|', t)
        # 7 reload-mid-flight  (KNOWN GAP: storage.local re-arm)
        if flow(d):
            taplabel(d, 'Sign (fake)')
            d.activate_app(SAFARI); time.sleep(0.6); webctx(d); d.get(URL); wait_btn(d)
        t = read_page(d); r = verdict(t); res.append(('7 reload', r, t)); L('7 reload ->', r, '|', t)
    finally:
        d.quit()
    L('==================== MATRIX SUMMARY ====================')
    for name, r, t in res:
        L(f'  {name:<10} {r:<22} {t}')
    fails = [n for n, r, _ in res if r.startswith('FAIL')]
    L('RESULT:', 'ALL PASS' if not fails else f'FAILS: {fails}  (reload is a known non-fund-safety gap)')


if __name__ == '__main__':
    main()
