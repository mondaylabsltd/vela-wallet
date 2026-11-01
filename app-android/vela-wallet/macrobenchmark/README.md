# Android performance journeys (native parity Phase 4)

Two `com.android.test` modules drive `:app`'s **`benchmarkRelease`** variant
(release code, debug-signed, profileable; created by the
`androidx.baselineprofile` plugin):

| Module | What it does |
|---|---|
| `:macrobenchmark` | `StartupBenchmark` — cold start to the cached home (TTID, TTFD, the launch animation's span, memory), shipped vs no-profile compilation. `JourneyBenchmarks` — 200-row history scroll, Send → Confirm, signing sheet open (frame timing = jank, memory). |
| `:baselineprofile` | `BaselineProfileGenerator` — the same journeys, collected into `app/src/main/generated/baselineProfiles/{baseline,startup}-prof.txt`. |

The journeys themselves live once, in `src/journeys/java/.../perf/Journeys.kt`.
They need a returning person, which a release-shaped build cannot be given
through the debug launch extras, so the benchmark build types carry two doors
(`app/src/benchmark`, never in `release` — `scripts/check-release-apk.sh`
fails a release APK that has them):

- `BenchSeedActivity` writes a **view-only** fixture wallet: the parallel
  space's golden 1-of-3 Safe (`0x88cC…6894`, deployed on Gnosis) with the
  fixture keys' PUBLIC halves only, a fresh holdings cache, and 200 activity
  records. A benchmark build has no signer for it.
- `BenchSignActivity` opens a `personal_sign` signing sheet, offline.

Send → Confirm asks the live relay for a quote (the journey retries a refusal a
few times); the other journeys need no network beyond the home's own reads.

## Run (one device only)

Gradle's `connected*` tasks run on **every** attached device — a phone on the
cable is somebody's wallet. Use the script, which installs and runs on
`ANDROID_SERIAL` alone:

```sh
cd app-android/vela-wallet
./gradlew :app:assembleBenchmarkRelease :macrobenchmark:assembleBenchmarkRelease -PvelaSkipRustBuild
ANDROID_SERIAL=emulator-5554 scripts/run-macrobenchmarks.sh build/macrobenchmark-results
#   one test:  … scripts/run-macrobenchmarks.sh out 'app.getvela.wallet.macrobenchmark.StartupBenchmark'
python3 scripts/summarize-macrobenchmarks.py build/macrobenchmark-results
```

On a **physical phone** — which may hold the person's own
`app.getvela.wallet` — build with `-PvelaDeviceTest` so the app under test is
`app.getvela.wallet.devicetest`, installed beside theirs; the script refuses
the plain id on a physical device and passes the id it installed to the
journeys (`vela.targetPackage`):

```sh
./gradlew :app:assembleBenchmarkRelease :macrobenchmark:assembleBenchmarkRelease -PvelaSkipRustBuild -PvelaDeviceTest
ANDROID_SERIAL=<phone> scripts/run-macrobenchmarks.sh build/macrobenchmark-results
adb -s <phone> uninstall app.getvela.wallet.devicetest   # afterwards; and app.getvela.wallet.macrobenchmark
```

The JSON (`*-benchmarkData.json`) and one Perfetto trace per iteration land in
the output directory; open a trace at ui.perfetto.dev. A journey step that
never arrives leaves `journey-failure-*.png` / `.xml` beside them.

Memory as Android vitals reads it (PSS), after the journeys:
`adb -s $ANDROID_SERIAL shell dumpsys meminfo app.getvela.wallet | grep -E "TOTAL PSS|TOTAL:"`.

## Regenerate the baseline profile

After a change to startup or to a journey's screens (needs API 33+, or a
rooted device below that; emulators are fine), on ONE device:

```sh
ANDROID_SERIAL=emulator-5554 scripts/generate-baseline-profile.sh
git add app/src/main/generated/baselineProfiles/baseline-prof.txt \
        app/src/main/generated/baselineProfiles/startup-prof.txt
```

The script also writes startup-prof.txt (the launch journey's classes), and
since release is R8-optimized (nat-android-6) it is checked in: AGP lays out
the primary dex by it (`baselineProfile { dexLayoutOptimization = true }`),
which it does only for an optimized build. The generator runs against
`nonMinifiedRelease`, so the rules name the original classes; R8 rewrites
both profiles for the names it gives them.
The plugin's own generateBaselineProfile task does the same, but on every
attached device.

## Budgets (Phase 4)

TTID ≤ 1.5 s, TTFD ≤ 2.0 s (Pixel 7 / API 34 class, `shipped` compilation),
jank < 1 % of frames, PSS ≤ 220 MB, `libvela_core_uniffi.so` arm64 ≤ 19.5 MB
(`scripts/check-release-apk.sh`). Emulator numbers are indicative only — they
compare runs on one machine, they are not a device's figures.

## CI (not wired yet)

A job on an API 34 `google_apis` x86_64/arm64 emulator (e.g.
`reactivecircus/android-emulator-runner`) would: build the Rust core for the
emulator's ABI (`rust/scripts/build-android.sh`), assemble the two
`benchmarkRelease` APKs, run `scripts/run-macrobenchmarks.sh` with
`ANDROID_SERIAL=emulator-5554`, upload the output directory, and fail on
`summarize-macrobenchmarks.py` numbers over budget once a runner's figures are
known to be stable. Non-blocking until then: shared CI emulators are noisy.
