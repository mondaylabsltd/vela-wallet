# Research — 028 Web Port Completion

Decisions D44–D52, continuing the sequence (024 D1–D8 · 025 D9–D14 · 026
D15–D30 · 027 D31–D43). Most of this feature's hard questions were answered by
earlier work and are CARRIED, with the source named — re-deriving a measurement
someone already paid for is how a program forgets what it learned.

---

## D44 — The encoder ports; the decorative pattern stays where a placeholder is meant

**The defect**: `src/lib/wallet/qr-pattern.ts` says it in its own header —
*"It never encodes data — spec 021 keeps real QR encoding out of scope, and a
pattern that looked scannable but wasn't would be worse than one that plainly
isn't."* The comment was honest about a placeholder. What made it a trap is that
021's card graduated into a live receive screen in 025 and nobody replaced the
generator, so the honest placeholder became a dishonest product.

**Decision, CORRECTED before a line was ported**: use the `qrcode` npm package
(1.5.4) — the one Expo actually ships — and port the 20-line `qr-path.ts`
beside it.

The plan said to port `src/services/qrcode.ts`, 554 hand-rolled dependency-free
lines. Checking its callers first found there are none: **nothing in the Expo
tree imports it.** Expo's real encoder is `qrcode`, used by
`components/QRCode.tsx` and `components/ui/TransactionReceipt.tsx`. Porting 554
lines of code that has never run in production, under a heading that calls it
"the porting truth", is the worst kind of port — it looks verified and is not.
A round-trip test would eventually have found whatever is wrong with it; not
writing it is better.

`qr-path.ts` (20 lines) does come across, and it earns its place: it merges
consecutive dark modules in a row into one `h` run so the whole code is a single
SVG path. Per-cell rendering produces hairline white gridlines from pixel
rounding, which is a code that photographs badly.

**Measured while deciding**: a plain address encodes to **29 modules at version
3** — exactly the `RECEIVE_MODULES = 29` the card was drawn at, so 021's
geometry was chosen against a real code. A 131-character EIP-681 payment link
encodes to **49 modules at version 8**, which at the card's fixed 344px is about
7px per module — still scannable, but it means the card must accept a varying
module count rather than assume 29.

`qr-pattern.ts` is NOT deleted. The galleries are canon and their screenshots
are diffed; a fixture that suddenly encoded a real address would change every
one of them and make the gallery a place where real addresses appear. It stays
exactly where a placeholder is what is meant, and the live builders stop using
it.

**How it is proven**: not by rendering. SC-401 is a ROUND TRIP — the rendered
code is decoded by the decoder from D45 and must return the address. A test that
asserts "a QR appeared" is the test that let this ship in the first place.

---

## D45 — Decoding: zbar WASM primary, jsQR fallback, and a preprocessing ladder

**Carried whole from `docs/qr-scanner-web.md`**, which recorded the measurements
that matter and cost real time to get:

- **iOS Safari has no `BarcodeDetector`** — the platform most likely to be
  scanning a wallet's QR code is the one that cannot use the browser's own
  decoder.
- **jsQR alone cannot read a real camera frame or photo.** It handles clean
  digital screenshots; it fails on JPEG noise, moiré and the lighting a phone
  actually produces.
- **zbar (a C barcode library compiled to WASM) is the only viable decoder**,
  and only with the right preprocessing: **downscale first**. A canvas
  `drawImage` downscale is a low-pass filter — it smooths JPEG noise and moiré
  and sharpens the code's edges. Measured sweet spots: **1200px for a photo,
  1000px for a video frame**, roughly 5px per module.
- The full ladder: zbar at [1200, 1000, 800, 600, 400], then jsQR with
  `binInvert(160)` / `invert` / `binarize(160)` for the clean-screenshot cases
  where jsQR is actually better.

**Decision**: port that engine. Two dependencies, `@undecaf/zbar-wasm` (239 KB
of wasm) and `jsqr` (257 KB), both **lazy**: nothing loads until a scanner opens.

**One deliberate divergence from Expo**: it loads zbar from a CDN with a local
fallback, because Metro cannot import a module using `import.meta`. Vite can, and
this repo does not fetch code from third parties at runtime — the same rule that
made the launch animations local assets. **Bundled from our own origin, never a
CDN.**

**Carried gotchas from the same document**, each of which is a bug someone
already found: an iPhone Safari file input must be mounted in the DOM to work; a
`<video>` element intercepts touch events; `getUserMedia` requires HTTPS, so it
is unavailable on plain-http origins and that is a normal state to explain, not
an error to log.

