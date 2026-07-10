#!/usr/bin/env python3
# Phase B headless real-sign check. Unlike check_provider.py (fake sign), this
# drives the REAL SigningRequestModal → Passkey.sign → EIP-1271 signature, with
# NO Face ID by running inside the parallel-space test env (fixed-key signer).
#
# Flow:
#   1. arm parallel space  (velawallet://parallel — needs dev_unlocked=1, which the
#      founder's device already has; the fixed-key signer persists + re-arms on the
#      extension's cold app-launch via applyParallelSpaceOnBoot).
#   2. connect on the test dApp (grants the fixture Safe).
#   3. personal_sign → hand-off sheet → launch → REAL SigningSheet → tap 签名/Sign
#      (fixed-key signs headlessly) → return → read the dApp's real signature.
#
# PASS = the dApp promise resolves with a REAL signature (0x…, NOT the 0xFA fake),
# with no Face-ID hang.
import os, time
import xml.etree.ElementTree as ET
from lib import (mk, L, webctx, nsrc, center, app_status, tap, provider_ready,
                 dapp_result, wait_dapp_result, sheet_shadow_click, URL, SAFARI, VELA)

# The approve BUTTON label (NOT the sheet title). signature→签名, send→授权,
# batch→确认, approval→批准. Deliberately excludes title words like 发送 (which is
# the send sheet's TITLE, a StaticText — tapping it does nothing).
APPROVE = ['签名', '授权', '确认', '批准', 'Sign', 'Approve', 'Confirm']
OPEN = ['打开', 'Open']                          # "Open in Vela?" scheme banner

# Which signing method to exercise (the whole Phase-B path is method-agnostic — the
# transport + two-slot provider + SigningRequestModal don't branch on method; only
# the dApp button + the RPC name differ). Both signature methods return a real
# EIP-1271 signature hex via the fixed-key signer; the PASS check is identical.
#   personal_sign         → testdapp btn-sign  (5)
#   eth_signTypedData_v4  → testdapp btn-typed (6)
SIGN_METHODS = {
    'personal_sign':        {'btn': 'btn-sign',  'rpc': 'personal_sign',        'wait': 25},
    'eth_signTypedData_v4': {'btn': 'btn-typed', 'rpc': 'eth_signTypedData_v4', 'wait': 25},
    # A send resolves only AFTER the UserOp is mined (handleSendTransaction →
    # waitForTxHash), so the dApp promise can take a minute+ on Gnosis. Longer poll.
    'eth_sendTransaction':  {'btn': 'btn-tx',    'rpc': 'eth_sendTransaction',  'wait': 90},
}
METHOD = os.environ.get('VELA_SIGN_METHOD', 'personal_sign')
if METHOD not in SIGN_METHODS:
    raise SystemExit(f'VELA_SIGN_METHOD must be one of {list(SIGN_METHODS)} (got {METHOD!r})')
SIGN_BTN = SIGN_METHODS[METHOD]['btn']
SIGN_RPC = SIGN_METHODS[METHOD]['rpc']
SIGN_WAIT = SIGN_METHODS[METHOD]['wait']

# UI/observe mode: VELA_OBSERVE=1 pauses at the real SigningSheet so you can SEE it
# (it's pixel-identical to production — the ONLY difference is the fixed-key in-app
# signer instead of a device passkey). VELA_OBSERVE=manual leaves the 签名 tap to you.
OBSERVE = os.environ.get('VELA_OBSERVE', '')
HERE = os.path.dirname(os.path.abspath(__file__))


def find_label(d, labels):
    src = nsrc(d)
    for lab in labels:
        xy = center(src, lab)
        if xy:
            return lab, xy
    return None, None


def find_approve(d, labels):
    """The approve CTA is the BOTTOM-most element matching any approve label.
    center()/find_label return the FIRST match in tree order, which for the send
    sheet is a non-button '授权' element ABOVE the real button (the signature sheet
    had no such duplicate, so find_label worked there). Pick the largest-y match =
    the bottom CTA button."""
    src = nsrc(d)
    best = None  # (y_center, lab, (x, y))
    for el in ET.fromstring(src).iter():
        a = el.attrib
        lab = a.get('label') or a.get('name')
        if lab in labels:
            try:
                x = int(a['x']) + int(a['width']) // 2
                y = int(a['y']) + int(a['height']) // 2
                if best is None or y > best[0]:
                    best = (y, lab, (x, y))
            except Exception:
                pass
    return (best[1], best[2]) if best else (None, None)


def tap_label(d, labels, tries=1):
    for _ in range(tries):
        src = nsrc(d)
        for lab in labels:
            xy = center(src, lab)
            if xy:
                tap(d, *xy); return lab
        time.sleep(0.6)
    return None


