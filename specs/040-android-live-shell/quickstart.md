# Quickstart — Android Live Shell (040)

## 0. A fresh checkout cannot build Android

`rust/bindings/kotlin/` and `app-android/.../jniLibs/` are **gitignored** and
consumed in place, so a new worktree has no Kotlin bindings and no `.so`.
Generate both, once, before the first Gradle run:

```bash
cd rust

# The Kotlin bindings (consumed as a source directory by app/build.gradle.kts)
cargo build --release -p vela-core-uniffi
cargo run --release -p vela-core-uniffi --bin uniffi-bindgen -- \
  generate --library target/release/libvela_core_uniffi.dylib \
  --language kotlin --out-dir bindings/kotlin --no-format

# The native libraries (three ABIs; ~6 min cold)
scripts/build-android.sh
```

Symptom if you skip the first: Kotlin cannot resolve `uniffi.vela_core_uniffi.*`.
Symptom if you skip the second: the app installs and dies on first FFI call.

**Regenerate the bindings whenever `bridge_object!` changes** — a new machine is
a new Kotlin class, and Gradle will not conjure it.

## 1. Build and test

```bash
cd app-android/vela-wallet

# Unit tests (the gate that runs on every phase)
./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild

# The APK
./gradlew :app:assembleDebug -PvelaSkipRustBuild
```

`-PvelaSkipRustBuild` skips the 3-ABI NDK cross-compile (~6 min) and uses the
`jniLibs/` already on disk. Drop it when the Rust side changed.

Baseline on this branch's base (`origin/main`): **118 unit tests, green**.

## 2. Run it on a device

```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n app.getvela.wallet/.MainActivity
```

Developer routes (fixture galleries — they stay fixture-fed by design):

```bash
adb shell am start -n app.getvela.wallet/.MainActivity \
  --es vela.startDestination settings-gallery
```

Valid values: `gallery`, `contacts-gallery`, `flows-gallery`, `settings-gallery`,
plus the real routes `wallet`, `contacts`, `settings`.

## 3. Prove the persistence criteria (SC-001…SC-004)

These cannot be asserted on the JVM — "survives a force-stop" needs a device.

```bash
# after changing the currency / adding a network / saving a contact:
adb shell am force-stop app.getvela.wallet
adb shell am start -n app.getvela.wallet/.MainActivity
# then look: the choice is still there
```

To read what was actually written (debug build, device with run-as allowed):

```bash
adb shell run-as app.getvela.wallet \
  cat files/datastore/vela_onboarding.preferences_pb | strings | grep vela.
```

To start from nothing:

```bash
adb shell pm clear app.getvela.wallet
```

## 4. Prove FR-018 (no network)

Two independent checks, both cheap:

1. **By construction** — every network-flavoured operation in
   `contracts/shell-operations.md` is a fail-closed arm carrying `// live in 041`.
   `grep -rn 'live in 041' feature/` enumerates them; the count is the budget
   spec 041 inherits.
2. **On the device** — put it in aeroplane mode and repeat §3. Every scenario
   behaves identically, because none of them was ever reaching the network.

## 5. Measure the bridge (FR-002 / SC-006)

```bash
cd rust
cargo ndk -t arm64-v8a --platform 29 -o /tmp/probe build --release -p vela-core-uniffi
"$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-strip" \
  -s /tmp/probe/arm64-v8a/libvela_core_uniffi.so
stat -f%z /tmp/probe/arm64-v8a/libvela_core_uniffi.so
```

Run it before and after touching `bridge_object!`. The numbers go in
`results.md`; spec 019's recorded gate is **+785,864 stripped bytes** for the
first three machines.

## 6. The ten-second orientation grep

Before believing anything about what is or is not wired:

```bash
cd app-android/vela-wallet/app/src/main/java/app/getvela/wallet
grep -n 'Fixtures\.' navigation/VelaNavHost.kt
grep -rn 'live in 041' .
```

The first says which surfaces are still pictures. The second says which arms
are deliberately fail-closed. Neither is a memory, and both take ten seconds —
which is less than one wrong assertion costs.
