# qr-lab — what can replace the web scanner's npm decoders

The web scanner (`app-web/vela-wallet/src/lib/services/qr-decode.ts`) reads QR
codes with two npm packages: `@undecaf/zbar-wasm` (a prebuilt LGPL-2.1+ C → wasm
binary) and `jsqr` (unmaintained since 2021). This lab measures, before any
production change, whether a Rust decoder built from this repository can do
their job: recognition, false reads, speed, download size.

It is **not** part of the vela-core workspace: own `[workspace]`, own
`Cargo.lock`, and nothing here is shipped.

## Run

```sh
./run.sh                    # corpus (once) → wasm shims → sizes → decode → report
./run.sh --only moire       # one category
./run.sh --limit 2          # a smoke run, 2 samples per category
REGEN=1 ./run.sh            # regenerate the corpus
```

Results: `out/report.md` (the tables), `out/results.json` (every sample,
every decoder), `out/preview/*.png` (the first few images of each category).
`out/` is ~850 MB and ignored.

Needs: the repo's Rust toolchain (wasm32 target), Node 22, `wasm-opt` (on PATH,
`$WASM_OPT`, or wasm-pack's cache), and `bun install` in `bench/` — the
baseline decoders (`@undecaf/zbar-wasm` 0.11.0, `jsqr` 1.4.0, the versions the
web app shipped until vela-qr) are the lab's own dependencies now, never the
app's. Their "shipped" sizes come from a web build that still carried them;
without one, `bench/sizes.mjs` reports the Rust shims only.

## Layout

| path | what |
|---|---|
| `corpus/` | `qr-corpus`: generates the corpus from a fixed seed |
| `lab-abi/` | `export_decoder!`: the one C ABI every shim exposes (`qr_alloc`, `qr_decode`, `qr_result_ptr`, `qr_free`) — no wasm-bindgen |
| `shim-*/` | the contestants as wasm: `rqrr`, `rxing` (as `detect_in_luma`), `rxing-cpp` / `rxing-java` (its two QR readers alone), `vela` (`rust/vela-qr`'s `decode`), `vela-once` (vela-qr's single pass, no strategy) |
| `apple/` | `main.swift`: CoreImage's `CIDetector` (exactly iOS's photo path), the same with a doubling retry for small pictures, and Vision — `swiftc -O apple/main.swift -o target/apple-qr && target/apple-qr`, then `node bench/merge-external.mjs out/apple-results.json` |
| `probe/` | `qr-probe <id> [width] [row]`: vela-qr's stages on one image, side by side with rxing's binarizer and finder patterns |
| `bench/decoders.mjs` | the contestants; zbar and jsqr are the exact packages the app ships |
| `bench/run.mjs` | decodes every sample; also replays `decodeImage()` step for step as "web today" |
| `bench/sizes.mjs` | download size: the app's shipped chunks vs the wasm-opt'd shims |
| `bench/report.mjs` | `out/report.md` |

A new contestant is one more `shim-*` crate (`lab_abi::export_decoder!(decode)`);
`run.sh` and the bench pick it up. `./quick.sh <name> [--only <category>]`
rebuilds and reruns one shim alone, merging into the existing results.

## The corpus

572 images: 2 untouched real receive cards (the fixture every platform's suite
shares) + 19 categories × 30. Payloads are what Vela scans — addresses,
EIP-681 native/token links, WalletConnect URIs, `FIDO:/` codes, long links —
encoded by `qrcode` 0.14 at the levels vela-core uses (M on screen, H on the
share card).

clean · big-modules (screenshot, 16–32 px/module) · tiny-modules (1.2–2.2
px/module) · no-quiet-zone · inverted · low-contrast · blur · noise · jpeg
(q5–30) · rotation · perspective · uneven-light (gradient, vignette, glare) ·
moire (a screen photographed: pixel grid, subpixel stripes, aliasing) ·
share-card-logo (EC H, centre plate, recompressed) · clutter-large (3 MP / 12
MP photos, small code, busy background) · phone-photo (everything at once) ·
camera-frame (720p/1080p, motion blur) · real-fixture-jpeg (the real cards,
chat-app recompression) · real-fixture-photo (the real cards, photographed).

## What the numbers do and do not say

- **Synthetic.** Every degradation is modelled, not photographed. It is
  reproducible and broad, but a set of real phone photos should be added before
  a production decision (drop them in with their expected text).
- **Resizing** uses `image`'s Triangle filter as a stand-in for a canvas
  `drawImage` downscale. Browsers differ from it and from each other.
- **Speed** is V8 (Node), single thread — Chrome-like. iOS Safari's
  JavaScriptCore will differ in absolute terms.
- **rxing is patched**: it stamps every result with `chrono::Utc::now()`, which
  panics on `wasm32-unknown-unknown`, so unpatched it traps on every successful
  decode in a browser. `run.sh` vendors it into `out/vendor/rxing` with the
  stamp set to 0 (`[patch.crates-io]` in `Cargo.toml`).

## Findings (2026-11-14)

- **rxing does not run in a browser unpatched** (the timestamp above).
- **rxing's strength is its zxing-cpp detector**: alone it reads 90.9% of
  what all of rxing reads 92.1% of. Its size is not the detector — 164 KB of
  the module is `encoding_rs` charset tables.
- **That detector, ported** (`rust/vela-qr`), matches it sample for sample at
  a sixth of the size; with a decode strategy on top (inversion, halving,
  doubling small images) it reads 97.2% of still images and 97.4% of camera
  frames, with no wrong reads, at ~4 ms a frame.
- **Two defects in today's web pipeline** (`qr-decode.ts`): images ≤ 400 px
  wide never reach zbar (the ladder filters every size out), which is 72 of
  its 90 misses; and zbar runs with every symbology on while the code takes
  `found[0]` unfiltered — it read Interleaved 2-of-5 out of photo clutter
  twice.
- **Apple's readers on macOS** (`apple/`, 2026-11-14): CIDetector as iOS calls
  it reads 94.9% of still images (96.0% of frames), Vision 96.3%, no wrong
  reads; CIDetector misses mostly tiny modules (19/30) because it never
  enlarges. Retrying a picture under 640 px at double size brings it to 96.5%
  — two images short of vela-qr, with no new code in the app's graph. Native
  times, GPU where used: not comparable with the wasm timings.
