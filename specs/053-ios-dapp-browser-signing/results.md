# Results — 053 iOS dApp Browser and Signing

## Baselines, taken at the branch point (`143b9b11`)

| | at 053's start | at closeout |
|---|---|---|
| `@Test` declarations | 402 | |
| hermetic test run | 371 | |
| XCUITest methods | 20 | |
| literal-audit violations | 35 | |
| `vela_core_uniffi.swift` bytes | 353,772 | |
| `// live in 05x` markers | 0 | |
| Swift files referencing any of the six machines | **0** | |
| WebKit imports in the app | **0** | |

The baseline is taken at **this cut's branch point**, not at `origin/main`:
050, 051 and 052 sit in between and their work is not this cut's.

## Phase 0 — the papers

The finding that shapes the cut, again: **all six machines are already exported
and already in the committed Swift bindings**, together with every free
function the executors need (`dappOriginOf`, `dappIsSigningMethod`,
`hashTypedData`, `keccak256`, `safeMessageHash`, `eip1271Signature`, the
Multicall3 pair, `decodeCalldata`, `matchSelector`). Research D0. So this cut,
like 052, adds no bridge line and regenerates no bindings.

The second finding is how much is **already drawn**: E1–E7 with twelve
components, cs1–cs33 with five, the connection panel, the allowance editor
whose "as requested" chip is already *disabled* rather than unselected for an
unlimited request, the slide, and the technical-details fallback. 2,468 lines
of screen with nothing behind them.

The third is where the seam is: **one view**. `ExploreScreen`'s `.browsing` arm
draws `ScrollView { DemoPageView }`, and that model's own comment has said
since spec 021 that a real `WKWebView` replaces it wholesale.
