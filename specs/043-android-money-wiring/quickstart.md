# Quickstart: Android Money Wiring

Validation guide. Every step is what a phase's device task runs.

## Prerequisites

- The Xiaomi (`alioth`, serial `9d5f42fb`) on the cable: `adb devices`.
- `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"`
  (the shell's default JDK has no `jlink`; `./gradlew --stop` after a wrong
  daemon).
- Rust 1.97 with `cargo-ndk` and the NDK; uniffi bindings regenerated after
  any core change:

```bash
cd rust && cargo build --release -p vela-core-uniffi && \
cargo run --release -p vela-core-uniffi --bin uniffi-bindgen -- generate \
  --library target/release/libvela_core_uniffi.dylib --language kotlin \
  --out-dir bindings/kotlin --no-format
# debug builds also need the fixtures lib (research D2)
VELA_DEV_FIXTURES=1 bash rust/scripts/build-android.sh
```

## Build and install

```bash
cd app-android/vela-wallet && ./gradlew :app:assembleDebug   # NDK build included, ~4 min cold
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## Open the parallel space

```bash
adb shell am force-stop app.getvela.wallet
adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true
adb exec-out screencap -p > shot.png
adb shell uiautomator dump /sdcard/ui.xml && adb shell cat /sdcard/ui.xml | grep -o 'text="[^"]*"'
```

Expected: the badge on every screen; the receive screen's address is the
fixture Safe the web's `/parallel` derives (see `data-model.md`).

## Drive a send

1. Home → 转账 → pick the fee token's row (Gnosis xDAI in the fixture Safe).
2. Type a recipient (a second fixture account's address) and an amount of
   dust; `input text` for both; tap Continue.
3. Expected on confirm: recipient, amount, a fee in the fee token; no
   biometric prompt when confirming inside the parallel space.
4. Expected on receipt: *submitted* with a hash within seconds; the home feed
   shows the pending row; *confirmed* within the chain's block time.

## Money in flight

```bash
adb shell am force-stop app.getvela.wallet   # right after "submitted"
adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true
```

Expected: the pending row is present; it reaches *confirmed*; with the app
backgrounded instead of stopped, a notification names the outcome.

## Refusals

Drive each of: amount above balance, Max then +1 unit, a non-address, a chain
with no relay coverage, treasury empty (relay test fixture), cancel during
signing. Expected: the core's wording on screen each time, and one prompt at
most per attempt.

## Tests and gates

```bash
cd app-android/vela-wallet && ./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild
cd rust && cargo fmt --all --check && cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings \
  && cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures
node rust/scripts/build-web.mjs && node rust/scripts/build-web.mjs --check   # last, after every Rust edit
node scripts/check-expo-residue.mjs && node scripts/check-native-reachability.mjs
```

## Founder step (SC-002)

Outside the parallel space, the same send from the founder's wallet: one
biometric prompt, one transaction on the explorer.
