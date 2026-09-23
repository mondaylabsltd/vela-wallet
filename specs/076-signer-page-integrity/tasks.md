# Tasks — 076 the signer page is checked before it is opened

**Nothing here is started.** The spec is the design; this is what implementing
it would be.

## P0 — probes (do these first, and alone)
- [ ] T001 P1 — `only-if-cached` on WKWebView and Android WebView: does it return the navigation's body?
- [ ] T002 P2 — what escapes a blocked WebView: fetch / XHR / sendBeacon / WebSocket / WebRTC
- [ ] T003 P3 — a logging server compares the probe's request with a real navigation's (UA, headers, JA4, H2)
- [ ] T004 Write up all three, with numbers, into spec.md's open questions

## A — core
- [ ] T010 The allow-set compiled in; the shape that lets it SHRINK between builds
- [ ] T011 Per-device allow-list and deny-list; **deny outranks both**, with tests for each ordering
- [ ] T012 The content-addressed URL builder, and the refusal when a hash is unknown
- [ ] T013 The verdict type the shells render: allowed / unknown-hash / denied / could-not-check

## B — the page
- [ ] T020 One file: inline CSS and JS, no subresources, and a REPRODUCIBLE build that produces it
- [ ] T021 `default-src 'none'` inside the hashed bytes; a test that a matching page cannot reach the network
- [ ] T022 Publish under `/b/<sha256>/sign.html`, and the rule that a published path never goes away

## C — the verifier (per shell)
- [ ] T030 Hidden WebView, real navigation, UA set to the browser's
- [ ] T031 Block everything but the document (iOS `WKContentRuleList`; Android `shouldInterceptRequest`)
- [ ] T032 Digest from `only-if-cached` inside the page's context
- [ ] T033 Fail closed on every failure, including the cache read (SC-002)

## D — when it runs
- [ ] T040 Background, unpredictable, result cached; never adjacent to an open
- [ ] T041 The re-check afterwards, and the warning naming expected vs actual (FR-011)

## E — Settings
- [ ] T050 The version in force: hash, when checked, and "block this version"
- [ ] T051 Paste a hash to block; record when and why; per-device, and say so
- [ ] T052 Trust a custom page's hash (FR-009), and the off switch confined to custom addresses
- [ ] T053 The standing warning while verification is off
