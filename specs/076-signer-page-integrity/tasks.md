# Tasks — 076 the signer page is checked before it is opened

**Phase A is done; everything else is not started.** The spec is the design.

Order note: plan.md says "P0 first and alone". Phase A was taken first anyway,
and deliberately: the DECISION depends on no probe outcome — the probes decide
whether the verification MECHANISM (FR-004) can keep the browser's fingerprint,
which is phase C's problem. Nothing in A is built on a guess from the spec.

## P0 — probes (do these first, and alone)

**Desktop Chromium first, as a check on the probe rather than on the platform**
(2026-09-23, headless Chrome for Testing 151 against a counting server):

```
max-age=300  → OK status=200 bytes=12603 secure/subtle=true/true sha256=86810537…
                server asked 1× for the document (plus /favicon.ico)
no-store     → THREW TypeError: Failed to fetch
```

So the mechanism itself is sound: `only-if-cached` returns the navigation's own
bytes, at exactly the page's length, with no second request for the document;
`crypto.subtle` is there because http on loopback IS a secure context; and
`no-store` throws, which is the fail-closed case FR-006 needs. It also caught a
flaw in the probe: a browser asks for `/favicon.ico` on its own, so "asked
once" has to be counted per PATH, not in total.

**This does not answer T001.** Android's WebView and WKWebView are the engines
076 will actually run in, and they are what the probes below measure.

- [x] **T001 P1 · Android WebView — YES.** Real device, M2012K11AC / Android 13:
      the body comes back at exactly the page's length (12603), the digest
      matches headless Chromium's for the same bytes, the DOCUMENT is asked for
      once, `crypto.subtle` is available (loopback http is a secure context),
      and `no-store` throws — fail-closed, as FR-006 needs.
- [ ] T001 P1 · **WKWebView — being measured.** The one that can still cost
      FR-004 its position.
      *Probes:
      `app-android/…/androidTest/…/SignerPageProbeTest.kt` and
      `app-ios/VelaWallet/VelaWalletTests/SignerPageProbeTests.swift` — each runs
      a counting server in-process, so "no second request" is the server's word
      and not an API's.*
- [ ] T002 P2 — what escapes a blocked WebView: fetch / XHR / sendBeacon / WebSocket / WebRTC
- [ ] T003 P3 — a logging server compares the probe's request with a real navigation's (UA, headers, JA4, H2)
- [ ] T004 Write up all three, with numbers, into spec.md's open questions

## A — core  ·  `vela_core::clear_signer::integrity`, 13 tests

- [x] **T010** `BUILD_ALLOWED`, a plain slice so it can SHRINK between builds.
      「一直累加」is the wrong invariant: a version found compromised must be
      removable, or every client already shipped accepts a replay of it for
      ever. **It is empty**, because no page is published under `/b/<sha256>/`
      yet — and a test asserts that, so filling it is a deliberate act with a
      failing test attached rather than a quiet edit.
- [x] **T011** Per-device allow-list and deny-list, with **deny outranking both**
      — this build's set and the person's own trust alike. Tested in each
      ordering, including on a custom address with the check turned off.
- [x] **T012** `content_addressed_url` → `https://sign.getvela.app/b/<sha256>/sign.html`,
      `None` for a custom address (opened where the person said it is; their
      bytes are pinned by the hash they trusted, which is the property that
      matters). `is_official` is host-only and refuses the lookalikes —
      `sign.getvela.app.evil.test`, `…@evil.test`, `?x=sign.getvela.app`.
- [x] **T013** `Verdict`: `Open` · `OpenUnverified` · `Denied` · `Refused` ·
      `AskToTrust` · `CouldNotCheck`. `OpenUnverified` is a separate variant
      rather than `Open` with a flag, so a shell cannot forget to draw the
      standing warning (FR-009). The official address has no off switch: that
      input is deliberately not read on that branch.

**Not wired into the launch path, on purpose.** With `BUILD_ALLOWED` empty,
enforcing this would open nothing and brick a Clear Signer that works today.
Phase B publishes the page and fills the set; phase C does the wiring.

## B — the page  ·  `samples/build-single.mjs`, `samples/single-file-test.mjs`

- [x] **T020** One file: one style block, one script block, 19 sources, 312KB,
      no subresources. Reproducible on purpose — `--check` builds twice and
      insists on the same bytes, because a hash nobody else can reproduce
      verifies nothing. It does not replace `sign.html`: the folder stays
      hand-written and `--check` keeps the artefact from drifting.
- [x] **T021** `default-src 'none'` inside the hashed bytes, and a real-browser
      test: 10/10. The scripts RUN under their own CSP hash, the page asks the
      network for nothing but its own document, and `fetch` / a remote image /
      `sendBeacon` / a `WebSocket` from inside it all fail to reach a listening
      server.
- [ ] **T022** Publish under `/b/<sha256>/sign.html`, and the rule that a
      published path never goes away. **Not done — this is deployment**, and
      `sign.getvela.app` is not mine to change.

Two things the build taught, both measured rather than reasoned:

- **`String.replace` with a replacement STRING ate the script.** `$&`, `` $` ``
  and `$'` are substitutions there, and JavaScript source is full of `$`, so the
  bytes that landed in the file were not the bytes that were hashed — the page
  loaded with its own code refused. Every insertion goes through a replacer
  function now.
- **`navigator.sendBeacon` returns `true` for "queued", not "sent".** It reports
  success on a request the CSP then refuses, so the test asks the SERVER what
  arrived instead of believing the API. Trusting the return value would have
  reported a leak that is not there.

## C — the verifier

The tasks below were written for FR-004b (the hidden WebView). FR-004 is a
plain request now, so they do not apply; what replaced them is measured.

- [x] **T030 · desktop, end to end over a LAN address.** index →
      `choose_version` → fetch by hash → `hash_page` → `decide`, over a real
      socket. *`dist/` served at 192.168.50.51:8920; `check(base) → Open`, the
      served bytes hashing to exactly their own path name.* Driven before
      anything was published, which was the point.
- [x] **T031 · content addressing belongs to the DEPLOYMENT.** It was limited
      to the official hostname, so a self-hoster's bytes were never fetched by
      hash — and the path everyone relies on could not be exercised on any
      machine but the production one.
- [x] **T032 · the allow-set and the off switch are separate.**
      `BUILD_ALLOWED` lists the first published hash; `ENFORCE` is false.
      Tying them together would make the commit that lists a hash silently
      start refusing every page not yet published. A test is a tripwire on
      `ENFORCE`.
- [x] **T033 · fail closed**, and two silences told apart. Blocking the only
      usable version used to answer "could not be checked" — which reads as a
      network fault. It now names the block list. *Found by driving it: no unit
      test would have noticed, because both answers refuse.*
- [ ] T034 Android and iOS: the same three steps. The core and the desktop show
      the shape; neither phone has it yet.

~~T030–T033 (FR-004b): hidden WebView, `WKContentRuleList` /
`shouldInterceptRequest`, `only-if-cached`.~~ Not being built — see FR-004b and
probe P2.

## D — when it runs
- [ ] T040 Background, unpredictable, result cached; never adjacent to an open
- [ ] T041 The re-check afterwards, and the warning naming expected vs actual (FR-011)

## E — Settings
- [ ] T050 The version in force: hash, when checked, and "block this version"
- [ ] T051 Paste a hash to block; record when and why; per-device, and say so
- [ ] T052 Trust a custom page's hash (FR-009), and the off switch confined to custom addresses
- [ ] T053 The standing warning while verification is off
