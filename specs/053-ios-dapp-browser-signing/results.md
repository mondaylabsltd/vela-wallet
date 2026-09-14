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

## Phase 0 code — the wires, the drift gate, the provider bundle

Nine new files, six machines mirrored, the routing table ported, and the
provider copied out of the web tree by a build phase. **No screen changed.**

### Two findings

**① Android's "an untold machine refuses every chain" is wider than the truth.**
Research D6 there says a `sign_request` that has not been told the network set
refuses **every** transaction with 4902. It does not. The chain check runs only
against a chain the **request** names — a stamped `per_request_chain`, or a
`chainId` embedded in the params — and a plain `eth_sendTransaction` carrying
neither reaches the sheet whatever the machine has been told
(`sign_request.rs:1232-1246`).

That makes the seeding order matter for a narrower and sharper reason: a dApp
that names its chain, which every multi-chain dApp does, is refused until
`networks_changed` has landed. Believing the wider claim would have sent
somebody hunting for a phantom bug on the requests that do work. The test now
asserts the real rule and says so in its own doc comment.

**② A parity test can pass against a set it never read.** The first version of
`theReadAllowlistMatchesTheSharedScript` anchored on the constant's **name**,
and `protocol.js` names each of these in a header comment before declaring it —
so `BUNDLER_METHODS` matched the comment on line 21 and collected whichever
array came next. Two of the three sets compared correctly by luck; the third
compared an unrelated one and would have gone on passing while the real set
drifted. It is anchored on `const <NAME> = ` now.

That is the same shape as 052's lesson — **a bad test that passes is worse than
no test** — and it is worth noticing that it recurred in a *different* form
within one cut of the other.

### The state of the six machines

| machine | wire | drift test | executor |
|---|---|---|---|
| `explore_sites` | `ExploreWire` | ✅ | `ExploreExecutor` (2 ops) |
| `browser_history` | `BhistWire` | ✅ + the dropped-visit proof | `BhistExecutor` (3 ops) |
| `dapp_permissions` | `DpermWire` | ✅ | `BrowserExecutor` (8 ops) |
| `sign_request` | `SignWire` | ✅ + the 4902 proof | `SignExecutor` (7 ops) |
| `clear_signing` | `ClearWire` | ✅ | `ClearExecutor` (5 ops) |
| `approval_guard` | `GuardWire` | ✅ | `GuardExecutor` (3 ops) |

The `browser_history` drift test does more than decode: it **proves the drop**.
A visit dispatched before the load lands is gone, and the same visit after it
is kept. The controller's wait is a tested rule rather than a comment.
