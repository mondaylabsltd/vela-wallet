# 055 — iOS Signing Depth

**Branch:** `055-ios-signing-depth` (stacked on `054-ios-send-contacts-parity`)
**Mirrors:** Android 046, desktop 035 + 036 + 037
**Machines:** `token_trust::SimDeltasComputed` · `clear_signing::MessagePresented`
(deepened) · `send::OpenScanner` / `ScanResolved` · `network_admin::AddByChainIdRequested`

## Why this cut

053 put a signing sheet in front of a dApp and 052 put money through it. Both
show what is being signed **from the calldata alone**. Three things a person
needs before they sign are still missing on iOS:

1. **What the transaction will actually do to their balances.** The drawn sheet
   has a `balances` block; nothing has ever filled it. The core has the
   judgment (`token_trust::judge_delta`, asymmetric by design); what is missing
   is the shell half — an `eth_simulateV1` call and the netting of its logs.
2. **Depth on message signing.** `personal_sign` reaches the core and renders,
   but a SIWE message's domain / URI / chain binding is not checked against the
   page that asked, so the one attack this format exists to prevent — a site
   asking somebody to sign another site's login — is invisible.
3. **The camera.** `ScanSurfaceView` is a drawn rectangle. Every scanner entry
   point in the app leads to it, and none of them can read a code.

## User stories

### US1 — the sheet says what moves (P1)

Signing a transaction shows the balance changes the simulation found, judged by
the core: a native move always, an outflow whenever the token resolved, and an
INFLOW of an unverified token as direction and name **without a number**.

A transaction that cannot be simulated says so, in a distinct block from "we
simulated it and nothing moves" — those are different facts and collapsing them
is how a wallet learns to say "safe" about something it never looked at.

**Never a write.** A simulated delta may never add a token to anybody's list;
that entrance is the confirmed receipt's alone (spec 017, invariant ⑤).

### US2 — a message is checked against the page that asked (P1)

A SIWE message's `domain`, `uri` and `chainId` are compared to the origin and
chain of the request. A mismatch is a phishing warning in the sheet, in the
core's words. `eth_sign` keeps its existing refusal in the built-in browser and
its danger face everywhere it is reachable.

### US3 — the camera reads a code (P1)

The scanner surface gets a real preview, a torch, a lens flip and the photo
library. A decode becomes `send::ScanResolved` and the CORE decides what it
means — which token, which chain, how much. A code naming a chain this wallet
does not have becomes `network_admin::AddByChainIdRequested`.

Permission refused, no camera, and restricted are three different answers and
each gets its own sentence.

## Success criteria

| | claim |
|---|---|
| SC-001 | a contract call's balance block matches what the receipt later shows |
| SC-002 | an unverified inflow renders direction and name and **no amount** |
| SC-003 | a transaction that cannot be simulated says so, distinctly |
| SC-004 | a SIWE message whose domain differs from the asking origin warns |
| SC-005 | the camera reads this wallet's own receive code and locks the send |
| SC-006 | a code naming an unknown chain offers to add it |
| SC-007 | a refused camera permission explains itself and offers the library |
| SC-008 | nothing simulated ever writes a token into anybody's list |

## Out of scope

The app's own number and date preferences inside a signing panel (056), the
Safari extension (its own spec), and any change to `vela-core`.
