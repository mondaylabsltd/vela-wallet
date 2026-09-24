# Android: the install / verify loop (specs 040–047)

The loop the program ran on every phase, written down for the next person.
One connected phone (a Xiaomi alioth, serial `9d5f42fb`, was the program's),
the parallel space, and `adb`.

## Build and install

```sh
# 1. Rust bindings (gitignored; needed by the JVM tests and the device build)
cd rust && cargo build --release -p vela-core-uniffi && \
  cargo run -q -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
    --library target/release/libvela_core_uniffi.dylib --language kotlin \
    --out-dir bindings/kotlin --no-format
# 2. The app (Android Studio's JBR has jlink; a stale daemon on another JVM fails)
cd ../app-android/vela-wallet
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
./gradlew --stop
./gradlew :app:assembleDebug                 # full: rebuilds the NDK .so (~4–6 min)
./gradlew :app:assembleDebug -PvelaSkipRustBuild   # Kotlin-only change (~1 min)
adb -s <serial> install -r -d app/build/outputs/apk/debug/app-debug.apk
```

Every Rust edit also moves the web wasm fingerprint: `node rust/scripts/build-web.mjs`
then `--check` before committing.

## The parallel space

```sh
adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true
```

A debug-only fixture keyset signs for the golden Safe `0x88cC…6894` (Gnosis,
a little XDAI); the badge at the top right says it is on. The flag lives in
`shared_prefs/vela.parallel.xml`; `--ez vela.parallelSpace false` leaves it.
Real money on a real chain, so keep the amounts to dust (0.001 XDAI).

## Driving the screen

- `adb shell uiautomator dump /sdcard/ui.xml` lists on-screen nodes (text,
  content-desc, bounds); WebView text is often invisible to it.
- `input tap x y`, `input text …`, `input swipe …`, `screencap -p`.
- The keyboard covers buttons: check `dumpsys input_method | grep mInputShown`,
  BACK closes it. Tall sheets: scroll before reading the bottom.
- The send confirm's CTA is a button, not a slider; the signing sheet's is a
  slider (swipe along the wide node at the bottom).
- Pages in the in-app browser: `WebView.setWebContentsDebuggingEnabled(true)`
  is on in debug; `adb forward tcp:9222 localabstract:webview_devtools_remote_<pid>`
  and a WebSocket `Runtime.evaluate` (the program's `devtools.mjs`).
- The test dApp: `python3 -m http.server 8137` in `dev/testdapp`,
  `adb reverse tcp:8137 tcp:8137`, then
  `am start -n app.getvela.wallet/.MainActivity --es vela.openUrl http://127.0.0.1:8137/`.
- Files for the picker: `adb push f /sdcard/Download/f`, then
  `am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///sdcard/Download/f`;
  DocumentsUI's search finds it.
- Deep links: `am start -a android.intent.action.VIEW -d 'velawallet://pay?to=…&chain=100&amount=0.001'`.
- A forced crash (debug): `--ez vela.testPanic true`; the next launch shows the sheet.
- The store: `adb shell run-as app.getvela.wallet cat files/datastore/vela_onboarding.preferences_pb | strings`.
- Galleries: `--es vela.startDestination contacts-gallery` (also `settings-gallery`,
  `flows-gallery`, `gallery`), `--es vela.settingsState ST7`.

## Gates before a commit

```sh
./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild   # JUnit, incl. CoreWireDriftTest
node scripts/check-native-reachability.mjs
node rust/scripts/build-web.mjs --check
node scripts/check-android-event-parity.mjs            # the two rulers (047)
node scripts/check-android-dropped-judgement.mjs
ls -la app-android/vela-wallet/app/src/main/jniLibs/arm64-v8a/libvela_core_uniffi.so   # ≤ 19.5 MB
```

## Where the findings live

`specs/04x-*/results.md` — one per spec, phases, device evidence, what is owed.