def cold_prep(d):
    """Parallel space is armed manually (founder). Terminate Vela so the extension's
    velawallet://sign launch COLD-starts the app — Expo Router routes the deep link
    via getInitialURL, and applyParallelSpaceOnBoot re-installs the fixed-key signer
    (K_FLAG persists), so the real sign runs with the fixture key + no Face ID."""
    try:
        d.terminate_app(VELA); L('prep: terminated Vela (cold-launch for the sign)')
    except Exception as e:
        L('prep: terminate err', str(e)[:40])
    time.sleep(1.0)


def arm_parallel_dev(d):
    """DEV build (__DEV__=true): the fixed-key signer override actually works, and the
    app boots straight into parallel (K_FLAG persists → applyParallelSpaceOnBoot).
    Start from a CLEAN cold launch so each sign is fresh (no stale /sign rid), waiting
    out the Metro bundle download."""
    import xml.etree.ElementTree as ET
    try:
        d.terminate_app(VELA)
    except Exception:
        pass
    time.sleep(1.5)
    d.activate_app(VELA)
    # Wait for the JS bundle (Metro) to finish loading into the parallel home.
    armed = False
    for _ in range(15):
        time.sleep(2)
        try:
            src = nsrc(d)
            if center(src, '账户 Parallel One。切换账户') or any(
                (el.attrib.get('label') or '').startswith('账户 Parallel')
                for el in ET.fromstring(src).iter()
            ):
                L('arm: app loaded into parallel (fixed-key signer active)')
                armed = True
                break
        except Exception:
            pass
    if not armed:
        # Not in parallel → the real sign would hit a device passkey / Face ID prompt
        # that no one is here to approve; the run won't hang (bounded polls + quit in
        # finally) but WILL report FAIL(None). Flag it loudly so the cause is obvious.
        L('arm: !! PARALLEL NOT DETECTED — fixed-key signer likely inactive; the sign '
          'may prompt Face ID and FAIL. Arm parallel first (velawallet://parallel).')
    d.activate_app(SAFARI); time.sleep(2)
    return armed


