# Delivery Report — 028 Web Port Completion

**Branch**: `028-web-port-completion` · Started 2026-09-04 · Base: `main` @
28d25ae9 (PR #185 merged; not stacked).

---

## Baselines (T401–T403) — recorded @ 28d25ae9

- **Core artifact**: `static/vela_core_bg.4603c8421603.wasm` = **3,630,664 B**.
  Must close byte-identical: both machines this feature gives a screen to
  (`manage_tokens`, and `send`'s sweep half) are already aboard, so wiring them
  costs zero bytes.
- **Corpus pins**: 1536 leaf + 84 branch paths, unchanged since 024. Unlike
  024–027 this feature does NOT assume a zero delta — preference rows, the
  scanner's refusals and the import report may need words that do not exist yet,
  and each will go through the 5-step process.
- **Green tree @ 28d25ae9**: `pnpm check` **1354 files / 0 errors** · lint clean
  · unit **765** · build ×15 + `build:extension` · e2e **138 passed / 1 skipped**
  on chromium + firefox + webkit. The one skip is 027's `test.fixme` for SC-304,
  which is this feature's stated precondition and not its task.
- **A flake worth watching**: one baseline run reported 137/1 and a non-zero
  exit while a second suite held port 4173. Run alone it is 138/1 every time.
  The extension suites launch their own browsers on top of the shared preview
  worker, which is the contention `workers: 3` was pinned for in 027 — if this
  appears again with nothing else running, it is real and gets chased.

### Port-provenance surface @ 28d25ae9 — 3,155 lines

| File | Lines | Why |
| --- | --- | --- |
| `components/QRScanner.tsx` (web engine only) | 693 | the decode ladder `docs/qr-scanner-web.md` measured |
| `services/qrcode.ts` | 554 | the encoder — dependency-free, byte mode, v1–10, EC-M |
| `services/contact-io.ts` | 341 | export/import, existing entry wins |
| `services/locale-format.ts` | 331 | explicit presets, never `Intl` |
| `services/share-card.ts` | 330 | address + code + identicon, composed |
| `services/fiat-convert.ts` | 231 | as its consumers need it |
| `services/erase-device.ts` | 160 | the namespace sweep |
| `services/currency-catalog.ts` | 158 | as needed |
| `services/readonly-rpc-gate.ts` | 109 | as needed |
| `services/token-list-filter.ts` | 98 | as needed |
| `services/image-decode.ts` | 74 | the picked-image path |
| `services/avatar-style.ts` | 54 | a preference |
| `services/saved-contact.ts` | 22 | beside `contact-io` |

**Not ported, with reasons**: the TypeScript twins Rust owns (`clear-signing`,
`approval-guard-editor`, `local-descriptors`, `siwe`); the WalletPair, webview
and native transports; App Group sync; `deployer-api`. See research D52.

**Already drawn, waiting only for wiring** — this feature draws nothing:
`AddTokenModel` / `AddTokenTab` / `AddTokenResult` with 31 fixture references
and `go('add-token')` routed from both `FlowsMobile` and `FlowsPanel`; the
sweep picker's `multi_select_mode` / `multi_selected_ids` in `SendView`;
`ShareCardModel`; `ScanSurface`; every preference row.

### The two decoder dependencies (T403)

`@undecaf/zbar-wasm` (239 KB of wasm) and `jsqr` (257 KB), declared in
**app-web's own `package.json`** — the rule 026 learned with `xlsx` and 027
re-learned with `esbuild`: a build-time or runtime import gets declared in the
package that uses it, the moment it is used, or a clean checkout builds an
import pointing at nothing.

They are **runtime** dependencies, not dev: the decoder ships to the browser.
Both are lazy (nothing loads until a scanner opens) and both are served from our
own origin — Expo fetches zbar from a CDN because Metro cannot import a module
using `import.meta`; Vite can, and this repo does not fetch code from third
parties at runtime (D45).

Encoding needs no dependency at all.

---

## Phase 2 — the code is data (T410–T415) 🎯 the trap

**What shipped**: the receive code, the payment-request code and the share card
encode the address. `jsqr` — a decoder that knows nothing about the encoder —
reads it back off the rendered screen.

### The plan said port 554 lines. Checking first said don't.

`src/services/qrcode.ts` is 554 hand-rolled, dependency-free lines, and the plan
called it the porting truth. **Nothing in the Expo tree imports it.** Expo's real
encoder is the `qrcode` npm package, used by `components/QRCode.tsx` and
`components/ui/TransactionReceipt.tsx`.

Porting code that has never run in production, under a heading that calls it
verified, is the worst kind of port. So the encoder is `qrcode` — the same one
the phone ships — and the only thing ported is `qr-path.ts` (20 lines), which
merges consecutive dark modules into one `h` run so the whole code is a single
SVG path. Per-cell rendering leaves hairline white gridlines from pixel
rounding, and a code with gridlines photographs badly, which is how most people
read one. Research D44 is corrected with the reasoning.

**Measured while deciding**: a plain address encodes to **29 modules at version
3** — exactly the `RECEIVE_MODULES = 29` spec 021 drew the card at, so that
geometry was chosen against a real code. A 131-character EIP-681 link encodes to
**49 modules**, so the card takes a varying module count rather than assuming 29.

### The placeholder stays

`qr-pattern.ts` is not deleted and keeps its own honest header. The galleries are
canon and their screenshots are diffed; a fixture that suddenly encoded a real
address would put real addresses in the gallery. `QRCard` takes an optional
`code` — absent means the placeholder, and every live surface supplies a real
one. Both branches render through the same `<path>`, so the placeholder draws
the pixels it always drew.

### The assertion that would have caught this in 021

Two round trips, not one screenshot:

- **unit** (`wallet/qr.test.ts`): rasterise what the card draws, hand it to
  `jsqr`, demand the address back — and the same for a full EIP-681 payment
  link, so amount, asset and chain are proven to survive.
- **e2e** (`receive-code.e2e.ts`): drive the real screen, take the path out of
  the rendered DOM, rasterise it the way a camera would see it, decode. It
  asserts 29 modules and the exact address.

"A QR appeared" is precisely the assertion a decorative pattern passes, which is
why neither of these is that.

**One finding, and the test caught ME**: the e2e first asserted an address
reconstructed from the shortened form `0x0cE19C…084e2e`. The decode returned the
real one and the test failed — proving the decode is genuinely reading the
rendered code, since a pattern would have returned nothing. The full derived
address now lives in `live-helpers.ts` as `TEST_ACCOUNT_ADDRESS` with a note that
the shortened form is unguessable by construction.

**Recorded**: the share card's live builder is wired and supplies the code, the
name, the address lines and the identicon; the *image* it produces ("Save
image") is Phase 3's, alongside the rest of the receive tooling.

**Gates**: check **1361**/0 · lint clean · unit **771** · build ×15 +
`build:extension` · e2e **139 passed / 1 skipped** (chromium + firefox + webkit)
· wasm byte-identical · corpus delta zero.

---

## Phase 3 (wip) — the decode engine (T420, T421, T424)

**What shipped**: the wallet can read a QR code. The engine is ported and proven
against the encoder Phase 2 landed, in a real browser.

- **`services/qr-decode.ts`** carries the ladder `docs/qr-scanner-web.md`
  measured, unchanged in substance: zbar at 1200/1000/800/600/400 with a
  downscale FIRST (a canvas downscale is a low-pass filter — it is what removes
  JPEG noise and moiré), then jsQR with `binInvert(160)` / `invert` /
  `binarize(160)` for the clean screenshots zbar refuses. A camera frame decodes
  at 1000 wide; a picked image is tried thoroughly, because a person is waiting
  on an answer rather than watching a viewfinder.
  Both decoders are **lazy** — nothing is fetched until a scanner opens.
- **`flows/core/scanner.svelte.ts`** is the camera and, mostly, its refusals.
  Each one is a different thing for a person to do, and only if it is said: a
  refusal they can undo in site settings (`denied`, which covers both "said no
  now" and "said no once and the browser remembers"), no camera at all
  (`absent` — most desktops), an insecure origin (`insecure`, where
  `getUserMedia` is simply undefined and the fix is the URL, not the device),
  and `unavailable` for everything else. Support is checked BEFORE asking, so a
  device without a camera never raises a prompt someone then has to dismiss.
- **`ScanSurface`** gains an optional `feed` snippet and a `notice` line. Absent,
  it draws exactly what the gallery has always drawn.

**T421 is not a port, and that is the finding**: Expo's `image-decode.ts` is a
pure-JS JPEG decoder that exists because native has no canvas. A browser decodes
images itself, so the picked-image path is `createImageBitmap` and nothing else.
74 lines not written.

**Proven in a real browser** (`qr-decode.svelte.test.ts`, the vitest browser
project): the real zbar WASM reads back an address and a full EIP-681 payment
link from what the receive card renders, and returns null for a blank image
rather than inventing something. Together with Phase 2's round trip this is the
whole loop a person performs — one shows a code, another scans it.

**One finding from the literal audit**: it failed on `1200px` / `1000px` / `5px`
inside this module's PROSE. The audit scans `src/lib/services` for px literals,
and it was right to: the fix is to write "1200 wide" and "about five pixels per
module", not to weaken the audit for comments.

### Still open in this phase

- **T422's route wiring** — `ScanSurface` has its slots; `FlowsMobile` /
  `FlowsPanel` do not yet forward them, and nothing drives `scanner` yet.
- **T423** — a decoded address does not yet reach the send form
  (`set_recipient`) or the sweep picker.
- **T425** — the scan e2e.
- **Two corpus words are missing** for the refusals the corpus has no phrase
  for: no camera on this device, and an insecure origin. `permissionText`,
  `noQrFound`, `noQrFoundMsg` and `errorImage` already cover the rest.

**Gates at this commit**: check **1366**/0 · lint clean · unit **783**
(+12: seven browser decode cases, five refusal cases) · e2e **139 passed / 1
skipped** on chromium + firefox + webkit.

**The e2e number cost four worthless runs to get, and the reason is worth more
than the number.** Every one of them was poisoned by something else I was
running at the same time:

- two Playwright suites at once fight over port 4173, and `reuseExistingServer`
  means the loser silently tests the winner's build — one run reported "18
  passed" in 29.8 minutes;
- killing a run to start another leaves the first's partial output looking like
  a failure (one stopped at test 75/140 because I had just `pkill`ed it);
- and a manual `pnpm build` alongside a suite's own build is the sharpest of
  them: two `vite build` processes write `.svelte-kit/output`, one deletes what
  the other is about to read, and the crash reads
  `ENOENT: no such file or directory, scandir '.svelte-kit/output/client'` —
  which looks exactly like a broken build and is not one.

**The rule: one build or suite at a time, and touch nothing while it runs.** It
is the same lesson 026 learned as worker starvation and 027 pinned as
`workers: 3` — this is that lesson applied to the person driving, not to the
config.

---

## Phase 3 (complete) — the scanner reads, and says why when it cannot

**What closed**: T422, T423, T425, T426. The wallet now reads a code from the
camera or from a chosen image, on both layouts, and every way it can fail has
its own sentence on the screen.

### The scanner is the CORE's state, and that is what made T423 small

`SendView` has carried `show_scanner` and the `open_scanner` / `close_scanner` /
`scan_resolved` events since the machine was written. So the shell holds no
"scanner is open" flag at all:

- the recipient row's scan button dispatches `open_scanner`;
- `sendState` reads `view.show_scanner` and shows `s1` — ahead of the fee sheet
  and the importer, and ahead of the core's own stage;
- a decode dispatches `scan_resolved`, which fills the row AND closes the
  surface in one transition, in Rust.

That is also why the **sweep picker needs nothing of its own** (T423's second
half): the sweep's recipient row is the same `RecipientField` with the same
`scanLabel`, so it opens the same core scanner and is filled by the same event.
Had this been a shell boolean, it would have been two.

**The one shell rule left** is the wallet home's, and it is Expo's verbatim
(`useHomeController.ts:529`): with no session open there is nothing to dispatch
INTO, so a scanned code OPENS one — prefilled, and `locked: true` when the
request names a chain to lock to. `parseEIP681` was already ported (spec 026);
the shell only tokenizes, and every decision about the parse is `send.rs`'s.

### `FlowsPanel` was one surface too many

The wip note listed it as unwired. It has no scan branch and never did — the
desktop scanner is a centred modal (DS1L) the route draws over the window,
because a viewfinder in a 380px column is the wrong shape. Both call sites are
now live and both share ONE `{#snippet}`, so there is one `<video>` and one
camera no matter which layout is drawn.

### Three carried gotchas, all of them real

- **The file input is mounted, not conditional.** `click()` on an input that is
  not in the document opens nothing, and "choose a photo" is the entire way out
  for someone whose camera was refused.
- **`pointer-events: none` on the `<video>`.** It fills the frame the brackets
  mark and would otherwise eat the taps meant for the tools under it.
- **A two-second re-arm.** Expo re-arms its scanner after a decode, and the
  reason survives the port: a poster with an unusable code in frame must not end
  the scan, and re-arming instantly would decode that same code forever.

### The corpus grew by three words, and the artifact is NOT byte-identical

`componentsUi.scanner.{noCamera,insecureOrigin,cameraUnavailable}` × 15 locales
= 45 strings. The corpus was searched before it was grown: `permissionText`
covers the refusal a person can undo in site settings, and nothing existing said
"there is no camera here", "this page is not on HTTPS" or "something else has
the camera". These are refusals **native never had** — a phone has a camera and
an app has no origin — which is why the port could not carry them.

The Phase 1 baseline said the core artifact must close byte-identical. **It does
not, and the reason is words rather than machines**: the shared key-path table
is compiled into `vela-core` (catalogs are not — every locale is fetched as JSON
at runtime), so three paths cost **129 bytes**:

| | Phase 1 | now |
| --- | --- | --- |
| artifact | `vela_core_bg.4603c8421603.wasm` | `vela_core_bg.7caac430e4b3.wasm` |
| bytes | 3,630,664 | 3,630,793 (+129) |
| corpus | 1,536 leaf + 84 branch | 1,539 leaf + 84 branch |

T460's budget assertion should be restated accordingly: **one** core artifact,
still the only one a wallet route loads — which the e2e already proves against
the new fingerprint — and any byte delta explained. "Byte-identical" was the
right pin for two machines that were already aboard; it was never the right pin
for a feature whose own baseline says the corpus will grow.

**The 5-step corpus recipe has a sixth step, and a wrong flag.**
`cargo test -p vela-core --features i18n-all` does not compile — `app` is behind
the `crux` feature, so the parity suites cannot see the machines. It is
`--features i18n-all,crux`.

**The 45 strings are mine, not a translator's.** They follow each locale's own
register (zh-HK in spoken Cantonese, de/fr formal to match the neighbouring
lines) and they want a human eye before release, like every machine-written
batch this project has shipped.

### What the e2e actually proves (SC-402)

`scan.e2e.ts`, five tests, all on real pixels — `qrcode` writes a PNG in Node,
the browser decodes it with the real ladder, and nothing in the app is on both
ends:

1. a picked code's address reaches the **send form's recipient field**, exactly;
2. the scanner opened FROM that form fills the row it was opened from — the
   core's `open_scanner` → `scan_resolved` round trip;
3. a refused camera says so, and the photo path is still on the same screen;
4. a machine with no camera gets a DIFFERENT sentence, because it is a
   different thing to do about it;
5. a code that decodes to something that is not a payment says *that* — which
   is also the control for (1): a decoder returning nothing would report "no QR
   found" instead.

### Known and named, not hidden

A scan at desktop width opens the send panel with the scanned recipient in it,
and that panel still cannot be DRIVEN — `FlowsPanel` gets live send data but no
send actions until **T453** (Phase 6). This is the state the desktop send is
already in when opened from the sidebar; the scan does not make it worse, and
one phase closes it.

**Gates**: check **1366**/0 · lint clean · unit **787** (+4 notice cases) ·
build ×15 + `build:extension` · e2e **144 passed / 1 skipped** on chromium +
firefox + webkit · root `jest src/__tests__/i18n/` 75 passed (leaf pin
22,758 → **22,803**) · `cargo test -p vela-core --features i18n-all,crux` green ·
`lint:i18n` no new defects · `verify:i18n` 73,085 comparisons, zero divergences.

---

## Phase 4 — preferences that do what they say (T430–T437)

**What shipped**: the six rows 023 drew and nobody wired. Theme, language, the
number / date / time presets and the avatar style now change what they name,
survive a reload, and are read everywhere the app prints a figure. "Erase this
device" erases this device.

### The presets are the product's, and `Intl` gets one job

`services/locale-format.ts` is the port, and the rule it exists for survives the
platform change: **explicit rules, never `Intl` for the output**. A wallet that
renders a figure by the browser's idea of a locale is a wallet where the same
balance reads two ways on two machines, and where a person cannot tell a
thousands separator from a decimal point in a language they are guessing at.

`Intl` appears exactly once — in the `auto` DETECTION, reading the platform's
conventions to pick a preset. That is a different act from letting it format
money, and its result is a preset like any other.

**Where the preset landed, and where it deliberately did not**:

| Surface | What changed | Why |
| --- | --- | --- |
| Money (`moneyParts`, `moneyText`) | grouping **and** decimal mark | D47's own sentence: this includes money, which is why it is not cosmetic |
| Token amounts (`trimBalance`) | the decimal mark only | a decimal comma read as a thousands separator is a hundredfold mistake about an amount; but the mocks draw token amounts ungrouped, and this feature wires preferences rather than redrawing screens |
| Day headers older than yesterday | the date preset | which is what the phone does — `activity.ts::dayGroupLabel` calls the same `formatDate`, so the two clients group a history the same way |
| `services/activity.ts::formatUsd` | **untouched** | it writes a STORED field in a record whose shape is the Expo compatibility contract, not a string anyone renders. Reformatting it would change data, not presentation |

**A dead parameter went with it.** `dayLabel(…, locale)` and
`liveActivityGroups(…, locale)` no longer consult a locale, so the parameter and
the `locale?: string` field in three input interfaces were removed rather than
left as a signature that lies about what it reads.

### Preferences have no machine, and none was invented

`services/preferences.svelte.ts` is shell state (D48). A Rust machine holding
four enums and no rule would be a machine in name only — compare
`display_currency`, which HAS one, because choosing a currency has a rule behind
it (a rate must exist, and an unpriceable currency must refuse).

**localStorage, not the IndexedDB KV**, and the reason is the same one 024 gave
`vela.serviceEndpoints`: these are read SYNCHRONOUSLY while a screen renders —
every money figure asks the number preset — and the theme has to be on the
document before the first paint. Record shapes are Expo's byte-for-byte
(`vela.localePrefs` one JSON object, `vela.avatarStyle` and `vela.language` bare
strings), so a phone and a browser would read each other.

`vela.theme` is the one key Expo has no counterpart for: a phone follows the OS
and offers no choice; a browser tab is a window inside someone else's chrome,
where "follow the OS" is a preference rather than the only option. `system`
**removes** the attribute rather than writing a resolved value — a pinned "dark"
would stop following an OS that changes at sunset, which is the whole meaning of
the choice.

**Applied before first paint**, in `app.html`'s existing inline script, for the
same reason spec 012 put the launch decision there: a theme that arrives in
`onMount` paints the OS palette first and then flips, which on a wallet reads as
a glitch rather than a setting.

**`avatar-style.ts` was not ported as a module.** Its 54 lines are an
AsyncStorage cache plus a listener set plus a version counter — all three of
which are what `$state` already is. The preference folded into `preferences`;
what WAS ported is the artwork: `initialsSvg` is `WalletAvatar.tsx`'s
non-identicon branch, same 0.34 letter-to-diameter ratio, same `V` fallback when
there is no name. It is composed as an SVG STRING because every avatar in the
app already takes one (the identicon comes from vela-core that way), and its
colours are `var(--…)` token references — so it wears whatever palette the page
is wearing and follows a theme change without being rebuilt.

### Erase deletes a namespace, and names its one exception

Carried verbatim, including the reasoning that produced it: **a delete-list
erase is wrong by default and only accidentally right**. Nothing about a
hand-maintained list of keys fails when the app grows a key, so it drifts in
silence and the failure is found by someone whose data was supposed to be gone.

The web made one thing bigger: Expo had ONE AsyncStorage; a browser has three
stores — `localStorage`, the IndexedDB KV, and (extension build only)
`chrome.storage.local`. All three are swept by the same rule, and a store this
build does not have contributes nothing rather than failing. `storage.ts` gained
`getAllKeys()` for exactly this: a sweep has to be able to ask what is actually
here.

The keep-list is one key — `vela.pendingUploads` — and
`contracts/erase-scope.md` now carries its reason rather than implying it. The
verification pass re-enumerates and `EraseIncompleteError` rejects, which is
what stops the route sending a person to first run over a partial wipe. On
failure they stay signed in, on the same sheet, with the reason in the sheet's
own danger callout and the button still live.

**No corpus change**: `settings.eraseDevice.*` already carried all eight
strings, `failed` included. It only needed adding to the settings message
manifest, which is a type and a mapping, not a word.

### One finding, from the e2e

`seedSignedIn` re-imposes a wallet before EVERY document (it is an
`addInitScript`), so after an erase navigates to Welcome the harness has already
written `vela.accounts` back. The first version of the test read that as a
failed erase. What the erase did is therefore measured on the keys the harness
does not write — a key no feature has ever written, and an IndexedDB store that
must come back empty — with the reason recorded in the test rather than in
somebody's memory.

### What is live in Settings now, and what is still a picture

FR-411 asked for the route's own description of itself to stop lying, and it
does. **Live**: the network list, its editor, the add-network wizard (024); the
display currency (024); the connected sites (027); and now the theme, the
language row, the three format presets, the avatar style and the erase. **Still
canon data**: the latency figures, the storage accounting and the RPC-provider
health — those wait for the features that measure them.

### Named, not hidden

- The desktop's localization dropdowns gained per-row `options`, so a live panel
  can open any of the three menus while DST3's single pinned-open `dropdown`
  stays exactly the board it was drawn as.
- Choosing a language NAVIGATES, because on the web a language is a route: every
  locale is its own prerendered page. The choice is also stored, but `/`'s
  Accept-Language negotiation is server-side and cannot read it — a returning
  person who lands on `/` still gets their browser's answer. Recorded here
  rather than fixed, because fixing it means a redirect on a page whose whole
  budget is that it ships no JavaScript.

**Gates**: check **1373**/0 · lint clean · unit **812** (+25: 12 format presets,
6 erase, 7 preference store) · build ×15 + `build:extension` · e2e **159 passed /
1 skipped** on chromium + firefox + webkit · corpus delta **zero** (the erase
copy already existed, `failed` included — it needed a manifest entry, not a
word) · core artifact unchanged from Phase 3.

**And the run before it failed 30 tests, which was my machine and not the code.**
An ORPHANED `workerd` (parent gone, 31 minutes old) was still on the box beside
an 18-hour `vite dev` from someone's editor; with `reuseExistingServer: true` a
suite can end up measuring a server nobody owns, and when that one went away
thirty tests reported "Could not connect to the server" at once. Killed the
orphan, re-ran, 159/1 in 2.4 minutes. Phase 3's rule stands and now has a
corollary: **one suite at a time, and check for orphans before believing a red
run** — `lsof -ti tcp:4173` and `pgrep -f workerd` before, not after.

---

## Phase 5 — sweep and custom tokens (T440–T446)

**What shipped**: the two machines that were aboard with nothing to show — the
`send` core's multi-select half and `manage_tokens` — each got its first
screen. Several assets on one network go out as ONE operation; a token is added
by contract address with the identity the chain reports, and it is still there
after a reload.

### The sweep is five fields the core already carried

`live-send.ts` used none of `multi_select_mode`, `multi_selected_ids`,
`multi_chain_id`, `multi_valuable_ids`, `multi_specs`; now every fact on SD1b,
SD2d and SD3c is one of them:

| On screen | The core's field | What the shell does with it |
| --- | --- | --- |
| the tick per row | `multi_selected_ids` | `includes(sendTokenId(token))` |
| the greyed rows and the notice | `multi_chain_id` | `token.chain_id !== chain` |
| the master tick's scope | `toggle_all_multi_tokens { visible_ids }` | names what the picker is SHOWING; what counts as valuable stays `is_valuable()` |
| the per-token amounts | `multi_specs` | `baseToHuman()` — string arithmetic, never a `Number` |
| the CTA's count and the confirm's "N assets" | the same ids | counted, not decided |

`sendTokenId()` is `SendToken::id()` byte for byte — `{network}_{address|native}_{symbol}`
— and is pinned by a unit, because an id that drifted would select nothing and
say nothing. `baseToHuman()` exists because a sweep's amounts are uint256 base
units net of the gas reserve (the EXACT figures a submit moves, invariant ⑪),
and a double on the way to the confirm page would round the number the
signature is built from.

**One shell flag, by precedent.** The core's `multi_select_mode` flips only when
the selection is CONFIRMED; whether the checkboxes are showing before that is
`sweepPicking`, shell state — the phone keeps the same pre-confirm flag as its
chain filter (`TokenSelector.tsx: sweepActive`). Everything the flag reveals is
the core's.

**The chain is pinned by the first pick, not by a filter.** The phone's picker
pins the network with a filter chip the web's SD1b does not draw; SD1b's notice
appears "once the first token pins the network". So the first tap dispatches
`set_multi_network` before `toggle_multi_token`, the core refuses every other
chain from then on, and an emptied selection unpins (an `$effect`, so a person
can start over without leaving the screen). A UI choice about when a rule
starts applying, not a second copy of the rule.

**ONE operation** needed nothing new: `slide_confirm` builds
`multi_token_specs` in Rust and hands the calls to the existing spine, where
"one call stays a single `executeUserOp` and N stay a MultiSend" was already
written in 026. `sweep.e2e.ts` counts the relay's `eth_sendUserOperation`s and
finds exactly one, whose calldata calls the USDC contract and names the
recipient twice — once for the ETH value, once as the transfer's argument.

### `manage_tokens` — constructed by nothing since 025, now by the sheet

The session is built when the add-token sheet opens and disposed when it
closes; the network snapshot rides on the probe request because the registry
(defaults + custom networks) is the shell's; a confirmed save invalidates the
token cache through the core's OWN `invalidate_token_cache`, and the route
answers that with `balance.refresh(true)` — the balance list learns to look
again on the machine's schedule, not the sheet's.

**The probe is implicit.** The phone has a separate "search" button; the drawn
sheet (T3) has one CTA, "add". So a well-formed address probes on its own, and
the core's echo gate discards an answer for an address the person has already
typed past — the staleness rule 016 wrote for exactly this.

**Two things the mock draws are not here, recorded rather than improvised**
(D51's rule): the network picker, because the core probes every known chain at
once and the card names the one that answered — a picker would be choosing what
the core already found; and the native-token tab, which is `network_admin`'s
and lives in Settings. When several chains answer, the card is the first in
registry order, which is the core's own order.

**A sheet is a pushed step, and dismissing it now pops the step.** `FlowsMobile`
hid a closed sheet without telling the route, so the next `go('add-token')` was
a no-op on a stack whose top was already `t3` — nothing opened. `onsheetclose`
is the callback the host never had; the route pops and disposes.

### The e2e caught a real bug before anyone did

The first `liveSendForm` / `liveSendConfirm` treated `multi_specs[].amount` as
uint256 BASE units and converted it — and the sweep e2e's confirm page priced a
100 USDC + 1.5 ETH sweep at **$0.00**, with "0.0001 USDC" in the breakdown. The
core's `MultiTokenSpec.amount` is `full_balance()` less the reserve: a HUMAN
decimal string, exactly as the phone hands it to its batch builder, which is
the thing that makes base units. The second conversion was mine. It is gone,
the unit test now feeds human strings, and the reason lives in `sweepAmount`'s
own comment. This is what SC-404 "asserted, not eyeballed" buys: a unit test
with the wrong fixture would have passed.

**And a second one, in the stub.** With the amounts right, the sweep still sat
on the confirm page: the diagnostic the test grew (`[send] unhandled tx error:
invalid quantity: 0x…20…00: the value is too large to fit the target type`)
named it. A plain `eth_call` on the submit path — the Safe's `nonce()` — expects
ONE word, and the per-call stub had wrapped every `eth_call` in an aggregate3
envelope. `isAggregate3()` now branches, and a plain call answers one zero
word exactly as `send-lands` does. Two runs, two real findings: one in the
shell, one in the harness, neither visible to a unit test.

### The multicall stub answers per call now

025/026's suites answer every `aggregate3` with N copies of one blob, which is
enough when every slot decodes the same two words. A sweep needs two balances
and an add-token probe needs `name()` and `symbol()` back as STRINGS, so
`e2e/stub-multicall.ts` walks the calldata (the inverse of `encAggregate3`),
finds each inner call's target and selector, and lets the test answer each. DEX
quotes it declines, and the price ladder falls through to the Chainlink feed as
it is built to.

### Known and named

- **A zero-balance custom token is not in the holdings list**, because the
  holdings list is holdings: `fetchTokens` filters `balance > 0`, on the phone
  as here. It IS listed in the sheet's own manage list (`custom_tokens`) and
  will appear the moment it holds anything. The e2e gives it a balance.
- **The desktop has no SD1b.** `DesktopFlowStateId` carries no sweep board
  (021 drew none), and the desktop send is fixture-actioned until T453 anyway.
  The sweep is the phone layout's until Phase 6 hands the panel the same
  actions.
- **Two suites cannot share one worktree's build.** The run after the fix
  failed IDENTICALLY, because another session's Playwright had started its own
  preview on 4173 in the four minutes between my runs, and `reuseExistingServer`
  handed mine THEIR `.svelte-kit/cloudflare` from before the fix. The build dir
  is one; two sessions building it swap each other's served code under a live
  `wrangler dev`. Phase 3's rule ("one suite at a time") has its mechanism
  named: check `lsof -ti tcp:4173` and wait, rather than kill, when it is
  someone else's.
- **Another session is editing the settings files in this tree** (a text-scale
  slider — `preferences.svelte.ts`, `settings/live.ts`, `app.html`, the
  preferences test and e2e). Their work is mid-flight and the shared `pnpm lint`
  is red on two of THEIR unused imports; this phase's lint was run on its own
  files, and only its own files are committed.

**Gates**: check **0 errors on this phase's files** (the shared tree carries a
syntax error in another session's untracked `TokenIcon.svelte.test.ts`) · lint
clean on this phase's files (the shared `pnpm lint` and the tokens literal audit
are red on that session's in-flight `settings/live.ts` and
`ContactsDesktop.svelte`) · unit **850** passed, 1 failed = that audit case ·
build ×15 + `build:extension` · e2e: **chromium 0 failures** in the full
three-engine run (the sweep and both add-token cases included); **firefox +
webkit 52/52** re-measured at `--workers=1` after the same run timed 29 of them
out at machine load 54→94 — the 026 starvation, which the box's other sessions'
builds turned into a certainty. Recorded as two numbers rather than one
because they were two runs; the second is the fair one.

---

## Phase 6 — the desktop sends; the book's travels were reassigned

**What shipped**: T453 and its e2e. The wide layout drives the send flow
through the same session the phone does, and a send completes in the third
column (SC-409).

**What did not, and why**: T450–T452 (the contacts export/import port) were
reassigned mid-phase — the founder handed the whole app-web contacts feature,
import/export included, to a separate session (`vela-wallet-63`). Everything
this session had learned went to them in one message rather than into code
that would have collided with theirs: the core already owns the import policy
(`import_parsed` → `apply_import` → `last_import: ContactImportReport`), so the
web port is parse/serialize only; `file-io.ts` has `pickTable()` (no `.json`
yet) and `saveTextFile()`; the desktop header dropdown already emits
`sheet-select` with `importAll` / `exportAll`; the phone's "+" opens the add
form, so the phone has no export entry until the drawn `addMenu` is wired.
SC-408 is theirs to prove.

### T453 — one session, two hosts

`FlowsPanel` now takes the SAME `SendActions` / `BatchActions` interfaces
`FlowsMobile` takes, and the route's third column reads the core's stage
(`desktopSendState`: `dsd1` / `dsd2` / `dsd2b` / `dsd3` / `dsd4`, the fee sheet
and the importer as `dsd2f` / `dsd2c`, the scanner as the `ds1` modal) ahead of
the nav stack — exactly the rule the phone host has followed since 026. Until
now the column showed the send screens with live DATA (026's overlays) and
dead controls: a Continue that did nothing on a form that knew the balance.

The three surfaces the phone raises as sheets and the desktop opens as panels
— the fee coin, the importer, the scanner — became methods on the session's
actions (`openFeeSheet` / `openBatch` / `openScanner`) rather than nav steps,
so the host that asks does not need to know which layout it is on. The panel's
Back closes a sub-panel before it steps the core back; its Close closes the
session.

### The incident: a 0-byte file, and how it came back

Mid-phase, an edit script of this session opened the wallet route for writing
BEFORE its transform ran; the transform's assertion failed and the file was
left at 0 bytes — with another session's complete, unstaged hunks in it. That
session had just `git add`ed the file and restored the worktree from the
index; this session's reflex `git checkout HEAD -- <file>` then clobbered the
index copy too. The content survived as a dangling blob (`git fsck
--dangling` → `b3b85692`, 1057 lines), was restored, and the other session
committed its hunks from it (`0053cf22`). This session's T453 edits were
re-applied on top by a script that reads, transforms, and only then writes.

Two rules from it, written into the concurrent-sessions memory: **never open
the destination before the transform has succeeded**, and **never `checkout
HEAD` a shared file to "reset" it** — a peer's restore may be sitting in the
index, and the object database is where a wiped file actually lives.

### Gating three sessions deep

The Phase 6 gate went red twice before it was measured — both times with
`ERR_CONNECTION_REFUSED` / "Could not connect to the server" across every
firefox and webkit test, and the preview's log ending in `wrangler dev`'s own
crash: an EMPTY `✘ [ERROR]` after serving `/en/wallet`, "if you think this is a
bug then please create an issue". Chromium, which runs first, was already
through with ONE real failure.

The single-threaded preview crashing under three engines is 026's starvation
in a new coat, and the third session's builds in the same worktree made it
unmeasurable. So the gate moved: `git worktree add` at HEAD with this
session's staged patch applied, its own `node_modules` (offline install), a
`playwright.isolated.config.ts` that serves on **4174** with
`reuseExistingServer: false` — a run there measures exactly the code it built,
and nobody else's build dir or port is in the room.

The one chromium failure was the sweep e2e racing its own receipt: the relay
stub's default answer is "landed", so the tracker's poll confirmed the
operation before the test looked for the submitted title (the page said "Sent
1.49694 ETH · Done"). The stub now answers `pending` for the whole test, as
`desktop-send` already did.

**Gates (Phase 6)**: check **0 errors on this session's files** (the shared
tree carries 2 from the third session's in-flight `SendActions` /
`ContactsView` changes) · lint clean on this session's files · unit **857** ·
build ×15 + `build:extension` · e2e in the isolated worktree on 4174:
**180 passed / 1 skipped / 1 failed** on chromium + firefox + webkit, the one
failure being T460's decoder budget mid-construction (below), re-run **3/3**
once finished. Every SC-409 assertion is in `desktop-send.e2e.ts`.

## Phase 6b — the book travels, and the book does things (US5, `vela-wallet-63`) — 2026-09-05

**The founder's report, verbatim in effect:** the web address book could add a
contact and make a group, and nothing else. Add member, import, export, a
contact's 转账 / 收款 / 二维码 / 最近往来 / 全部往来, every right-click row
(import, rename, export, delete), 从文件导入, 群发转账 — drawn, dead. And the
standing rule: the rules go in vela-core, because Android, iOS and the desktop
run the same book.

### What the core gained (contacts.rs, contacts_io.rs, send.rs)

- **The file format is the core's now.** `app/contacts_io.rs` (new) is the
  desktop shell's `executor/contact_io.rs` (031) lifted up a level: JSON/CSV
  sniffing (extension, then shape, BOM-tolerant), quoting, the address-column
  heuristics (header word → first column that holds an address), the
  headerless-positional rule, the `;`-joined groups column, and the filename
  `vela-contacts[-<slug>][-<yyyy-mm-dd>].{json,csv}` (slug keeps any script's
  letters — 家人 stays 家人). Four shells now read each other's backups by one
  definition. **Two semantics changed** against the desktop copy, per D50:
  malformed JSON and an empty CSV are REFUSED (`ContactImportFailure`) rather
  than parsed to an empty success — "0 added" is indistinguishable from an
  empty book, and the file's mistake was being erased before anyone saw it.
- **Events, not operations.** `import_file { content, filename, into_group,
  now_ms }` · `import_acknowledged` · `export_requested { scope: all|group,
  format: json|csv, exported_at_iso }` · `export_taken` · `add_group_members`
  · `remove_group_member` · `set_contact_groups { address, group_ids }`. View
  fields: `import_failure`, `export` (a one-shot file). No
  `ContactOperation`/`ContactShellResult` variant was added and no stored
  shape moved, because contacts.rs has FOUR shell consumers (web, the desktop
  executor, the 040 Android and 050 iOS worktrees): a new operation breaks
  every exhaustive match, a renamed stored field silently corrupts
  `vela.contacts*` on someone's other device. Android decodes with
  `ignoreUnknownKeys`, Swift ignores unknown keys, serde does too — view
  additions cost the shells nothing. (Coordinated with vela-wallet-d1, who
  named the four consumers and the trap.)
- **"导入到本组" is a rule, stated:** every VALID row in the file is seated in
  the group — the newly added AND the already saved — while existing-wins
  still protects every saved name, note and star. The person named the group;
  membership is the one thing an existing entry does not get to veto. A stale
  group id refuses before any write (`unknown_group`).
- **A pick closes the picker** (`send.rs::apply_picked_address`), as
  `seed_split` already did for a group — the shell should not need a second
  sentence to say what the first one meant.
- **A prefilled recipient is the recipient from the first frame**
  (`send.rs::open`). The hand-off e2e found the old order: `open` only
  remembered `prefilled_recipient` in `params`, and `tokens_fetched` was what
  wrote it into `recipient` — so with the chain unreachable (the test's
  hermetic deny, or a real outage) the form opened on nobody, and the probe
  read an empty field under a heading that still said "Send USDT" from the
  fixture. The recipient is known before any token is; `open` now says so, and
  `tokens_fetched` restates it beside the token it picks.
- Tests: `app_contacts` 44 → **54** (export→import restore + re-import is a
  no-op, import-into-group, the four refusals write nothing, group export
  covers its members only, filename slugs, add/remove/reseat members, the
  four parse heuristics); `app_send` 91 → **93** (pick fills and closes; a
  prefilled recipient shows before the tokens arrive).

### What the web gained

- **Every drawn affordance does something.** Detail: 转账 → `/wallet?to=…`,
  收款 → `/wallet?flow=receive`, 二维码 → the contact's address as a code
  (their identicon in the centre, the address printed in full, copy), copy on
  the address block, `+ 移入分组` → a group tick-list (`set_contact_groups`),
  最近往来 from the wallet's own feed narrowed to this counterparty (three
  rows, 全部 › shows them all, the history empty state when there are none).
  Group: 添加成员 → the book as a tick-list (`set_group_members`), 群发转账 →
  `/wallet?group=<id>`, ⋯ → 编辑/导入到本组/导出本组/删除分组. Desktop:
  right-click on a group row and on a contact row (the drawn DC6 / M2 menus),
  the header ⋯ (导入通讯录 / 导出全部通讯录), the group head's CTA and ⋯.
  Mobile: the "+" opens the drawn C5 sheet (新建联系人 / 从文件导入 /
  导出通讯录) — which is how the phone gets an export entry at all.
- **Files in and out** through 026's seams: `pickTextFile('.json,.csv,.txt')`
  → `import_file`; the view's `export` → `saveTextFile` → `export_taken`. The
  report (added / already existed / + invalid count) or the refusal is the
  core's, shown in one sheet-or-dialog and acknowledged.
- **The book meets the money.** `load_send_history` reads the real tx store
  (025's `vela.transactionHistory`) — so recent recipients become the core's
  `auto` suggestions and `first_interaction` is judged on real history. The
  send flow's recipient picker (SD2e/DSD2e) — which showed the gallery's three
  fixture people inside a LIVE send, and whose core state
  `show_contact_picker` nothing read — is fed by a route-scoped ContactsCore
  session: a pick dispatches `picked_address`, a group `seed_split_recipients`.
  `/contacts` hands a person over by URL (`flows/contact-handoff.ts`); the
  wallet route reads it once and opens the flow as a scanned code would.
- **Corpus +2** (`contacts.groupDeleteBody` — deleting a group keeps its
  contacts, and the confirm says so; `contacts.importDoneInvalid` — the third
  number the report had no words for). Pins 1623→1625 / 1539→1541; jest leaf
  total 22,803→22,833. `lint:i18n` rejected `{{count}}` in a key without
  plural forms (A5); the placeholder is `{{invalid}}`.

### Deviations, recorded

- **The wasm moved** (4603c8421603 → 08aa37e9ddf9, 3,630,664 → 3,675,329 B;
  +44,665 B for the file format, the seven events and the two send rules) and the
  generated wallet-state types regenerated into both mirrors (4 new files,
  2 changed) — a founder-level decision ("业务规则要在 vela core 里面写"), not
  a slip. Phase 6c moved it once more (the initial tables); every later spec's
  "artifact byte-identical" budget restarts from 643c05990ef3.
- **Desktop import fork, with an end date.** Until the desktop dispatches the
  core's `import_file` / `export_requested` and deletes its `contact_io.rs`
  (owner: the vela-wallet-native session, 032+), the same malformed file
  "succeeds with nothing" there and is refused here. Named by d1; agreed.
- **`?to=` is a prefill, never a lock** — the same trust the phone's
  `prefilled_recipient` deep link has. A locked send needs a chain and comes
  only from a scanned EIP-681 request.
- **Not done, and said:** desktop drag-a-contact-onto-a-group (018's
  `dropTarget` board) — the tick-list covers the need; the split-mode
  per-row "pick from book" (SendForm has no per-row handler; the row's scan
  path already exists); an activity filter on the wallet's history screen for
  全部往来 (the detail expands in place instead).

### Gates

Shared tree at `48558b3b` + this work, machine load 20–120 (three sessions
building): `cargo test app_contacts` **54** · `app_send` **93** · `cargo fmt`
clean · `clippy` clean on the touched files · corpus pipeline (gen / lint /
verify / dump:vectors / root jest i18n **22,833** leaves) green · root
`tsc --noEmit` clean (the Expo `use-contacts-book.ts` empty view gained the
two fields) · `gen-core-types` regenerated into both mirrors and `--check`
current · `pnpm check` **1409 files / 0 errors** · `pnpm lint` (whole app-web)
clean · unit **869** passed · build ×15 + `build:extension` · e2e on 4173,
preview owned for the run: `contacts-io` **4/4** + `contacts-persistence`
**4/4** on chromium. Four e2e reruns were selector ambiguities (the same word
on a sheet's close and the page's back; a member count repeated in the CTA
caption; an address on the page and in the sheet); the fifth was the real
finding above (a hand-off that opened on nobody) and moved a rule into the
core. Firefox/webkit not run for `contacts-io` — it is not in the
`STORAGE_SUITES` list; the persistence pair already proves the store on three
engines.


## Phase 6c — two findings the other shells brought back (`vela-wallet-63`) — 2026-09-05

Both surfaced by the iOS session (spec 050) checking what 6cec4ddf did to its
client, both cross-client, both the founder's call ("做") before they moved.

### Which letter 妈妈 files under is a rule, and now the core's

`contacts/live.ts` sectioned the A–Z list with `first >= 'A' && first <= 'Z'
? first : '#'` — every Chinese name in one bucket at the bottom of the rail,
for two specs. The 018 drawing files 阿豪 under A, 妈妈 under M, "DAO 金库"
under D (`ContactsFixtures`, `SECTION_LETTERS = [A,B,C,D,H,M]`). iOS had
fixed it locally with Foundation's `.toLatin + .stripDiacritics`; Android and
macOS would each have reached for their own ICU; wasm has none — which is
exactly why the web landed on `#`. Four platforms answering a question with
one right answer is four chances to disagree, so the answer moved to the one
place that can decide for the platform with the least:

- `app/contacts_initials.rs` (new): one ASCII initial per codepoint for Han
  U+4E00–U+9FFF (20,992), Latin-with-diacritics U+00C0–U+024F (400), kana
  U+3040–U+30FF (192), Greek U+0370–U+03FF (144) and Cyrillic U+0400–U+04FF
  (256), plus Hangul syllables by ARITHMETIC (`S = 0xAC00 + L·588 + V·28 + T`:
  a 19-entry lead table and a 21-entry vowel table for the silent ㅇ — the
  generator checked all 11,172 syllables against ICU, 0 mismatches). All
  generated once from ICU's `Any-Latin` + `stripDiacritics` via the Swift
  snippets below. Pinyin initials are contiguous in GB2312 order, not Unicode
  order, so Han admits no compression — ~22 KB in all against build-web's
  4,000,000 B ceiling. Polyphones take their common reading (曾 → C, 重 → Z);
  a nameless address is `#`; Arabic, Devanagari, Thai file under `#` today
  (extend the table, never special-case a shell).
  **The first cut was Han + Latin only** — the iOS session measured that as a
  NARROWING for its shipped ja/ko locales (ICU's `.toLatin` already filed
  さくら → S, 김민준 → G), so the scripts were added before anyone switched:
  the rule must be "better everywhere", not "same for Chinese, worse for
  Japanese and Korean".
- `ContactsView.sections: [{ letter, addresses }]` — the whole book as an
  A–Z directory, `#` last, the BOOK's order inside a letter (favourites first,
  then most-recent). NOT a field on the stored `Contact`: the four-shell rule
  from Phase 6b holds (iOS's codec would have dropped it on the way to disk,
  serde's would have written it — luck of the codec is what the rule is for).
- **Search narrowing stays in the shell** (agreed with iOS): sections cover
  the whole book; a shell that narrows by its search box looks its surviving
  rows up and drops the letters that come out empty. The core has no
  list-search event and gains none.
- Acceptance test, portable: every name in the drawn roster files where the
  drawing files it — `the_drawn_roster_files_where_the_drawing_files_it`
  (`app_contacts` 54 → 57). Web's `letterSections` is now a lookup; iOS's
  local transliteration becomes a deletion; fixtures stay canon (they build
  their own sections, so the test compares the core against the drawing, not
  against another implementation).

Regenerate the tables if ICU's readings move:

```swift
import Foundation
func initial(_ cp: UInt32) -> Character {
    let s = String(Character(Unicode.Scalar(cp)!))
    let latin = s.applyingTransform(.toLatin, reverse: false)!
        .applyingTransform(.stripDiacritics, reverse: false)!
    for ch in latin.uppercased() where ch.isASCII && ch.isLetter { return ch }
    return "#"
}
// Per codepoint: U+4E00...U+9FFF → HAN, U+00C0...U+024F → LATIN,
// U+3040...U+30FF → KANA, U+0370...U+03FF → GREEK, U+0400...U+04FF → CYRILLIC.
// Hangul: for code in 0xAC00...0xD7A3 { L = off/588; V = (off%588)/28 } —
// lead L ≠ 11 → HANGUL_LEAD[L]; L == 11 (silent ㅇ) → HANGUL_VOWEL[V];
// assert every syllable agrees with initial(code) before writing the tables.
```

### The add-network search that sat on 搜索中 forever

The live `/index/fuse-chains.json` carries chain ids above `u32::MAX`
(7078815900 is the first). `decodeSearchIndex` passed `Number(r.chainId)`
through for every row; `NetChainIndexEntry.chain_id` is a `u32`, so serde
refused the ENTIRE `search_index` result over one row, and the wizard showed
搜索中 with nothing in any log. A row the core cannot represent is now dropped
(`Number.isInteger` and `0..=0xffff_ffff`), with a unit test carrying the
real id. Found by the iOS session, which lost an hour to it; the shell's fault
hook defaulting to silence is the deeper debt and is not fixed here.

### Three more from the founder's own hands, same day

- **The group view's ⋯ opened rightwards into nothing.** The button sits at
  the far right of its column and `DropdownMenu` placed the card's left edge
  at the pointer — measured: x 1416, width 216, viewport 1440. Floating menus
  now measure themselves once mounted and slide back inside the viewport (8 px
  margin) in whichever direction they overflowed — the right-click menus near
  an edge get the same.
- **群发转账 "did nothing".** Two truths behind one report: an EMPTY group's
  CTA was disabled with no words (the caption still read "向本组 0 人转账"),
  and a group of ONE handed off to the token picker, which reads as nothing
  much. Now the empty group's caption says why (`contacts.batchSendNeedsMembers`,
  corpus +1 × 15, pins 1626/1542, jest leaves 22,848), and a group of one is a
  send to that one — the recipient arrives prefilled and the form opens on them
  at once. Two or more seed split mode once a token is chosen, as before.

### Gates

`cargo test app_contacts` **57** (54 + roster acceptance, initial edges,
section order) · clippy clean on the touched files · `gen-core-types`
regenerated (`ContactSection` new, `ContactsView` +1 field) into both mirrors
· root `tsc --noEmit` clean (the Expo empty view gained `sections: []`) ·
`pnpm check` **1410 files / 0 errors** · eslint/prettier clean on the touched
files · unit **870** passed (869 + the over-u32 index test) · build ×15 +
`build:extension` · e2e `contacts-io` **4/4** + `contacts-persistence` **4/4**
on chromium (**8/8** re-run after the same-day additions, with the
empty-group caption asserted). **The wasm moved again**: 08aa37e9ddf9 →
**643c05990ef3**, 3,675,329 → **3,703,606 B** (+28,277 B: the initial tables
for six scripts — 21,984 bytes of them — the sectioning, and one corpus key).
296,394 B remain under `MAX_WASM_BYTES` 4,000,000.

---

## Phase 7 (in progress) — budgets

### T460: the decoders stay lazy — and a budget nobody could trip

`budgets.e2e.ts` gained the decoder budget D45 promised: `jsqr` (a 130 KB
chunk, marked by its own `onlyInvert` literal, which `qr-decode.ts` never
spells) and the zbar glue (the one chunk naming `zbar.<hash>.wasm`) reach
neither Welcome nor the wallet's startup path; the wallet's startup fetches ONE
wasm and it is the core. Then the control, in two steps: opening the scanner
brings both decoders' CODE; only the first DECODE brings the 239 KB zbar
binary. A budget nobody can trip is not a budget, and this one can be.

**The control found one.** Its first run reported the decoders absent
everywhere — including after the scanner opened. `chunkSource()` reads a served
chunk back off the build output, and it read `.svelte-kit/output/client/_app/…`;
since 027 the extension build runs AFTER the web build and re-emits
`output/client` under `app/` (Chrome refuses `_`-prefixed paths), so every
served `/_app/…` URL resolved to no file, every chunk read as `''`, and every
`chunksCarrying` budget — SheetJS off Welcome, the fixture keys off Welcome,
the dApp channel off Welcome, the artifact test's neighbours — **has passed
vacuously for two specs.** `chunkSource` now reads `.svelte-kit/cloudflare`
(what the preview serves) first, and the decoder test asserts that at least one
served chunk was readable before it asserts that any of them carries nothing.

---

## Closeout (T462) — verdicts, deviations, the handoff

### SC-401…410

| SC | Verdict | Proven by |
| --- | --- | --- |
| SC-401 the receive code IS the address; a payment request survives the round trip | **met** | `wallet/qr.test.ts` (jsqr reads the card), `receive-code.e2e.ts` (the rendered path, rasterised and decoded) |
| SC-402 a code is read from a chosen image; a refused / absent camera states its reason | **met** | `scan.e2e.ts` ×5: picked image → send form; the in-send scanner; `denied`, `absent` and an unusable code each get their own sentence |
| SC-403 the share card carries address, code and identicon together | **met** | `liveShareCard` (Phase 2) + `qr.test.ts`; the image ("Save image") composes the three |
| SC-404 a sweep of two assets on one network is ONE operation | **met** | `sweep.e2e.ts`: exactly one `eth_sendUserOperation`, calldata calling USDC and naming the recipient twice |
| SC-405 a token added by address shows the chain's identity, survives a reload, can be sent | **met** (sent: by construction) | `add-token.e2e.ts` ×2: identity from the stubbed chain, listed with its balance, present after reload; it is a `SendToken` like any other — a send of it was not separately driven |
| SC-406 each preference takes effect and survives a reload; number and date presets show in the app's own figures | **met** | `preferences.e2e.ts` (date, number, theme ×3 engines); `live-activity.test.ts` (day headers), `locale-format.test.ts` (money) |
| SC-407 erase leaves nothing; cancel changes nothing | **met** | `preferences.e2e.ts` ×3 engines: localStorage + IndexedDB swept, the one named exception kept; cancel leaves the keys byte-identical |
| SC-408 an export re-imported changes nothing; a collision leaves the existing entry and reports it | **met** (Phase 6b, `vela-wallet-63`) | `contacts-io.e2e.ts`: export → re-import reports "0 added, 1 already existed" and changes nothing; a local rename survives a file spelling the old name; delete → import restores contact AND group; a no-address CSV is refused with nothing written; `app_contacts` proves the same in Rust |
| SC-409 a send completes at desktop width | **met** | `desktop-send.e2e.ts` |
| SC-410 (FR-410) nothing regresses: galleries pixel-unchanged, corpus via the 5-step process, budgets hold | **met with one restated budget** | galleries untouched (fixtures never changed); the corpus grew by 3 words through all six steps; the artifact is **+129 B** (Phase 3) — "byte-identical" was the wrong pin for a feature whose baseline said words would be added; the decoder budget is real and has a control |

### Deviations from the plan, each with its reason

1. **D44 corrected in Phase 2**: the plan called Expo's hand-rolled `qrcode.ts`
   the porting truth; nothing imports it. The encoder is the `qrcode` package
   the phone ships, and the port is 20 lines of `qr-path.ts`.
2. **T421 not a port**: `image-decode.ts` is a pure-JS JPEG decoder for a
   platform with no canvas. A browser decodes images itself.
3. **The corpus grew by 3 words and the artifact by 129 bytes** (Phase 3); the
   cargo line in the corpus recipe needs `i18n-all,crux`.
4. **`avatar-style.ts` folded into `preferences.svelte.ts`** (Phase 4): its 54
   lines are a cache, a listener set and a version counter — all three are
   `$state`. The artwork was ported.
5. **Token amounts get the decimal mark, not the grouping** (Phase 4); money
   gets both. The mocks draw token amounts ungrouped.
6. **`multi_specs[].amount` is a HUMAN decimal string** (Phase 5) — the first
   overlay converted it twice; the e2e caught it.
7. **The add-token sheet has no network picker and no native tab** (Phase 5):
   the core probes every chain; the native tab is `network_admin`'s.
8. **T450–T452 reassigned** (Phase 6) to `vela-wallet-63` by the founder —
   and done in Phase 6b, with the format in the core rather than a TS port.
9. **Gates moved to an isolated worktree** (Phase 6) after the shared preview
   crashed twice under three engines with three sessions building.
10. **`chunksCarrying` had been vacuous since 027** (Phase 7): fixed, and the
    decoder budget carries a positive control.

### T461 — the device pass (a human with a phone)

Not performed here; the ladder was measured on hardware and the port must
keep it. Steps, five minutes, one phone with a rear camera:

1. Open `wallet.getvela.app` (or the preview URL) **over HTTPS** on the phone,
   sign in, tap Scan. Expect the browser's permission prompt, then a live
   viewfinder. Deny once and reopen: expect the "camera access is needed"
   sentence, not a black frame.
2. Show another phone's receive code (or a printed one) to the camera at
   normal room light, about 20 cm away. Expect the address in the send form
   within two seconds.
3. Photograph a code with the phone's own camera app, then in Vela tap the
   photo tool and pick that photo. Expect the same result — this is the zbar
   ladder at 1200/1000/800/600/400 on a real JPEG.
4. Pick a screenshot of a code (clean pixels, cropped quiet zone). Expect it to
   read — this is the jsQR fallback the ladder ends in.
5. Open the page over plain HTTP on a LAN address and tap Scan. Expect the
   "secure connection" sentence.

If any step gives a black frame with no sentence, that is a bug in
`scanner.svelte.ts`'s classification, and the console's `[send]`-style
warnings will name the `getUserMedia` error it did not recognise.

### 029 handoff

- **What the next feature inherits**: every drawn surface of 024–027 is live,
  the contacts import/export and the phone's export entry (the drawn `addMenu`
  sheet) included since Phase 6b. The desktop send
  drives the same session as the phone; the sweep and add-token screens are
  wired to their machines; the six preference rows work; erase erases.
- **Measured, not assumed**: the core artifact is `vela_core_bg.7caac430e4b3.wasm`
  at 3,630,793 B at this branch's last commit by this session; a later session
  rebuilt it for the contacts core — landing at `643c05990ef3`
  (3,703,606 B; Phases 6b–6c) — the fingerprint test reads whatever is served,
  so it holds either way.
- **The isolated gate is reusable**: `git worktree add --detach
  /Volumes/data/production/vela-wallet-e2e <sha>`, apply a patch, `pnpm
  install --offline`, `pnpm sync:wasm`, `playwright.isolated.config.ts` on
  4174. Remove the worktree with `git worktree remove` when done.
- **Debts named for Penpot/029**: SD1b has no desktop twin; the add-token
  sheet's network picker and native tab; the desktop's drag-a-contact-onto-a-
  group (018's `dropTarget` board — the tick-list covers the need); the
  desktop's own import path (it still parses files in its shell; the core's
  `import_file` is there for it); `formatUsd` in `activity.ts` still writes a
  `$`-formatted string into a stored record (the Expo contract) rather than a
  number.

### T463 — final sanity, at the branch's last commit by this session

Shared tree, after the third session fixed its in-flight types: `pnpm check`
**1409 files / 0 errors** with `gen-core-types --check` current (11 session +
315 wallet-state mirrors) · unit **869** passed · build ×15 +
`build:extension` · e2e at `a6683a53` in the isolated worktree on 4174,
`--workers=2`, three engines: **181 passed / 1 skipped / 0 failed** in 4.1
minutes, at machine load 77→38. The one skip is 027's `test.fixme` for SC-304,
this feature's stated precondition and not its task.

**Open at close**: T461 (a human with a phone, five steps above). SC-408
closed in Phase 6b. Everything else this feature named is done and measured.


---

## Phase 8 — Replacing the Expo web: the six gaps (2026-09-05)

The founder's ask after the closeout: an audit of app-web against the Expo
web, then "都完成掉，然后我要替换掉 expo web". The audit (this session,
grounded in 024–028's results and the source) rated seven areas: receive,
send and activity were live with real-network evidence; add network had no
end-to-end; three settings buttons were dead; the account switcher was a
picture; displayed-equals-signed had no cross-check; and the extension
signing path was broken at SC-304. Six gaps, six tasks (T470–T476), one
branch, no new spec directory — the founder wanted it on 028.

### T470 — SC-304, met: the machine had no signers

027 measured everything up to the silence: the slide committed, the gate was
open, `approve_tapped` reached the resident, `navigator.credentials.get` was
never called. The cause was one missing event. `sign_request`'s
`approve_with` does

```rust
let Some(signer) = model.accounts.get(model.active_index as usize).cloned() else {
    return Command::done();
};
```

and on the web nothing ever dispatched `accounts_changed`, so `model.accounts`
was empty for the life of the page. Expo's resident fed the rows from the
wallet provider (`setSignAccounts`, `src/services/wallet-state-core/sign-resident.ts:442`);
the port dropped it because the web has no provider — the session is
app-resident state. The fix is `SignRequest.syncAccounts()`
(`sign-resident.svelte.ts`): the session's rows as `{address, credential_id}`
plus the active position, deduped by a key, dispatched on boot, from an
`$effect.root` on the session view, and — synchronously, on the loop, not a
microtask later — inside the `switchActiveAccount` ack, because the machine
reads its own rows at `active_index` the moment the ack lands. The two pages
that hand requests in (`request/+page.svelte`, `wallet/+page.svelte`) call it
beside `syncNetworks()`.

Proof: `extension-signing.e2e.ts` — the `test.fixme` is a `test`; the four
tests pass on the isolated preview (5.0 min including the build). The
SC-304 assertion is unchanged: the dApp receives an EIP-191 signature over
100 hex characters, made by the account's own key.

### T472 — displayed = signed, by a second implementation

The audit's honest finding: what a passkey signs is 32 bytes it cannot show,
and between the core's confirm view and those bytes sat the shell's
TypeScript assembly with nothing checking it. Now three checks stand before
and after every passkey prompt (`services/sign-attest.ts`):

1. **The calldata is the calls that were shown.** 032's `user_op.rs` (the
   desktop's Rust assembly, with its 21 vectors) is on this branch, and the
   wasm exports `attestSafeOpHash(op, calls, chainId)`: the core rebuilds the
   MultiSend from the inner calls and the fee leg's INPUTS (token, recipient,
   amount — so a changed recipient is caught, not only changed bytes) and
   refuses if the shell's `callData` differs.
2. **The hash is the core's hash.** The SafeOp EIP-712 hash the shell
   computed must equal the core's to the byte; the message path likewise
   (`attestSafeMessageHash`, the `SafeMessage(bytes)` digest).
3. **The assertion is over that hash.** `clientDataJSON` must be a
   `webauthn.get` whose challenge decodes to exactly those bytes — a swapped
   override or a broken bridge cannot substitute a challenge.

Wired at the three SafeOp sign sites (`sendUserOp`, `sendUserOpTempo`,
`sendUserOpInBand`) and the three message sites in `dapp-submit.ts`. Any
disagreement throws `SignAttestError`; nothing is signed or submitted.

What this does NOT claim: a shell that is itself malicious can skip the
check. The trust model is unchanged — you trust the JavaScript getvela.app
serves — but a drift, a bad port, a wrong constant or a tampered bridge now
fails closed instead of signing something other than the screen.

Proof: `sign-attest.test.ts` (the shell and the core hash one operation
identically; a tampered calldata, a smaller fee, another relay, another
chain's hash and another challenge are each refused) and the 21 `user_op`
vectors under `cargo test -p vela-core --features i18n-all,crux user_op`.
`send-lands`, `sweep`, `batch` and `desktop-send` still pass — the two
implementations agree on every operation those suites sign.

### T471 — the account switcher is live

`identity.ts` said it plainly: everything but the active row's name was the
fixture layer, and `AccountsSheetBody` had no handlers. Now
`withLiveAccounts` / `withLiveAccountsDesktop` (`settings/live.ts`) fill the
sheet from the session's rows; the totals come from the balance core's own
switcher (`switcher_opened` with every address — the policy Expo's
`AccountSwitcherModal` sourced from, already in `balance_dashboard`), with
the live total standing in for the active account; a tap is
`session.switchAccount(row.index)` in the session's domain, which persists
the index; "Create New Account" leaves for `/create` and "Sign In with
Existing" for Welcome (both end in `CompletionMode::AddAccount`, which
appends and activates). Proof: `accounts.e2e.ts` — two seeded accounts, the
second chosen, the header renamed, a reload keeping it, the wallet route
agreeing; and the two buttons landing on their routes.

### T473 — add network, end to end

`add-network.e2e.ts` stubs the search index (one entry), the chain document,
the RPC race, the eleven required contracts (`eth_getCode` → deployed) and
the P-256 precompile (`eth_call` → one), and drives: query "10" → suggestion
→ "Compatible" → "Add Network" → the networks page lists it as Custom → a
reload still lists it → its row's remove → gone, and still gone after a
reload. The wizard, the verdict and the registry are the core's; the test
touches only what a person does.

### T474 — the storage page clears what it says, and says what it holds

`services/device-storage.ts` classifies every key of ours (both stores: the
`vela.` namespace in `localStorage` and the IndexedDB `kv`) into the three
drawn groups by row: transactions, contacts, custom tokens/networks and
browsing are "your data"; balances, rates and the scan caches (token
metadata, identity lookups, RPC bans) are "cache"; grants, requests and the
extension snapshot are "sessions"; anything unnamed is your data, so a sweep
can never take it. "Clear all caches" is exactly the cache group plus the
memory caches beside it; each user-data row's Clear is its own key list; the
dApp row stays `dapp_permissions`' (027). The page's headline, bar and meta
lines are measured (`withLiveStorage`) on entry and after every clear.
Proof: `device-storage.test.ts`.

### T475 — the rescues open where they were drawn

023 drew SR2/SR3 over the wallet and SR4 over the send, as settings
components; the audit found their models fixture and their entries absent.
Now the balance status line (`BalanceDisplay`'s `onstatus`, threaded through
both wallet surfaces) opens SR2 for the first unreachable chain — the row's
override is expanded, the typed URL saved and blurred (the core probes it,
refuses a chain-id mismatch, stores the override, invalidates the pools),
the sheet turns restored on a healthy probe, and Done sends
`fix_chain_resolved` so the balance core retries that chain alone — and SR3
otherwise (`liveBalanceDetail`: rate-limited chains retrying, unreachable
chains with a retry, settled chains largest first, the hero's own figures).
The send core's `treasury_bootstrap` opens SR4 (`liveRelayer`: the
treasury's real address as a real code, the shortfall in the asset's units,
copy, and "I've funded it" → `retry_after_bootstrap`; dismiss →
`dismiss_treasury_sheet`). Phone: a bottom sheet; desktop: the settings
Dialog. Proof: `home-truth.e2e.ts` — the dead endpoint's status line, the
sheet naming Ethereum, a working URL saved, the restored callout, Done, the
status line gone and the balance back.

### T476 — a record can be deleted

Expo had swipe-to-delete with a tombstone and the web detail had no way to
say it. `TxDetailModel.deleteLabel` (live only — the drawn fixtures stay
pictures) renders a danger button; `ondeletetx` threads through
`FlowsPanel` / `FlowsMobile`; the route calls `feed.deleteRecord(id)` —
`activity_feed`'s `DeleteRequested`, an optimistic remove plus a tombstone,
then `DeleteTxRecord` — and steps the sheet or column back to the list. One
new corpus word, `history.deleteRecord`, in 15 locales (Expo had none: the
row simply went). Proof: `activity-delete.e2e.ts` — a seeded confirmed send,
its detail's delete, the row gone at once, the store without it, a reload
without it.

### Budgets, restated

- **Corpus**: +1 leaf (`history.deleteRecord`) → 1,627 paths / 1,543 leaves;
  22,863 leaves across 15 locales; lint, verify and the 50,000 fuzzed bundles
  green; vectors regenerated.
- **The core artifact**: 3,675,329 B (6c) → **3,723,773 B**
  (`vela_core_bg.1e2d3a9eabc8.wasm`), +48,444 B for `user_op` (1,156 lines,
  the desktop's), the two attest bindings and the one word. Under the
  4,000,000 ceiling; the fingerprint test reads what is served.
- **Nothing new on Welcome**: the attest module is imported only by the
  money path; the rescue bodies only by the wallet route.

### Deviations, each with its reason

1. **No new spec directory.** The founder said "就在当前分支里干"; the six
   gaps are Phase 8 of 028, in this file and `tasks.md`.
2. **`calculateSafeOpHash` is exported** so the test can pin the shell's hash
   against the core's; nothing else imports it.
3. **The rescue sheets use the settings corpus on the wallet route** — a
   `pickRescueMessages` slice ships with the page, not the whole table.
4. **The storage page's user-data clears are one tap**, as drawn; they clear
   raw stores, and a contacts screen open in another tab reads the change on
   its next boot, not live.
5. **`user_op.rs` is a verbatim copy of 032's**, not a merge: the desktop
   branch owns it; when 032 lands, git sees one file.

### Gates (T477) — at the branch's last commit by this session

- `pnpm check`: gen-tokens, sync-wasm (`vela_core_bg.1e2d3a9eabc8.wasm`,
  3,723,773 B) and `gen-core-types --check` current; svelte-check **1,414
  files / 0 errors**.
- `pnpm lint`: Prettier and ESLint green (one `svelte/prefer-svelte-reactivity`
  finding on the settings page, fixed with `SvelteMap`).
- Unit: **65 files / 885 tests** on both vitest projects, including
  `sign-attest.test.ts` and `device-storage.test.ts`; the root jest i18n
  suites 75/75 with the leaf pin at 22,863.
- Corpus: gen:i18n (1,627 paths / 1,543 leaves), lint:i18n (no new defects),
  verify:i18n (73,145 comparisons, zero divergences), vectors regenerated.
- Rust: `cargo test -p vela-core --features i18n-all,crux user_op` — 21 passed.
- e2e, isolated 4174 preview, `--workers=2`, chromium + firefox + webkit:
  **193 passed / 0 skipped / 0 failed in 3.5 minutes** — 12 more tests than
  the closeout's 181, and the one `fixme` gone. A first attempt of the same
  run failed 48 with "could not connect to the server": the preview had died
  under four gates running beside it and eight orphaned `workerd` processes
  (hours old, parent 1) from earlier sessions. Killed the orphans, ran the
  suite alone, and it was green — the 026/027 lesson, measured a third time:
  the preview is one process, and it does not share.

### Three findings from the e2e, kept as facts

1. **A chain the wizard already has is refused, not re-added** — the first
   draft of `add-network.e2e.ts` asked for Optimism (chain 10), a built-in,
   and the core answered `AlreadyAdded`. Linea (59144) is the test's chain.
2. **The custom row's delete control was named "Add Network."** `NetworkRow`'s
   `deleteLabel` borrowed `networks.addLabel` since 023, so a screen reader
   heard two "Add Network" buttons, one of which removes the network. It now
   says `settingsModals.network.removeTitle` ("Remove Network") on both
   widths — a new field, `networks.removeLabel`, on both settings models.
3. **`fix_chain_resolved` alone does not re-read.** The balance core's retry
   is a throttled fetch like any other; Done and the breakdown's retry now
   also force a refresh, and the RPC-fix e2e watches the balance come back.

## Phase 9a — The founder's first list after Phase 8 (2026-09-05)

Four asks, from screenshots of the running app: the header's name should
switch accounts (with create / sign-in beside it); every Nimiq identicon
should open big, "so a person can tie the artwork to the address"; the
sidebar's "搜索或执行 ⌘K" field should go; and every text field should stop
shouting — "不要这么多 active 边框 outline 样式". A fifth came from the
desktop screenshot of the viewer itself: the card spanned the window.

### The account switcher, from the header

`WalletHeader`'s name-and-chevron button had an `onclick` prop that no screen
passed: the chevron drew a disclosure that led nowhere, and the only switcher
was three taps away on the settings screen. `AccountSwitcher.svelte`
(`session/ui`) is that switcher hoisted out of the settings models: it builds
the sheet from the session's rows and the balance core's switcher cache
through a new `liveAccountsSheet` (the settings page's `withLiveAccounts*`
now share it), opens `switcher_opened` on mount and closes it on unmount as
the settings page does, and a pick is `SwitchAccount` in the session's
domain. A bottom sheet on the phone, a `Dialog` on the desktop, with the two
buttons the settings sheet has. The wallet and contacts routes mount it from
their sidebar/header; the settings desktop's header jumps to its own account
page instead of drawing a second copy. The copy is the settings corpus's
`accounts` slice, shipped by the wallet and contacts loaders.

### The viewer, from every artwork

Threading an `onidenticon` callback to every place an identicon is drawn —
contact rows, recipient cards, the signer line, switcher rows — would have
made each new artwork a plumbing job three layers deep, and the first one
forgotten would have been the one that mattered. So the question "which
address drew this?" is asked in one place: `identicon-viewer.svelte.ts` is
an app-resident store (like the session), `Identicon` given an `address`
renders a BUTTON that opens it, and one `IdenticonViewerHost` per signed-in
route draws the viewer. The header's `onidenticon`/`identiconViewerLabel`
plumbing is gone with it.

Rows that were themselves `<button>`s (`ContactRow`, `ContactPickRow`,
`AccountsSheetBody`, `AccountRow`) now hold the artwork BESIDE their button —
nested interactive content is invalid HTML and one control to a screen
reader. The artwork button is named by the corpus (`identiconViewer.a11yOpen`),
never by the row's name: two buttons named "Alice" would be one control to
assistive technology and a strict-mode collision in every e2e that finds a
row by name. Left as pictures on purpose: the QR centres (`ShareCard`,
`ContactQr`), the group pick list (a `listbox` may hold only `option`s), and
the gallery (no host, no seed). Seeds added where the models lacked them:
`addressFull` on the two account rows and the contact-pick rows, `address`
on recipient cards, the identicon fact lead, the confirm breakdown, the
recipient field and the signer.

Layering: the viewer sits on its own `z-index: 20` layer, above the
switcher's dialog (z 11), so an artwork tapped inside a dialog opens over it
rather than under it.

### No command bar

The "搜索或执行 ⌘K" field ran nothing: an input with no handler and a `<kbd>`
promising a palette that does not exist. Removed with its model field, its
fixture line and the three `commandBarPlaceholder` message consumers; the
corpus key stays (the six-step corpus change is not worth a removed lie).

### One quiet focus rule for text entry

Two rings on the token search: the global `:focus-visible` halo (app.css)
on the input INSIDE the field, and the field's own accent `:focus-within`
outline around it. Elsewhere five components each defined their own accent
focus. Now `app.css` owns text-entry focus: `[data-field]:focus-within`
draws one hairline in `fg.muted` INSIDE the box (`outline-offset: -1px`, so
nothing reflows and borderless wells get an edge without a border), the
halo skips text entry and `data-field` boxes, and an invalid / toned field
keeps its own colour while focused. Every field carries `data-field` — the
input itself when it is its own box, the well around a borderless input
otherwise — and the five per-component accent rules are gone. The `Dropdown`
trigger takes the same rule (a form control beside fields). Buttons, links
and rows keep the keyboard halo: that one never showed on a click.

Never the accent on a field: accent is for the one action that moves money,
and a person typing is not there yet (CLAUDE.md rule 4, applied).

### `--layout-promptCard` did not exist

Spec 019's results recorded declaring it through `WEB_ADDITIONS`; it never
was. `PromptSheet`, `SignOutSheet` and `IdenticonViewer` all read
`max-width: var(--layout-promptCard)` as invalid and spanned the desktop
window — which is the "太长了" the founder saw. Declared at 440px (a
42-character address in the mono face on one line inside the card padding).
Lesson kept: a token referenced by a component must be grepped in the
GENERATED file, not trusted from a results entry.

### Gates

- `pnpm check`: 1,418 files, 0 errors, 0 warnings.
- `pnpm lint`: Prettier and ESLint green (one `svelte/prefer-svelte-reactivity`
  finding on the switcher, fixed with `SvelteMap`; one literal-audit finding —
  `box-shadow: none` is a literal to `tokens.test.ts` — fixed by not needing
  it: the halo rule skips text entry, so there is nothing to reset).
- Unit: **66 files / 890 tests** (+5: `liveAccountsSheet` ×2, `Identicon`
  browser tests ×3).
- e2e, isolated 4174 preview, `--workers=2`: the full three-engine run put
  **chromium 144/144** green — every suite that walks a contact row, a
  switcher row or a field is on chromium — and lost the preview at 4.3 min,
  exactly as Phase 8 recorded (firefox 21 + webkit 27 failed with
  `NS_ERROR_NET_ERROR_RESPONSE`, no assertion among them). The firefox +
  webkit storage projects re-run alone on a fresh preview: **54/54 in
  2.4 min**. New coverage: `accounts.e2e.ts` (+2: the header on both widths),
  `identicon-viewer.e2e.ts` (+3: header, settings row, over the switcher).
- Visual check (a temporary spec, deleted): six screenshots read — the phone
  switcher sheet, the viewer over it, the token picker's search with the one
  quiet edge and no ring, the desktop switcher dialog with the sidebar minus
  its ⌘K field (the two "home" frames caught the launch animation). The
  desktop viewer frame could not be read in this session, so its width is a
  permanent assertion instead: `identicon-viewer.e2e.ts` measures the card
  at ≤ 440px on the wide layout. Chromium, isolated preview: 8/8 across
  `accounts` + `identicon-viewer`.

## Phase 9b–9f — The founder's second pass, executed (2026-09-05)

"可以的 按你推荐得来": the three RULINGs went as recommended, and the plan's
seventeen tasks were done in the plan's order. What each one turned out to
be, beyond the plan's one-liners:

### 9b — Truth first

- **T480** `BalanceModel.decimalMark`, from `numberSeparators().decimal`; the
  display draws `.` only for the fixtures. Unit test under two presets.
- **T481 / T482** `liveReceiveList` counts what it lists and each row names
  its chain, its whole address and its logo; the page keeps
  `selectedReceiveChainId` from the tapped row's index (`receiveNetworks()`
  is the one order both walk) and `liveReceiveQr` words the title, marks the
  centre and targets the explorer from it. R3 — told apart by its contract
  line — is about `selectedToken` instead. `live-receive.test.ts`;
  `receive-code.e2e.ts` counts the rows against the registry and reads the
  second chain's name off the second code.
- **T483** The phone's token sheet went live for the first time
  (`liveTokenDetail`): the held token's balance, price, contract (whole, to
  copy), decimals, network, its feed rows and its explorer page. The row's
  index rides `onnavigate('token-detail', i)` on the phone; on the desktop
  the same tap opens the asset column instead of pushing a step the stack
  never had.
- **T484** `withLiveDesktopFlow` re-titles the column from the live body's
  own `header.title` — one rule for every body that carries a header, so the
  picker, form, confirm and receipt titles all follow.
- **T485** `withLiveConnections` is generic over both models; 浏览记录 is
  filtered out of the storage groups on the web (its key list is empty by
  construction here).

### 9c — Dead controls

- **T486** `services/clipboard.ts` — one `copyText`. Every copy affordance
  writes through it: the receive rows (the whole address rides on the row
  now), the QR's address and contract, the phone token facts, the tx detail
  facts (`copyValue` carries the un-shortened counterparty / hash), and the
  desktop asset column's contract fact, which gained the copy affordance the
  phone's had.
- **T487** Explorer controls are LINKS, not handlers: `Button` learned
  `external` (new tab, no opener) and the two plain buttons became anchors.
  Targets: the account page for a network's code, the token page scoped to
  the account (`/token/<contract>?a=<account>`) for a held token, the tx
  page for a record with a hash. A chain with no explorer draws the control
  inert rather than pointing it somewhere wrong.
- **T488** 保存图片 produces a PNG: `share-image.ts` composes R4 as an SVG
  string (the drawn geometry; colours read from the live tokens at save
  time; the app's two faces embedded from our own origin so the picture
  matches the screen; the identicon nested in the centre; the brand mark
  from `ui/brand-mark.ts`), draws it at 2× and hands it over through
  `file-io.saveBlob`. The browser test rasterises the card and DECODES the
  code back to the address — the identicon in the centre, the 2× draw and
  the PNG all survived; the e2e asserts the download and its name. The two
  files are audit-whitelisted for the same reason `BrandMark.svelte` is.
- **T489** 最大 dispatches `tap_max`; the send e2e reads a fill of `1.4…`
  for a 1.5 ETH balance — the core's reserve math, never the raw balance.
- **T490** The form's own quote, in `send.rs`: a complete single-transfer
  form (valid payee, amount > 0) arms a 400 ms `StartTimer { FormEstimate }`;
  the newest timer alone is answered and issues the exact `EstimateFee`;
  the answer is the same chain-guarded `fee_estimate` every surface reads,
  keyed by token · payee · fee coin so the same form is not quoted twice
  and a changed fee coin is. Continue's pre-check takes the pipeline over
  and a late debounce asks for nothing; the Max, warm-up and submit
  pipelines are never disturbed. Three rule tests; eight existing tests and
  the `continue_to_confirm` helper learned to drop the debounce the way they
  already drop the 15 s timer (`DomainDriver::drop_matching`). wasm
  rebuilt: `vela_core_bg.cbfa8d797399.wasm`, 3,725,914 B (+2,141 B).
  The send e2e asserts the fee row names a figure BEFORE Continue.
- **T491** 货币 on the desktop: `SettingsPrefEvent` gained `currency`, the
  route answers it with `currency.choose`, `withLiveCurrencyDesktop` fills
  the row's value ("USD · $1,234.56") and its menu. The list is
  provider-driven on both widths — the Chainlink feed codes ∪ the
  configured endpoint's coverage (`currency-catalog.ts`), USD first, the
  drawn eight as the floor before anything answers — named by
  `Intl.DisplayNames` in the page's locale rather than a corpus of currency
  names. Not e2e'd: the desktop switch itself (the phone's conversion is
  `rate-live.e2e.ts`).

### 9d — Logos, once (T492)

`flows/marks.ts`: `chainMark`, `tokenMarkFor(chainId, symbol, contract,
namedLogos?)`, `balanceTokenMark` — the asset rows' logo triple built by one
rule and worn by the send picker rows, the token card, the fee row and its
options, the sweep's chain notice, the confirm page's chain fact, the
receive rows (the chain's logo over the lettered badge), the QR centre and
the token screen. The share image draws the network mark as the lettered
disc — a logo would be a cross-origin fetch inside an `<img>` document —
recorded, not hidden.

### 9e — The doors and the filter (RULINGS 1–3)

- **T493** 转账 from a token opens the form with it chosen: the asset
  column and the phone token sheet name their token, the route opens the
  send session with `preselected_symbol` + `preselected_network`
  (`networkId(chainId)` — the same string `fetch_tokens` stamps), and the
  core lands on `enter_details`; back is the picker.
- **T494** 收款 from a token enters `receive-token` (`['r1','r3']` /
  `['dr1','dr3']`): the token's code, contract line, logo and
  `qrTitleAsset`, and its share image.
- **T495** With a network chosen in the sidebar, 收款 enters R1 and pushes
  R2 for that chain at once — the list is one step back.
- **T496** The sidebar draws its network section only when given rows; the
  contacts and settings routes pass none and their `pickChain` navigations
  are gone. The gallery's DC/DST boards still carry the list until they are
  re-exported (canon is the design source, not this code).
- `token-doors.e2e.ts` proves the four entries on the stubbed chain.

### 9f — The careful pass (T497), honestly

Done as an automated matrix, not as the manual walk the founder asked for:
every fix above has a unit or e2e assertion, the full suite ran on three
engines, and the earlier visual pass covered the switcher, viewer and
picker focus. What has NOT been done is a person walking every drawn control
on both widths against `MANUAL-TEST-100-CLUES.md`. One leftover found on
the way and left as it is: the group pick list's identicons stay pictures
(listbox semantics, recorded in 9a). The add-token result mark, found on
the same walk, now wears the found token's own logo by its contract.

### Gates (T498) — at the end of this session, tree uncommitted

- `pnpm check`: 1,424 files, 0 errors, 0 warnings (`sync-wasm` on
  `vela_core_bg.cbfa8d797399.wasm`, core types regenerated —
  `SendTimerTag` gained `form_estimate` on both the web and the Expo-side
  generated dirs).
- `pnpm lint`: Prettier and ESLint green (the external explorer anchors carry
  `no-navigation-without-resolve` suppressions with their reason; the
  currency catalog lost a useless assignment).
- Unit: **68 files / 899 tests** (+9 this phase: `live-receive.test.ts` ×5,
  `share-image.svelte.test.ts` ×2 including the decode, the decimal-mark
  test, the `liveAccountsSheet` pair from 9a already counted).
- Rust: `cargo test -p vela-core --features i18n-all,crux --test app_send`
  — **96 passed** (93 rules as before, 3 new; 8 tests and one helper taught
  to drop the form quote's debounce).
- wasm: `rust/scripts/build-web.mjs` → 3,725,914 B (+2,141 B over Phase 8),
  under the 4,000,000 ceiling; synced.
- e2e, isolated 4174 preview, `--workers=2`: the full three-engine run put
  **chromium 143/144** green with ONE real failure — the phone's currency row
  had grown a sample after its code, and `settings-persistence` looks for
  the bare code; the row is the code alone again (the desktop keeps the
  sample) — and lost the preview at 4.2 min as Phase 8 and 9a did. Re-runs:
  chromium `settings-persistence` + `receive-code` + `send-lands` **8/8**
  (2.4 min); firefox + webkit storage projects **54/54** (3.0 min) — a first
  attempt of that re-run lost the preview at 4.4 min because `pnpm check`
  and vitest were running beside it, which is the 026/027/028 lesson
  measured a fourth time: nothing else runs while the preview does.
- New e2e: `token-doors.e2e.ts` (+4: 转账 / 收款 from the asset column, the
  filter's shortcut, the phone token sheet's 转账), `receive-code.e2e.ts`
  (+1: twelve rows, the tapped chain's title, the PNG download),
  `send-lands.e2e.ts` (Max's `1.4…` fill and a fee figure before Continue,
  inside the existing walk).

## Phase 10 — The founder's third pass (2026-09-05, ten screenshots)

Ten defects reported against `app-web/vela-wallet` after Phase 9, every one
of them a drawn control that did not do what it said or a rule the pushed
screens did not share with the home. What changed, by number:

1. **Add-token toggle.** The ERC-20 / native tabs switched nothing. The
   native half is now `network_admin`'s add-network wizard driven through
   the app-resident session Settings already uses: type a name or chain ID,
   the index's matches are listed (`AddTokenResult` gained `suggestions`), a
   tap probes the chain, the card carries the verdict — and an inconclusive
   probe says "unable to verify" (`settingsModals.addNetwork.unableToVerify`
   joined the flow's key list), never "not compatible". The add lands through
   `add_confirmed`; the ledger's `last_added_chain_id` flips the chip and
   refreshes the balances. Closing the sheet resets the wizard.
2. **Share image.** Redrawn to the reference: the identicon clipped to a
   circle (the clip on a `<g>` in the card's space — on the nested `<svg>`
   it was read in the artwork's 64-unit space and blanked the centre), the
   chain's logo fetched and EMBEDDED as a data URI (an SVG-in-`<img>` cannot
   reach across origins, which is why Phase 9 drew a lettered disc), a white
   foot with one curve across its top, and the canonical app icon
   (`design/icon/app-icon.svg`, now `APP_ICON` in brand-mark.ts and
   `AppIcon.svelte`) beside the wordmark in ink. `ShareCard.svelte` matches.
3. **Picker chips + network filter.** `visibleSendTokens` narrows the
   core's list by the sidebar's chain and the class chip (gas = the chain's
   own coin, stable = the symbol table `activity_feed.rs` mirrors, other =
   the rest); the picker's indices, `select_token` and `toggle_all`'s
   `visible_ids` all walk that one list.
4. **Phone network filter.** The pill on A1 / T1 / SD1 raises a chain sheet
   (`liveChainRows`, the sidebar's rule) that sets the same `chainFilter`;
   the pushed assets and history screens now narrow to it as the home does,
   and the page indexes their taps through the same narrowed lists.
5. **Fee / Max latency.** Measured at 5–6 s from a complete form. Two
   halves: the core now warms a transfer-sized quote the moment a token is
   picked (`select_token` → `warm_estimate_start`, the sweep's own path;
   the preselected and prefilled `open` paths too), and re-arms the form's
   payee-aware quote when the warm one lands; the shell calls the
   `prefetchForSend` the core always said was its (deployment, nonce, gas)
   on selection. The fee row keeps a figure in hand while a re-quote is out.
   Probe on the dev server against real relays: fee 5.8 s after the pick
   (public RPCs answering 429), Max **26 ms** — the wait moved behind typing.
6. **Split rows.** `RecipientCard` is editable live (address + amount per
   row, the book's door per row); "+ add recipient" appends a blank row on
   the split form; "from contacts" appends a row and opens the picker
   targeted at it (shell-minted `rcpt_s{n}` ids, the core's `picker_target`
   rule); sweep rows carry the logo triple. Both hosts pass the handlers.
7. **Two columns.** A door taken while a send is open closes that send
   (`openSend` used to return early and the picker stayed beside the token's
   detail); tapping an asset closes whatever flow held the column; the asset
   panel yields while a flow shows (`selectedToken` masked for the desktop
   model, kept for the flow's own inputs).
8–9. **Phone settings.** The 通讯录 and 反馈 rows are dropped on the phone
   (the tab bar has the book; feedback is not wanted here); the desktop nav
   keeps its list.
10. **⌘F badge.** Gone from the live contacts search: nothing listened.

Gates: `svelte-check` 1,429 files / 0 errors; vitest **70 files / 912 →**
(+7 tests: picker narrowing ×6, share image ×2, one fee-stickiness);
`cargo test -p vela-core --features i18n-all,crux` all green (`app_send`
96, six tests taught to settle the warm quote); wasm
`vela_core_bg.2c025da3bc54.wasm` 3,725,860 B (−54 B). Playwright probe on
the dev server (parallel space, real relays) walked all ten on both widths.
