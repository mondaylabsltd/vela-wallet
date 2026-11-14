# vela-qr

Reads QR codes (model 2) out of 8-bit luminance images. No dependencies, no
`unsafe`, ~28 KB of wasm after brotli (`wasm-opt -Oz`; `-Os` is ~31 KB).

```rust
if let Some(code) = vela_qr::read(&luma, width, height) {
    match code.text() {
        Some(text) => { /* ASCII, UTF-8, Latin-1, UTF-16/32, CP437: done */ }
        None => { /* a segment needs the platform's decoder — see below */ }
    }
}
let all: Vec<vela_qr::Code> = vela_qr::read_all(&luma, width, height);
```

It answers only *what a symbol carries*; what it means (an address, an
EIP-681 link, a WalletConnect URI) is the caller's to judge.

## Character sets: everything, without tables

Every mode the standard defines is read — numeric, alphanumeric, byte,
Kanji, Hanzi, ECI, FNC1, structured append. A `Code` is a list of segments,
each its raw bytes plus a `Charset`:

- **Read by the crate itself** (no tables needed): ASCII, ISO-8859-1, UTF-8,
  UTF-16, UTF-32, CP437. `Code::text()` returns the text.
- **Named for the platform**: `Charset::Table(label)` with the WHATWG label —
  `shift_jis` (also what Kanji mode becomes), `gb18030` (also Hanzi mode),
  `gbk`, `big5`, `euc-kr`, `iso-8859-2…16`, `windows-1250…1256`. A browser's
  `TextDecoder(label, { fatal: true })` decodes it; iOS, Android and the
  desktop have their own decoders for the same names. This is what keeps the
  crate small: the CJK tables are most of what makes other readers 150 KB+.
- **Undeclared bytes** (no ECI, as older generators write GBK or Shift_JIS):
  `Segment::guessed` is set and `Segment::candidates()` lists what to try in
  order — UTF-8 if valid, GB18030 / Shift_JIS by their byte grammar, and
  always ISO-8859-1 last, which decodes anything, so nothing is left
  unreadable. Inherent ambiguity: a short GBK string can also be valid UTF-8
  (元 = D4 AA = "Ԫ"); valid UTF-8 wins, as in ZXing, because the reverse
  (UTF-8 "é" = a GB2312 hanzi) is far more common.
- **Binary** (ECI 899) and unknown ECIs: the bytes, `text()` is `None`.
- **Structured append**: each symbol is read and `Code::part` says it is
  part *i* of *n* — never mistaken for the whole message. Assembling the
  parts is not implemented.

## Where it comes from

The detector, sampler and decoder are a port of zxing-cpp's QR reader (read
through rxing's Rust port), reduced to model 2 and to plain Rust types. That
code is Apache-2.0, so this crate is too — see `NOTICE` and `LICENSE-APACHE`.

On top of the port, `read` adds a strategy the libraries leave to callers:
each size is tried dark-on-light and light-on-dark; the image is halved (2×2
mean) until it is too small, which reads modules wider than the binarizer's
40 px window and smooths noise; images under 640 px are finally tried at
double size, which reads modules under 2 px.

## How it was measured

`rust/qr-lab` (see its README): 572 images in 20 categories, against the npm
decoders the web app ships and the Rust alternatives. At the time of writing:

| | still image | camera frame (1000 px) | wrong reads | frame p50 | brotli |
|---|---:|---:|---:|---:|---:|
| web today (zbar-wasm → jsqr) | 84.3% | — | 0 | — | 165 KB |
| zbar-wasm alone | 89.0% | 86.4% | 2 | 45 ms | 140 KB |
| rxing (QR, patched for wasm) | 92.1% | 77.6% | 0 | 8.1 ms | 171 KB |
| **vela-qr** | **97.2%** | **97.4%** | **0** | **4.3 ms** | **28 KB** |

(Brotli q11 of the whole wasm module; vela-qr with `wasm-opt -Oz`.)

## Tests

```sh
cargo test                                   # units, round trips, robustness
VELA_QR_FUZZ=10 cargo test --release --test robustness
cargo +nightly fuzz run grid                 # coverage-guided (fuzz/)
cargo +nightly fuzz run image
cargo +nightly fuzz run text                 # charset guessing and decoders
```

The `probe` feature exposes diagnostics for the lab and the fuzzers; it is not
for shipping.

## Standalone

Not a member of `rust/Cargo.toml`: it is neither compiled into nor
fingerprinted with the core the native shells ship. Platforms opt in:

- **web** — `wasm/` (a plain C ABI, no wasm-bindgen), built by
  `node rust/scripts/build-web-qr.mjs` into the committed `rust/pkg-web-qr/`;
  the scanner (`app-web/vela-wallet/src/lib/services/qr-decode.ts`) fetches it
  when it opens. CI: `build-web-qr.mjs --check`, and this crate's tests.
- **desktop** — a path dependency (`app-desktop/vela-wallet/src/executor/qr.rs`),
  table charsets through `encoding_rs`.
- iOS and Android keep their platform readers for now (VisionKit/CoreImage,
  ZXing).
