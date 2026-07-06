# R1 Spike — Increment 2: App Group echo (FACT-2 / R2 / R4)

**Goal:** prove the extension process and the app process share the App Group container **bidirectionally** via immutable JSON files + a Darwin notification, and settle **R2** (can a content script reach native directly?). No signing, no real UI.

> **✅ Prepared & verified by tooling 2026-07-05 (no device):** files written + the one build blocker fixed (`os_log` signature); `tsc` 0 errors; esbuild emits `content/inpage/background`; `expo prebuild` (SDK 55) copies `VelaAppGroupModule.swift/.m` into the app target (pbxproj ✓); `AppGroupStore.swift` auto-compiles into the extension (synchronized folder); `pod install` done. **Swift not compiled here — the on-device build is your step.** App Group is the **same** `group.app.getvela.wallet` as Inc 1, so **no re-registration**.

---

## Architectural finding (R2) — content script CANNOT reach native directly

On Safari iOS, `browser.runtime.sendNativeMessage` is a **background-only** API. A content script **must relay through the background** service worker (the evictable MV3 one). There is **no** content-script→native bypass. **This refutes ARCHITECTURE.md §12.1.2's "return poll bypasses the JS background" idea** — every native round-trip is gated by the evictable background. The return-path design must move wait/poll state into the **native** side and keep JS↔native as short atomic round-trips with client-side timeouts. (Folded into the architecture separately.)

---

## What was built

- `modules/vela-app-group/ios/VelaAppGroupModule.{swift,m}` — classic RN bridge (mirrors `vela-cloud-sync`, an `RCTEventEmitter` for the `onDarwin` stream): `writeFile/readFile/readAndRemove/list/remove/postDarwin/observeDarwin/unobserveDarwin` over `group.app.getvela.wallet`, atomic writes with `completeFileProtectionUntilFirstUserAuthentication`.
- `src/modules/app-group/index.ts` — JS surface (`NativeModules.VelaAppGroup` + `NativeEventEmitter`).
- `targets/safari/AppGroupStore.swift` — self-contained `Foundation`/`CoreFoundation` helper for the extension.
- `targets/safari/SafariWebExtensionHandler.swift` — echo: on `{type:'echo'}` writes `echo-from-ext-<id>.json`, posts Darwin `app.getvela.wallet.ext-wrote`, reads newest `echo-from-app-*.json`, returns both.
- `packages/safari-extension/src/{manifest.json,background.js,content.js}` — `nativeMessaging` permission + non-persistent background relay; content script gained a floating **"R1 echo"** button (Inc-1 injection kept intact).
- `plugins/with-native-modules.js` — `vela-app-group` added to the source-copy list + `VelaAppGroupModule.*` to the app's compile sources. (App Group entitlement already present from Inc 1.)
- `src/services/dev/app-group-echo.ts` + one line in `src/app/_layout.tsx`'s `if (__DEV__){…}` boot effect — writes `echo-from-app-<id>`, reads newest `echo-from-ext-*`, installs the Darwin observer. Dev-only, no UI.

---

## Build + run

```bash
# (web assets already built into targets/safari/assets; rebuild if you edited src/)
node packages/safari-extension/build.mjs

npx expo run:ios --device        # prebuild + pods already done; just build+install
```

## Trigger the round-trip (order matters)

1. **Launch the app once** (foreground). The `__DEV__` effect writes the first `echo-from-app-*.json` and installs the Darwin observer.
2. Switch to **Safari** → make sure the Vela extension is enabled + allowed on the page → open any `https://` page.
3. Tap the floating **"R1 echo"** button (bottom-right).
4. **Foreground the app again** (so its dev effect re-reads the ext file / the Darwin observer fires).

## Where to read each log

| Log | Where |
|---|---|
| `Vela R1 echo -> background` / `<- native … {type:'echo-ack', wrote, newestFromApp:{…}}` | Safari **Web Inspector** → the **page** console |
| `Vela R2 background … -> native / <- native` | Safari **Web Inspector** → the **service-worker** context |
| `Vela R1 echo: wrote … , replied with app-echo keys …` (Swift handler) | **Console.app** (filter the extension process) or Xcode |
| `[app-group-echo] WROTE app file …` / `READ ext file echo-from-ext-… : {…}` / `DARWIN observed: app.getvela.wallet.ext-wrote` | **Xcode console** (app process) |

## ✅ Increment 2 PASS = all four

> **VERIFIED ON DEVICE 2026-07-05 — all four green.** Content got `echo-ack`; `newestFromApp.source == "app"` (ext read app file); app logged `READ ext file … "source":"extension"`; `DARWIN observed: app.getvela.wallet.ext-wrote`. FACT-2 / R4 confirmed. Proceeding to Increment 3.

1. **Content script got a native response** — Web Inspector shows the `echo-ack` object.
2. **Extension read a file the app wrote** — that response's `newestFromApp` is non-empty (`{source:"app", …}`).
3. **App read a file the extension wrote** — Xcode: `READ ext file echo-from-ext-… : {source:"extension", …}` (after step 4).
4. **Darwin observed cross-process** — Xcode: `DARWIN observed: app.getvela.wallet.ext-wrote`.

If PASS 2 shows `newestFromApp: {}`, you tapped before the app ever wrote — foreground the app once (step 1) and tap again.

## Watch on device (from the critic / research)

- The `CFNotificationCenter` Darwin trampoline + `os_log` (now fixed) are the two Swift edges; if the extension fails to build, check those first.
- Darwin delivers **only to live processes** — that's why step 4 foregrounds the app. A notification posted while the app is fully suspended isn't queued (fine for the echo).
- `deploymentTarget 15.1` + `service_worker` background: robust on iOS 16.4+; if the test device is iOS 15.x and the background misbehaves, switch the manifest to the event-page form `"background": { "scripts": ["background.js"], "persistent": false }`.
