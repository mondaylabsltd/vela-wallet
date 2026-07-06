# Vela Safari-extension E2E regression (Appium + WebDriverAgent)

Fully-automated on-device regression for the R1 sign return-path (the spike proven
2026-07-05, conditional GO). Drives Safari + the native app on a physical iPhone
and reads the page-side verdict. Reusable to catch regressions in the sign
round-trip (injection → launch → sign → return → result reaches the page).

## What it proves (the 4 fund-safety invariants)
- **(a) never silently lose** a result — `run_matrix.py` rows `3 kill` (app killed
  right after sign) and `5 evict` (background worker killed by 65s idle) must
  still resolve to `submitted` or `check-vela`.
- **(b) never false-decline** — `4001`/rejected may appear ONLY on the `8 reject`
  row; any other row showing 4001 is a hard fail.
- **(c) never hang** / **(d) no double** — implicit in the bounded poll + single
  resolve; regression shows as `FAIL(None)` or a duplicated verdict.

Current state: `1 happy, 8 reject, 3 kill, 5 evict` PASS; `7 reload` is a KNOWN
non-fund-safety gap (the reloaded page's `storage.local` re-arm doesn't re-show
the verdict — result still persists + is retrievable).

## Prerequisites (one-time)
1. **Build a RELEASE build** of Vela on the device (the dev-client Expo launcher
   breaks deep-link automation and can't be cleanly killed):
   `npx expo run:ios --device <UDID> --configuration Release`
2. **Enable the Safari extension**: iPhone Settings → Safari → Extensions →
   Vela Wallet → ON, permissions → All Websites → **Allow**.
   *(A Release reinstall resets this — re-enable after every rebuild.)*
3. **Appium + WebDriverAgent** (per-machine):
   ```bash
   npm i appium                          # local, or -g
   npx appium driver install xcuitest
   brew install ios-webkit-debug-proxy   # Safari web automation on real devices
   python3 -m venv venv && ./venv/bin/pip install -r requirements.txt
   ```
   WDA builds+signs automatically from the caps (`xcodeOrgId`, `updatedWDABundleId`)
   on first run; the F9W689P9NE dev cert is already trusted on the device because
   the app is signed by the same team (no manual trust step).

## Run
```bash
# 1. start the Appium server (leave running)
./node_modules/.bin/appium --port 4723 --relaxed-security

# 2. in another shell, from this dir:
export VELA_UDID=00008030-001A75961445802E   # `xcrun xctrace list devices` or ios-deploy -c
export VELA_TEAM=F9W689P9NE
./venv/bin/python check_injection.py         # sanity: extension injecting?
./venv/bin/python run_matrix.py              # the 5-row matrix (~4 min, incl. 65s evict)
```

Env overrides: `VELA_UDID`, `VELA_TEAM`, `VELA_WDA_BID`, `VELA_APP_BID`,
`VELA_TEST_URL` (default example.com — must be a site the extension is granted on),
`VELA_APPIUM` (default http://127.0.0.1:4723).

## Gotchas learned (so you don't re-discover them)
- **RN Pressables are `XCUIElementTypeOther`; `element.click()` and `mobile:tap`
  do NOT fire onPress reliably** — but a coordinate `mobile: tap` at the element
  center DOES (native iOS `Button`s like the scheme banner respond to taps too).
  We read the verdict from an injected DOM element, not `browser.storage.local`
  (an extension API not reachable from the page's main world / Appium `execute_script`).
- **`newCommandTimeout` must be ≥ the evict idle** (set to 300s in `lib.py`) or the
  Appium session dies during the 65s wait.
- **Dev build won't work** — the Expo launcher intercepts `velawallet://` and the
  app can't cleanly terminate. Use a Release build.
- **`page_source` snapshots** (not live element handles) avoid `StaleElementReference`
  churn on re-rendering RN screens.

Note: `venv/` and any Appium `node_modules/` are gitignored — only the scripts are
tracked. The scripts fake the sign (spike); once the real SigningRequestModal +
passkey + bundler are wired in (ARCHITECTURE §12), the same matrix validates the
real path (add a fixed-key/parallel-space signer so it runs unattended).
