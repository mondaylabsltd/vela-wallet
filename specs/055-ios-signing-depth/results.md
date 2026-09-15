# Results — 055 iOS Signing Depth

## Baselines

| | at 055's start | now |
|---|---|---|
| hermetic test run | 473 in 58 suites | **523 in 66 suites** |
| XCUITest methods | 33 | **35** |
| literal-audit violations | 35 | **35** |
| `vela_core_uniffi.swift` bytes | 353,772 | untouched |

## Phase 0 — the pure pieces

`SimDeltas`, `SignedDigits`, `Eip681`, `QrDecoder`, and
`NSCameraUsageDescription`. Each with its own tests, so phase 1 could be about
the screen rather than about arithmetic.

**D1 changed while writing it.** Vision was chosen because
`VNDetectBarcodesRequest` reads a sample buffer and a still image alike. It does
— on a device. In the simulator it reads NEITHER: a code this app itself
rendered came back with zero observations and no error, and `CIDetector` read it
correctly in the same test. A decoder the hermetic suite cannot exercise is a
decoder nobody can prove works, so the decoder is CoreImage's. It keeps the
property that mattered — one decoder for the camera and the library both — and
the round trip is now a test, square and rotated.

## Phase 1 — the sheet says what moves

The drawn `balances` block has existed since 022 and nothing had ever filled it.

The asymmetry is the core's and it now has a test on both sides: the same token
with the same metadata renders "−5 USDC" leaving and "unverified token · +"
arriving, because an outflow cannot be understated (the real token emits its own
log) and an inflow's amount is whatever the site chose to emit.

`simulation` is a TRI-state, because three outcomes are three sentences —
judgments, "checked, nothing moves", and the danger-toned "could not check". The
third is the one that matters: a wallet that stays quiet when it could not look
teaches people that silence means safe.

## Phase 2 — message depth

The SIWE machinery landed with 053 and nothing had driven it. Five tests do now,
through the real core, asserting on the RENDERED sentence rather than the key —
Android 044 shipped a phishing warning whose placeholder never interpolated.

Two rows the core parses and the sheet was dropping: the chain the message names
and its nonce. They are shown as FACTS — the core binds on the domain, not the
chain, and a shell that decided "this chain is wrong" would be a second,
disagreeing adjudicator.

## Phase 3 — the scanner

`ScanSurfaceView` was four brackets around a grey rectangle. It has a camera
now, one decoder, four refusals with four sentences, a torch, a lens flip and
the photo library.

**A fifth session rule, found by a test that hung for ten minutes:** asking for
permission before checking a camera exists. On a simulator `requestAccess` never
answers. Asking permission for hardware that does not exist is absurd on its
face, and a viewfinder that waits forever with nothing on screen is the worst of
the four refusals.

**Two dropped judgement fields wired:** `lock_error` and `add_network_msg`. A
scanned code for a chain this wallet does not have used to land on the token
picker with nothing at all to say why — the core had computed the reason and
this client was not reading it.

**One drawing restored:** the single-send form's recipient row had no scan
button, while its own sweep sibling has drawn one since 021. Without it the
scanner was reachable only from inside the address book.

## Success criteria

| | claim | verdict |
|---|---|---|
| SC-001 | a contract call's balance block matches the receipt | **test-only** — `SimulationSheetTests` drives the real core end to end; matching a LIVE receipt needs a live contract call and is owed with the 052 live sends |
| SC-002 | an unverified inflow renders direction and name and **no amount** | **test-only** — asserted in both directions, which is the security property |
| SC-003 | a transaction that cannot be simulated says so, distinctly | **test-only** — three states, three sentences, one test each |
| SC-004 | a SIWE message whose domain differs from the asking origin warns | **test-only** — `MessageDepthTests`, on the rendered sentence |
| SC-005 | the camera reads this wallet's own receive code | **half device-verified** — the scanner opens on the phone and asks for the camera with this app's own sentence (screenshot in `/tmp/depth.xcresult`); **holding a code up to the lens needs a hand**, and the decoder's round trip is proved hermetically |
| SC-006 | a code naming an unknown chain offers to add it | **test-only, and partly deferred** — the refusal is now SAID (`ScanLockTests`); the core's 添加该网络 affordance has no drawn home on this client and is recorded below rather than invented |
| SC-007 | a refused camera permission explains itself and offers the library | **device-verified** — `testTheScannerShowsACameraOrSaysWhyNot` and `testTheScannersToolsAreAllThere` both pass on the iPhone 11; the three tools are present whichever way the permission goes |
| SC-008 | nothing simulated ever writes a token into anybody's list | **test-only** — `nothingSimulatedEverEntersTheTokenList` asserts it from the shell's side, which is where the temptation lives |

**Device evidence:** `/tmp/depth.xcresult` — 2 of 2 passed on the iPhone 11
(`00008030-001A75961445802E`).

## Needs a hand

1. **Tap 允许 on the camera prompt once.** Until then the viewfinder cannot run
   on that phone, and every later run shows the permission refusal instead of a
   picture. The test passes either way and records which.
2. **Hold a receive code up to the lens** — SC-005's other half.
3. A live contract call whose receipt can be compared to its own balance block
   (SC-001), which spends money and belongs with the 052 live sends.

## Recorded, not fixed

1. **添加该网络 has no drawn home.** `send::AddNetworkTapped` exists and the
   corpus has the words; no iOS screen draws the button. The refusal is said,
   the offer is not made. Needs a drawing.
2. **The chain binding is the core's to add.** A SIWE message naming chain 1
   presented on chain 100 shows both numbers and adjudicates neither. Making
   that a verdict means changing `clear_signing`, which this program does not do.
3. **`Info.plist` usage strings are English in a Chinese app.** The camera
   sentence joins the photo-library and Bluetooth ones, which have always been.
   A localised `InfoPlist.strings` is a separate, whole-app task.
4. **`send.sim_json` is still unread**, as on every other client. Recorded in
   054 and unchanged.
