# Shared Appium helpers for the Vela Safari-extension E2E regression.
# See README.md for setup (Appium + WebDriverAgent + venv). Device/team are
# read from env so this is portable across machines.
import os, time, xml.etree.ElementTree as ET
from appium import webdriver
from appium.options.ios import XCUITestOptions

UDID    = os.environ.get('VELA_UDID',    '00008030-001A75961445802E')  # ABC iPhone 11
TEAM    = os.environ.get('VELA_TEAM',    'F9W689P9NE')                 # Apple org team
WDA_BID = os.environ.get('VELA_WDA_BID', 'app.getvela.VelaWallet.wda')
VELA    = os.environ.get('VELA_APP_BID', 'app.getvela.VelaWallet')
URL     = os.environ.get('VELA_TEST_URL','https://example.com')        # extension must be granted here
APPIUM  = os.environ.get('VELA_APPIUM',  'http://127.0.0.1:4723')
SAFARI  = 'com.apple.mobilesafari'


def mk(safari=True):
    o = XCUITestOptions()
    o.platform_name = 'iOS'; o.automation_name = 'XCUITest'
    o.udid = UDID; o.device_name = 'iPhone'
    if safari:
        o.set_capability('browserName', 'Safari')
        o.set_capability('safariInitialUrl', URL)
    else:
        o.set_capability('bundleId', VELA)
    o.set_capability('xcodeOrgId', TEAM)
    o.set_capability('xcodeSigningId', 'Apple Development')
    o.set_capability('updatedWDABundleId', WDA_BID)
    o.set_capability('wdaLaunchTimeout', 300000)
    o.set_capability('newCommandTimeout', 300)   # survive the >60s evict idle
    return webdriver.Remote(APPIUM, options=o)


def L(*a):
    print('[H]', *a, flush=True)


def webctx(d):
    for c in d.contexts:
        if c.startswith('WEBVIEW'):
            try:
                d.switch_to.context(c); return c
            except Exception:
                pass
    return None


def nsrc(d):
    """Native page_source snapshot (one atomic read — avoids stale-element churn)."""
    d.switch_to.context('NATIVE_APP')
    return d.page_source


def center(src, label):
    """Center point of the element whose label/name == label, from a page_source snapshot."""
    for el in ET.fromstring(src).iter():
        a = el.attrib
        if a.get('label') == label or a.get('name') == label:
            try:
                return (int(a['x']) + int(a['width']) // 2, int(a['y']) + int(a['height']) // 2)
            except Exception:
                pass
    return None


def app_status(src):
    for el in ET.fromstring(src).iter():
        for v in (el.attrib.get('label'), el.attrib.get('value')):
            if v and v.startswith('status:'):
                return v
    return None


def tap(d, x, y):
    d.execute_script('mobile: tap', {'x': x, 'y': y})


def page_status(d):
    """The injected page-side verdict element (RN Pressable taps don't fire via
    Appium clicks — see README; content-script injects this DOM element)."""
    webctx(d)
    return d.execute_script(
        "var e=document.getElementById('vela-r1-sign-status');return e?e.textContent:null;")


def wait_btn(d, timeout=12):
    """Wait for the injected R1 sign button (handles the content-script inject race)."""
    end = time.time() + timeout
    while time.time() < end:
        try:
            if d.execute_script("return !!document.getElementById('vela-r1-sign-btn');"):
                return True
        except Exception:
            webctx(d)
        time.sleep(0.6)
    return False


# ==== Phase A (real EIP-1193/6963 provider) helpers ==========================

def provider_ready(d, timeout=12):
    """Wait for window.ethereum (isVela) to install."""
    end = time.time() + timeout
    while time.time() < end:
        try:
            if d.execute_script("return !!(window.ethereum&&window.ethereum.isVela);"):
                return True
        except Exception:
            webctx(d)
        time.sleep(0.5)
    return False


def dapp_result(d):
    """Latest JSON-RPC result the test dApp recorded (testdapp/index.html)."""
    webctx(d)
    return d.execute_script("return window.__velaTestResult||null;")


def wait_dapp_result(d, method, tries=20):
    """Poll until the test dApp's last result is for `method` and settled."""
    for _ in range(tries):
        try:
            r = dapp_result(d)
        except Exception:
            webctx(d); r = None
        if r and r.get('method') == method:
            return r
        time.sleep(1)
    return None


def sheet_shadow_click(d, el_id):
    """Synthetic click a control inside the open shadow-DOM sheet. OK for the
    CONNECT confirm/cancel (no app launch) — NOT for the sign CTA (needs a real
    gesture; use sheet_cta_tap)."""
    webctx(d)
    return d.execute_script(
        "var h=document.getElementById('vela-sheet-host');"
        "if(!h||!h.shadowRoot)return false;"
        "var e=h.shadowRoot.getElementById(arguments[0]);"
        "if(!e)return false;e.click();return true;", el_id)


def sheet_cta_tap(d, el_id='cta', yoffset=None):
    """Coordinate-tap the sign hand-off CTA (an <a href=velawallet://…>). A real
    tap is REQUIRED — a synthetic .click() carries no user activation, so iOS
    drops the scheme launch (FACT-1). Converts the shadow-element CSS rect to
    device points via devicePixelRatio + a chrome y-offset. Calibrate the offset
    once per device with VELA_WEB_YOFFSET (Safari's top toolbar height in points)."""
    webctx(d)
    r = d.execute_script(
        "var h=document.getElementById('vela-sheet-host');"
        "if(!h||!h.shadowRoot)return null;"
        "var e=h.shadowRoot.getElementById(arguments[0]);if(!e)return null;"
        "var b=e.getBoundingClientRect();"
        "return {x:b.x+b.width/2,y:b.y+b.height/2,dpr:window.devicePixelRatio||1,"
        "iw:window.innerWidth,ih:window.innerHeight};", el_id)
    if not r:
        return False
    yo = float(os.environ.get('VELA_WEB_YOFFSET', '0')) if yoffset is None else yoffset
    # CSS px → device points (XCUITest mobile:tap uses points, not pixels, so the
    # dpr cancels for width but Safari renders web content 1:1 in points here;
    # keep it simple: tap at CSS coords offset by the toolbar height).
    x = int(r['x'])
    y = int(r['y'] + yo)
    tap(d, x, y)
    return True
