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

## Phase 3 — a dApp connects

Device-verified on the iPhone 11.

| | |
|---|---|
| consent | the sheet opens itself, names the **origin** (twice — heading and subtitle), the account being granted, and the chain; 拒绝 / 批准 |
| approve | the page is answered `0x88cCA0…6894`, the golden multi-key Safe |
| `eth_chainId` | `0x64`, from the wallet's own state with no network |
| `eth_blockNumber` | answered through this wallet's pool for the browser's chain |
| `eth_sign` | **refused 4900**, and nothing was asked of any endpoint |
| the padlock | **absent** over `http://127.0.0.1` |

### The defect: a sheet that showed the wrong panel

The consent sheet opened and drew the **connected** panel — 断开连接, and the
explainer written for a site that already has an account — for a site that was
still asking.

`ExploreSheet` carried its *contents*: `sheet = .connection(model.menus.connection)`
stored the panel as it read at the moment of the tap, one frame before the
consent existed. So the sheet was correct about which sheet to open and stale
about everything inside it.

Android recorded the same bug on its group-manage sheet in its phase 2 ("the
sheet held a snapshot"), and iOS had it on all three: the group sheet would
have shown a group after it was deleted, the site menu a site after it was
unpinned. The fix is structural rather than local — `ExploreSheetKind` stores
**which** sheet, and `sheetContent` resolves the contents from the current
model on every render. The type now makes the bug unspellable.

Worth stating plainly: this one was found by *looking at the screenshot*, not
by a failing assertion. The test failed on the button's label being English in
a Chinese app; the panel behind it was wrong for a completely different reason,
and an assertion on "Approve" alone would have been made to pass without ever
seeing it.

## Phase 4 — the gate: dust leaves the Safe because a page asked

**Device-verified on the iPhone 11.** The golden Safe went from **0.50067** to
**0.48967 xDAI** — 0.001 sent plus ~0.01 in fees — because a web page called
`eth_sendTransaction`, and the page was answered with a transaction hash.

The sheet, screenshotted before the slide:

| | |
|---|---|
| who | `127.0.0.1:8137`, twice — host as name and as subtitle |
| what | **发送**, −0.001 xDAI in the amount card, 接收方 with the short and the full address |
| where | Gnosis |
| the fee | ~0.01 xDAI, quoted, not guessed |
| the technical details | present, collapsed |
| the slide | armed only once all three machines agreed |

**发送 and not the blind card.** A plain transfer with no calldata is the one
transaction the core resolves without a descriptor, and drawing "cannot decode
— 0 bytes" for it reads a dust send as an unknown contract call. Android
shipped that for one screenshot; it never shipped here.

### Three defects, all in driving the sheet, and the last one is a real lesson

**① The slide query matched an ancestor.** `app.buttons.containing(predicate)`
finds elements whose **descendants** match. The slide is a single accessibility
element with no children, so `containing` matched some container above it,
reported it enabled, and tapped a view that does nothing. `matching` is the
query that finds the slide.

**② `tap()` does not invoke an accessibility action.** With `matching` the
right element was found — and tapping it still did nothing. The slide carries
an `accessibilityAction` so VoiceOver and Switch Control can confirm by
activating, but XCUITest's `tap()` synthesises a **touch at the element's
centre**. The drag gesture then sees a press at ~50% of the track, under the
88% commit threshold, and resets. The sheet was left looking exactly as if
nothing had been tapped — which is what had happened.

A slide-to-confirm has to be **dragged**: press near the knob, drag to the far
end. That is also the only way the test exercises what a person does.

**③ The harness was slower than the chain.** Every `waitForExistence` on the
app snapshots the whole accessibility tree, web content included, and the test
page rendered a pretty-printed state object that grew with every action —
about 30 seconds of wall clock per poll. The page now keeps its last six
verdict lines and a 400-character state dump, and the submit is watched on the
**wallet's** surface (`已提交`) rather than the page's, because that is both
cheaper and the stronger claim: it means the relay accepted the operation, not
merely that a string reached a web page.

The through-line of all three: **a test that cannot drive the product cannot
report on it**, and each of these failed in a way that looked like a product
defect. The second one especially — a sheet that does not respond to a tap is
exactly what a broken confirm button looks like.
