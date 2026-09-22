# Quickstart — how each fix in 081 is proved

Two questions per fix: does it work, and did it break anything. Automated where possible (FR-018), on a real device where it touches a native shell (FR-019: Xiaomi `9d5f42fb`, iPhone 11 `ABC`).

## Standing gates (run before every PR is called done)

```bash
# core + conformance replay (Kotlin, Swift, shipped wasm)
cd rust && cargo test --workspace --features vela-core/i18n-all && cargo clippy --workspace --all-targets -- -D warnings

# web wallet + site
cd app-web/vela-wallet && pnpm check && pnpm test
cd app-web/getvela.app  && bun run check && bunx vitest run && bun run i18n:status --gate && bun run build

# desktop
cd app-desktop/vela-wallet && cargo test && cargo clippy --all-targets -- -D warnings

# Android / iOS
cd app-android/vela-wallet && ./gradlew :app:testDebugUnitTest
cd app-ios/VelaWallet && xcodebuild test -scheme VelaWallet -destination 'platform=iOS Simulator,name=iPhone 16'

# cross-shell parity
npm --prefix scripts run check:expo-residue
node scripts/check-ios-android-parity.mjs && node scripts/check-android-event-parity.mjs   # payload-aware after WP5
```

## Per requirement

| FR | Proves it works | Proves nothing broke |
| --- | --- | --- |
| **005/006** self-call guard | Core tests: each of the 13 selectors alone, in a batch leg (names the leg), nested in MultiSend, inside `execTransaction`, smuggled via `params_override_json`, and a `SafeTx` typed-data request. Per shell: the test dApp's new buttons show the blocked sheet with its explanation, and cannot be signed | Negative tests: empty-data self-transfer, view selector to self, ordinary dApp call. A **real send and a real dApp swap on each shell** (device for Android/iOS) still work; existing unlimited-approval tests stay green |
| **001** iOS endpoints | On the iPhone: set each of the four endpoints, kill and relaunch — values persist, badges reflect the real health check, Reset restores defaults | Android/web/desktop endpoint tests unchanged; new payload-parity script run catches any other field mismatch |
| **002** index everywhere | With a stub index host recorded: create a wallet (web), sign in (web, iOS), start signed-in (desktop), look up a name (Android, iOS, desktop) — every request goes to the stub | No path silently falls back to Vela's index when the stub is unreachable: it fails visibly (on-chain registry walk is allowed) |
| **007** X-Rpc-Url | grep: no shell sets it; a relay request captured on each shell carries no RPC URL header | Send still works on all four shells (device for native); the web simulator's tevm fork still seeds from `best_rpc_url` |
| **004** guide links | Desktop and web settings open `getvela.app/docs/self-hosting` | — |
| **008** provenance | Core tests for each provenance value; a fetched-only descriptor shows the unauthenticated label; a fetched descriptor equal to the built-in shows "Verified" | Permit/Permit2 sheets still show "Verified" (pinned built-ins); desktop's `partial` no longer says "verified ABI"; web's token-shape decode no longer says "selector not listed" |
| **009** readiness | Core tests: a chain missing only the signer factory is `compatible && !multi_key_ready`; the site's drift test still matches the Rust list | A currently-added network keeps working for single-key wallets; chain-setup page still deploys what anyone can deploy |
| **010** names | Core rule tests (verified, mismatch, unavailable); per shell, a reverse record that does not resolve back shows no name | A normal ENS name still shows on all four shells; contacts and the activity feed still show saved contact names |
| **011** network count | Corpus says 24 in all 15 locales; extension manifest and packaging metadata agree | Six-step i18n gate passes, including `dump:vectors` and the wasm rebuild |
| **012/013** releases | A release candidate: `gh attestation verify <artifact> --repo mondaylabsltd/vela-wallet` passes for every attached artifact; a flipped byte fails. macOS dmg: `xcrun stapler validate` + `spctl -a -t open -vv` | Release workflow still publishes the same artifacts; download page and checksums unchanged |
| **014** iOS privacy | Archive → Xcode privacy report lists every required-reason API; `plutil -lint` passes; `nm -u` shows no undeclared category | App still builds and runs on the iPhone; the uniffi-bindgen split does not change the generated bindings (conformance replay) |
| **015** routes | The five paths return 404 on a built preview | `og`, `downloads`, `bug-report`, `exchange-rate` still work; site build and crawl clean |
| **016** feedback | Web: a report reaches the issue tracker (or, with the endpoint unconfigured, opens a prefilled issue with the fields populated by id) | Nothing in the payload is an address, balance, endpoint URL or raw `vela.*` value — asserted by a test |
| **017** erase | Desktop and iPhone: create wallet, add a contact, browse a dApp, erase — a storage scan finds only the keep-list, the app restarts as new | Web and Android erase still pass their tests; `vela.pendingUploads` survives everywhere |
| **003** service repos | `docker build` from a clean clone of each repo succeeds; a relay and an index start from `.env.example` values alone | Existing deployments unaffected (defaults unchanged) |
| **020** docs | Each closed gap's disclosure is gone from the site (15 locales), README, ARCHITECTURE and the 080 ledger | `i18n:status --gate` 0 stale; link crawl clean; no disclosure removed for a gap still open |

## Device runs (FR-019)

Recorded in `results.md` with date, device, build and what was exercised:

- **Android (Xiaomi 9d5f42fb)**: blocked self-call from the test dApp; a real send; a name lookup against a custom index; erase.
- **iPhone 11**: endpoints page save/persist/reset; sign-in against a custom index; blocked self-call in the in-app browser; erase including dApp browser data; archive privacy report.
