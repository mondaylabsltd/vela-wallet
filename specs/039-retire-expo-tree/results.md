# Results: Retiring the Expo tree

Written as the work lands (per 033–038). Branch `039-retire-expo-tree`,
off `main` @ 936f1b3f.

## Baseline @ 936f1b3f — 2026-09-11

Every gate run on the branch tip *before* the first deletion commit, so a
later difference can be attributed.

| Gate | Result | Note |
| --- | --- | --- |
| app-web `pnpm check` | green | tokens drift, wasm sync, generated types, svelte-check: 0 errors |
| app-web `pnpm lint` | **red — pre-existing** | prettier: `src/lib/flows/FlowsMobile.svelte`, `src/lib/services/endpoints.ts` unformatted on `main`; eslint not reached. Not this feature's; left as found |
| app-web `pnpm test:unit` | 941 passed / **2 failed — pre-existing** | `src/lib/signing/fixtures.test.ts:48` (catalogue 35 ≠ 33) and one sibling, both carried by `main` since 038 (its results.md names the same two) |
| app-web `pnpm build` | green | prerendered all locales, built in 7.75 s |
| rust `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | green | every `test result: ok`, no FAILED |
| desktop `cargo fmt --check` | green | |
| desktop `cargo clippy --all-targets` | green with warnings | 40 warnings, the same families the CI header records as the known baseline; not `-D warnings` by design |
| desktop `cargo test` | green | 395 passed, 37 ignored |
| root tooling gates (`build-web --check`, `verify-web`, `dump:vectors` + diff, identicon table + diff, `gen-i18n` + diff, corpus lint, Lottie self-test + lint, native reachability, identicon parity, i18n parity, `gen-core-types --check`, `vela-sdk` build) | green | no error output from any of the thirteen; `git status` clean after the three regenerate-and-diff gates. Exit codes were not captured by the runner (zsh `pipestatus`), so each is re-run with its code after the cut |
| android `./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild` | green | BUILD SUCCESSFUL in 57 s (host dylib + `rust/bindings/kotlin` already on disk) |
| ios `xcodebuild build -scheme VelaWallet -destination 'generic/platform=iOS Simulator'` | green | exit 0, no errors (xcframework already on disk) |