def main():
    d = mk(safari=True)
    ok_steps = 0
    L('method =', METHOD, '(btn', SIGN_BTN + ')')
    try:
        arm_parallel_dev(d)
        # Robust page load: after arm_parallel_dev cold-launches Vela (heavy Metro
        # churn), Safari can be mid-foreground-transition when we navigate, so a
        # single d.get(URL) sometimes lands on a blank page (btn-connect null,
        # provider absent). Re-navigate until the static dApp button exists (page
        # really loaded) — THEN check the provider.
        loaded = False
        for attempt in range(6):
            try:
                webctx(d); d.get(URL); time.sleep(2.0)
                if d.execute_script("return !!document.getElementById('btn-connect');"):
                    loaded = True; break
            except Exception as e:
                L('load retry', attempt, str(e)[:50])
            d.activate_app(SAFARI); time.sleep(1.5)
        if not loaded:
            L('[FAIL] test dApp page never loaded (btn-connect null) — Safari/webview flaky')
        if provider_ready(d): ok_steps += 1; L('[PASS] provider injected')
        else: L('[FAIL] provider not injected')

        # connect (answered in-Safari; needs no app). Parallel armed → fixture Safe.
        # A stale real-address grant vs the fixture cache re-shows the connect sheet.
        d.execute_script("document.getElementById('btn-connect').click();")
        time.sleep(1.2); sheet_shadow_click(d, 'cta')
        r = wait_dapp_result(d, 'eth_requestAccounts')
        addr = r.get('value', [None])[0] if r and r.get('ok') else None
        armed = bool(addr and not addr.lower().startswith('0x14fb'))
        L('connect ->', addr, '| parallel armed (fixture Safe)?', armed)
        if addr: ok_steps += 1

        # personal_sign → real SigningSheet. Two moving parts, cleanly separated for
        # automation: (a) content.js's onSignLaunch (fired by the CTA click) writes
        # the REAL sign-req-<rid>.json via background→native; (b) the app must open
        # /sign for that rid. The Safari scheme-launch banner is flaky under Appium
        # (WebKit suppresses it for synthetic gestures), so instead of tapping the
        # banner we drive the launch with Appium's reliable `mobile: deepLink` —
        # warm/cold both route /sign (verified). This still exercises the whole REAL
        # Phase-B path (content sign-req write → sign.tsx → ExtensionBridgeTransport →
        # SigningRequestModal → fixed-key Passkey.sign → result file); only the
        # Safari→app hop (a Phase-A concern, already 5/5) is swapped for a deepLink.
        d.execute_script(f"document.getElementById('{SIGN_BTN}').click();")
        # The tx button is async (awaits wallet_switchEthereumChain → Gnosis before
        # eth_sendTransaction), so the sign-intent sheet + rid can lag. Poll for it.
        rid = None
        for _ in range(12):
            time.sleep(1.0)
            rid = d.execute_script(
                "var e=document.getElementById('vela-r1-sign-status');return e?e.dataset.rid:null;")
            if rid:
                break
        L('sign rid =', rid)
        sheet_shadow_click(d, 'cta')   # fires onSignLaunch → writeSignRequest (sign-req written)
        time.sleep(1.5)                # let the write reach native (races the launch)
        if rid:
            # VELA_LAUNCH=scheme (default) tests velawallet://sign → /sign trampoline;
            # VELA_LAUNCH=ul tests https://getvela.app/sign?rid → AccountFileWriter routing.
            launch = os.environ.get('VELA_LAUNCH', 'scheme')
            url = ('https://getvela.app/sign?rid=' if launch == 'ul' else 'velawallet://sign?rid=') + rid
            try:
                d.execute_script('mobile: deepLink', {'url': url, 'bundleId': VELA})
                L('launched via deepLink [%s]:' % launch, url[:48])
            except Exception as e:
                L('deepLink err:', str(e)[:50])
        time.sleep(3.0)  # app routes /sign, reads sign-req, renders SigningRequestModal

        # Wait for the REAL SigningSheet's approve control to appear (don't tap yet).
        lab, xy = None, None
        for _ in range(24):
            lab, xy = find_approve(d, APPROVE)
            if lab:
                break
            time.sleep(0.6)
        L('sign: approve control =', lab, '(None = SigningSheet not found)')
        if lab:
            ok_steps += 1
            # Always capture what the signing UI looks like (pixel-identical to prod).
            try:
                stem = 'signsheet' if METHOD == 'personal_sign' else f'signsheet-{METHOD}'
                shot = os.path.join(HERE, f'{stem}.png')
                d.get_screenshot_as_file(shot); L('screenshot ->', shot)
            except Exception as e:
                L('screenshot err:', str(e)[:40])
            # Sends: the 授权 button is DISABLED while 预估费用 shows 估算中 (fee not
            # yet estimated) — tapping it is a no-op. Wait for the estimate, then
            # re-locate the button (the fee row resolving shifts the layout).
            if METHOD == 'eth_sendTransaction':
                for _ in range(25):
                    src = nsrc(d)
                    if '估算中' not in src and (center(src, '授权') or center(src, '批准')):
                        L('sign: fee estimated — approve button ready')
                        break
                    time.sleep(1.0)
                else:
                    L('sign: WARN fee still 估算中 after wait — tapping anyway')
                lab2, xy2 = find_approve(d, APPROVE)
                if xy2:
                    xy = xy2
            if OBSERVE:
                secs = int(os.environ.get('VELA_OBSERVE_SECS', '25')) if OBSERVE == 'manual' else 15
                L(f'>>> OBSERVE: the REAL SigningSheet is on screen — look at the phone (~{secs}s).')
                L('    Fixed-key in-app signer will sign (NO device passkey / Face ID).')
                if OBSERVE == 'manual':
                    L('    Tap 签名 yourself now, or wait to auto-tap.')
                time.sleep(secs)
            if not (OBSERVE == 'manual'):
                L('sign: tapping approve', lab, 'at', xy)
                tap(d, *xy)  # bottom-most approve-label element (find_approve)
        time.sleep(2.5)   # fixed-key Passkey.sign + result write
        # Sends: capture the POST-approve app state (submitting / error / funding
        # modal / receipt wait) so a None result can be diagnosed.
        if METHOD == 'eth_sendTransaction':
            try:
                time.sleep(2.0)
                shot2 = os.path.join(HERE, 'signsheet-eth_sendTransaction-post.png')
                d.get_screenshot_as_file(shot2); L('post-approve screenshot ->', shot2)
                src = nsrc(d)
                hits = [kw for kw in ('估算中', '余额不足', '充值', '燃气', 'Gas', '失败', '错误',
                                      '提交', '签名中', '等待', '已提交', '完成', 'Insufficient', 'error')
                        if kw in src]
                L('post-approve state hints:', hits or '(none matched)')
            except Exception as e:
                L('post-approve diag err:', str(e)[:50])

        # return to Safari; the dApp promise should resolve with a REAL signature.
        d.activate_app(SAFARI); time.sleep(3); webctx(d)
        rr = wait_dapp_result(d, SIGN_RPC, tries=SIGN_WAIT)
        L(SIGN_RPC, '->', rr)
        val = (rr or {}).get('value')
        real = bool(val and isinstance(val, str) and val.startswith('0x') and not val.startswith('0xFA') and len(val) > 20)
        false_decline = bool(rr and not rr.get('ok') and rr.get('code') == 4001)
        if real and not false_decline:
            ok_steps += 1
            L('[PASS] REAL EIP-1271 signature returned (headless, no Face ID):', str(val)[:26], '…')
        else:
            L('[FAIL] no real signature (val=%s, false_decline=%s)' % (str(val)[:26], false_decline))
    finally:
        d.quit()
    L('==================== PHASE B REAL-SIGN SUMMARY ====================')
    L(f'RESULT: {ok_steps}/4 steps (provider, connect, approve-tapped, real-signature)')


if __name__ == '__main__':
    main()
