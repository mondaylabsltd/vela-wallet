# Results — 044 Android dApp Browser and Signing

Per phase: what changed, what the tests prove, what the device showed
(`uiautomator` text, screenshot names in the session's scratchpad), and the
defects only the device could show.

## Baselines (T001)

| Artefact | At 043's tip (`0d4a7a9e`) |
| --- | --- |
| `libvela_core_uniffi.so` arm64-v8a | 16,187,032 bytes |
| Machines driven by Android | 16 of 26 |
| Android unit tests | 431, 0 failures |
| Xiaomi system WebView | `com.google.android.webview` 151.0.7922.200 (document-start scripts need ≥ 90) |
| Ceiling for the six machines (plan) | ≤ 19.5 MB arm64 |

## Phase log

### Phase 0 — the bridge grows six machines (T001–T009)

**What changed**: `bridge_object!` for `DappPermissionsCore`,
`ExploreSitesCore`, `BrowserHistoryCore`, `SignRequestCore`,
`ClearSigningCore`, `ApprovalGuardCore`; two rules exported so the shell
never re-types them: `dapp_origin_of` (the grant key and the one fact about
a page the shell attaches) and `dapp_is_signing_method` (the routing
table's first question). Six Kotlin wires (`DpermWire`, `ExploreWire`,
`BhistWire`, `SignWire`, `ClearWire`, `GuardWire`, 1,370 lines) with every
numeric type read from the Rust struct, not the mirror (`u32` → `Int`,
`f64` → `Double`, `i32` codes).

**Tests**: `CoreWireDriftTest` +5 cases over 6 families — 58 assertions
green (views subsets; operations, results, page events, payloads,
outcomes, closed reason/kind/surface unions exhaustive; event payload
fields present). `BridgeSmokeTest`: each machine created, first event,
view, no fault; the origin rule (`https://app.uniswap.org/swap?a=1#x` →
`https://app.uniswap.org`, `HTTPS://user@Example.com:443/` →
`https://example.com`, `about:blank` → none).

**Bridge size**: arm64 `.so` 16,187,032 → **18,892,528** (+2,705,496,
+2.58 MB) for six machines and two exports — under the 19.5 MB ceiling.

**Found while running**: the history machine records nothing before its
store has answered (`Start → ReadHistory → Loaded`), and `BhistView` has no
"ready" flag — a visit dispatched a millisecond after `Start` is dropped.
The controller must wait for the load (the executor's answer is the
signal) before recording the first visit; the smoke test does.
