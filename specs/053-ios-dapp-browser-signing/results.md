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

## Phase 1 — the engine

**Device-verified on the iPhone 11.** The 探索 tab runs a real `WKWebView`, the
page finds this wallet before its own scripts run, and the address bar tells
the truth about where it is.

```
#verdict announce Vela Wallet app.getvela
#verdict legacy present isVela=true
```

`BrowserAcceptanceTests` 2 of 2. The harness is served by an `NWListener`
inside the UI-test runner on `127.0.0.1:8137`, with keccak-256 **vendored**
into the page: Android's phase 5 lost its on-chain check to a CDN script that
would not load on the phone, and a harness that needs the internet to verify a
signature fails for the wrong reason.

### The defect the first run found

**A page loaded, ran, and was invisible.** The provider announced itself, the
history recorded the visit, the engine reported its URL — and the screen went
on drawing the start page over it, because `ExploreScreen.view` came from the
FIXTURE (`model.view`, which is `.start` for E2) and only a tap could override
it.

This is the same shape as 052's empty token picker and Android's empty contact
picker, one layer further out: the machinery was right and the surface was
reading somebody else's answer about what to show. The live screen now asks the
**tabs**: a selected tab with a page means the browsing view, and none means
the start page. The fixture's `view` still drives E1–E7.

### Recorded, not fixed

**There is no corpus sentence for "this site is not secure".** `explore.*` has
`secureSite` and no counterpart, so an http page is described by its host and
by the absence of the padlock rather than by a phrase invented in the shell —
a security claim in a language nobody translated is worse than no claim. The
padlock's absence is asserted on the device
(`testTheAddressBarShowsThePagesOwnHost`). For 056.

## Phases 2–5 — memory, consent, the sheet

`ExploreLive` builds the start page, the tab strip, the groups and the recents
from `explore_sites` + `browser_history` + `dapp_permissions`.
`SigningController` hosts the four machines one request needs and dies with it.
`SigningLive` turns their views into the drawn sheet. Hermetic tests **371 →
423** (25 new across `BrowserMemoryTests`, `RequestRouterTests`,
`SignRequestReadingTests`, `SigningAssemblyTests`, `SigningLiveTests`).

### Two deviations from the drawing, both recorded rather than hidden

**① 新建分组 has no field to type into.** The drawn manage sheet has the row and
no input. The name is asked for with the platform's own prompt — the same class
of call the document picker and the share sheet are elsewhere — rather than a
screen invented here.

**② The spending-cap editor had a 自定义 chip and nowhere to type.** Choosing it
selected a mode that could not be completed, and FR-010 requires an unlimited
approval to be cappable **to the person's own figure**, not only to their
balance or to zero. `AllowanceInput` adds the field; every word in it comes
from the existing corpus. For whoever redraws this card.

### A test that passed against a set it had never read — the second kind

Fixing the confirm-verb test surfaced the mirror image of phase 0's parity bug.
The first version asserted the slide's label does **not contain** the intent id,
which is simply false in English: "Confirm send" contains "send". It would have
failed honestly — but the temptation was to weaken it. The assertion is now
that the label **equals** the corpus value for that intent, driven in Chinese
where a leaked id would be unmistakable, plus an unknown intent falling back to
the neutral verb rather than printing itself.

### Phase 2 device pass — 3 of 3 on the iPhone 11

After a force-quit and a relaunch with no URL, 探索 comes back with the tab
**and its page**, and behind it the start page holds the pinned site and the
recent visit. Two documents, two machines, one screenshot.

Two defects the device found, both in the tests rather than in the app:

**① The page was served stale.** `test-without-building` left the simulator
running an old copy of the harness inside the test runner — the *Android*
wording, no `#verdict` log at all — so the announcement assertion failed
against a page that could not have printed it. An uninstall and a full `test`
fixed it. Worth remembering: a UI test's own **resources** can go stale in a
way its code cannot.

**② The test assumed a browser forgets its tabs.** It relaunched without a URL
and expected the start page, then looked for the recents there. What actually
happens — correctly — is that `explore_sites` restored the tab, the engine was
rebuilt and the page reloaded on its own. The test now asserts that first, as
the stronger half of the memory, and then closes the page to check the start
page. **The app was right and the test was wrong**, which is the same shape as
052's "a bad test that passes", inverted: a bad test that fails costs a person
an afternoon hunting a defect that is not there.

### The device blocker, while it lasted

Midway through phase 2 the iPhone began refusing to launch the development
build:

```
Unable to launch app.getvela.VelaWallet because it has an invalid code
signature, inadequate entitlements or its profile has not been explicitly
trusted by the user
```

The app **installs**; it will not start. The signature is a valid Apple
Development certificate and the profile runs to 2027 — checked, both — so this
is the device's trust of the developer certificate, which only a person
standing at the phone can restore (Settings ▸ 通用 ▸ VPN 与设备管理 ▸ trust the
developer).

**Resolved by the founder at 15:28**, and phases 1 and 2 are device-verified.
Recorded because it will happen again, and because the symptom — "invalid code
signature, inadequate entitlements" — reads like a build problem and is not
one. Check the certificate's trust on the phone before touching the project.