---

## D46 — The share card carries an identicon because a doctored card cannot

Ported from `share-card.ts` (330 lines), under the founder's existing rules for
this surface: the identicon at the centre is an **anti-forgery mark** — a card
someone edited to swap the address carries artwork that no longer derives from
it — the code is square, and the composition is fixed. The address appears in
readable text as well as in the code, so a person can check it without a scanner.

---

## D47 — Formats are the product's own presets, never the platform's

Ported from `locale-format.ts` (331 lines). The rule it exists to enforce:
**explicit presets, not `Intl`**. A wallet that renders a number differently
depending on the browser's idea of a locale is a wallet where the same balance
reads two ways on two machines, and where a person cannot tell a thousands
separator from a decimal point in a language they are guessing at. The presets
are chosen by the person and are the same everywhere.

This includes money, which is why it is not cosmetic.

---

## D48 — Preferences have no core, and none is invented

Theme, language, number/date/time format and avatar style are **shell state**.
There is no Rust machine for them, and adding one would be a machine holding
four enums and no rule. They persist beside the other small preferences and are
read where they are used.

`display_currency` is the exception that proves it: currency HAS a machine,
because choosing a currency has a rule behind it (a rate must exist, and an
unpriceable currency must refuse). It is already wired and is not touched here.

The web today can only READ the theme (`isDarkTheme()` resolves a pinned
`data-theme`, then the OS). This feature adds the choosing and the persistence;
the reading stays exactly as it is, so nothing that already asks gets a second
answer.

---

## D49 — Erase deletes a NAMESPACE, and names its exceptions

Carried verbatim from `erase-device.ts`, including the reasoning that produced
it — the Expo module was rewritten precisely because the first version was a
hand-maintained list of eleven keys:

> It never covered contacts, contact groups, browser history, the `vela.perm.*`
> dApp grants, the receive-acknowledged flags, the balance/rate/token-metadata
> caches, or a single preference key — and it drifted out of date silently,
> because nothing about a delete-list fails when the app grows a key. **A
> delete-list erase is wrong by default and only accidentally right.**

**Decision**: enumerate what is actually stored and delete everything under the
`vela.` namespace, with the exceptions written down in
`contracts/erase-scope.md` rather than implied. The contract is the test.

**And the words matter**: erasing this browser loses LOCAL DATA. The wallet is
the passkey and survives. Copy that says or implies otherwise would frighten
someone out of an action that is safe, or reassure someone out of one that is not.

---

## D50 — An import never overwrites; the existing entry wins

Ported from `contact-io.ts` (341) and `saved-contact.ts` (22), keeping the
founder's rule: on a collision the **existing entry wins** and the import
reports what it skipped. The opposite default — newest wins — silently replaces
a name someone curated with one from a file they may not have written.

A malformed or hostile file is refused **before** anything is written: a partial
import is worse than none, because nobody can tell which half landed.

---

## D51 — Sweep and add-token need no drawing

Both surfaces exist and are reachable:

- `flows/model.ts` carries the sweep picker's multi-select (`multi_select_mode`,
  `multi_selected_ids`, and the scan button that appears beside the picker in
  sweep and not in single), and the core carries the same fields with 91 send
  tests behind them. `live-send.ts` uses NONE of them today.
- `AddTokenModel` / `AddTokenTab` / `AddTokenResult` are drawn, the fixture
  builds them, and both `FlowsMobile` and `FlowsPanel` already route
  `go('add-token')`. `manage_tokens` has an executor, a session, types and 22
  Rust tests, and is constructed by nothing but its own module.

**Decision**: wire, do not draw. If a surface turns out to be missing a state,
record it as a Penpot debt the way 024 did for the contacts form rather than
inventing one here.

---

## D52 — What this feature does NOT do

- **`browser_history` and `dapp_session`** stay unwired (spec 022; 027 D43).
- **The TypeScript twins Rust owns** — `clear-signing.ts`,
  `approval-guard-editor.ts`, `local-descriptors.ts`, `siwe.ts` — are not
  ported, for the reason 026 gave: a rule that looks like the source of truth
  while something else runs is worse than no rule.
- **WalletPair / webview / native transports, App Group sync, `deployer-api.ts`**
  — no counterpart on web.
- **027's SC-304** (an approve that never completes) is a defect, tracked in
  027's ledger, and a **precondition** of shipping this — not a task in it.
