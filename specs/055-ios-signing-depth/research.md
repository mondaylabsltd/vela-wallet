# Research — 055

## D1 · One QR decoder: CoreImage's `CIDetector`

**Revised during phase 0.** The first choice was `VNDetectBarcodesRequest`, for
the right reason: it reads a sample buffer and a still image alike, so the
camera and the photo library share one decoder — the fork Android's `QrDecoder`
exists to prevent, and two decoders eventually disagree about what a code says.

It does read both **on a device**. In the simulator it finds nothing at all: a
code this app itself rendered — large, square, four-module quiet zone — came
back from Vision with zero observations and was read correctly by `CIDetector`
in the same test, with no error thrown by either. A decoder that cannot be
exercised in the hermetic suite is a decoder nobody can prove works.

`CIDetector(ofType: CIDetectorTypeQRCode)` keeps the property that mattered: it
reads a `CIImage`, and a `CIImage` comes from a camera's pixel buffer and from a
photo alike. One detector is held for the life of the process — building one
compiles a pipeline, and the scanner asks thirty times a second.

`AVCaptureMetadataOutput` remains rejected: it cannot read a photo.
`VisionKit.DataScannerViewController` brings its own UI and is camera-only.

The round trip is a test: `QrCode.modules` renders what the receive screen
renders, and the decoder reads it back — square and rotated a quarter turn.

## D2 · The simulation is the SHELL's half only

The core already judges (`token_trust::judge_delta`, asymmetric by design:
an outflow renders whenever metadata resolved because the real token emits its
own log; an inflow renders a number only for a token already trusted). What iOS
needs is the desktop's `executor/sim.rs` and Android's `SimDeltas.kt`, ported:

- `payload(from:calls:)` — one `blockStateCalls` entry carrying every call,
  `validation: false`, `traceTransfers: true`;
- `logsOf(_:)` — the logs of every SUCCEEDED call in every simulated block,
  `nil` when the node answered an error;
- `deriveDeltas(logs:user:)` — net `Transfer` logs touching the wallet, per
  token, in first-seen order, zero totals dropped; the `0xeeee…` sender is the
  native coin.

**Swift has no `BigInteger`.** The values are 32-byte words, so a small signed
decimal-string helper (`SignedDigits`) does the netting — the same shape
`TokenReads` already uses for unsigned multiply and add.

## D3 · What "could not simulate" means

Three outcomes, and the sheet must not blur them:

| | what happened | what the sheet says |
|---|---|---|
| deltas | the node ran it and logs moved | the balance block |
| empty | the node ran it and nothing moved | "checked, nothing moves" |
| unavailable | no `eth_simulateV1`, or the node errored | the danger-coloured "could not check" block |

The third is desktop 037's rule and it is the one that matters: a wallet that
says nothing when it could not look teaches people that silence means safe.

## D4 · SIWE is CHECKED, not re-parsed

`clear_signing` already parses the message; the shell's job is to hand it the
two facts only the shell knows — the origin that asked and the chain it is on —
and render the core's verdict. Android 044's bug was a placeholder that never
interpolated; the test here asserts on the rendered sentence, not on the key.

## D5 · The camera's own rules

- `AVCaptureSession` is configured and started on a serial queue —
  `startRunning` blocks.
- `AVCaptureVideoDataOutput` with `alwaysDiscardsLateVideoFrames`; the
  connection's `videoRotationAngle = 90` or every frame is sideways.
- A `reported` flag: the first hit stops the session and is delivered once, on
  the main queue. Without it a code is decoded thirty times a second.
- `PHPickerViewController` is cross-process and needs **no** photo permission —
  051's add-only album key stays the narrowest thing in the app.
- Permission has three refusals: `.denied` (offer Settings), `.restricted`, and
  no camera at all (the simulator) — which is a `DiscoverySession` that comes
  back empty, not an error.

## D6 · EIP-681, and its two refusals

The scanner's payload is an EIP-681 URI. Two rejections are load-bearing
(desktop 036):

1. a `/transfer` call's `value` parameter is **not** the token amount (`uint256`
   is), and reading it as one would send the wrong number;
2. any function that is not `transfer` is not a payment and must not be
   silently treated as one.

The core owns what a resolved scan means; the shell only tokenises.
