# Implementation Plan: 076 — the signer page is checked before it is opened

**Branch**: `076-signer-page-integrity` (on `075-clear-signer-channel`) | **Spec**: [spec.md](spec.md)
**Status**: not started. Phase 0 is a set of experiments, and **they can kill the
design** — see spec.md's open questions.

## Phase 0 — measure, before writing anything that depends on a guess

Three probes, each a throwaway app, each answering one question with a number
rather than an opinion. If probe 1 fails on either platform, FR-004 collapses
and the "keep the browser's fingerprint" position has to be re-argued from
scratch; everything after it waits on that answer.

| # | Probe | Answers |
|---|---|---|
| P1 | A hidden `WKWebView` / Android `WebView` navigates to a page; injected JS does `fetch(location.href, {cache:'only-if-cached'})` and prints the byte length | Can the bytes be read without a second request? (open question 1) |
| P2 | A page in that WebView tries `fetch`, XHR, `sendBeacon`, `WebSocket` and a WebRTC `RTCPeerConnection` against a logging server, with the rule list / interceptor on | What actually gets out? (open question 2) |
| P3 | The same page loaded (a) by the real browser, (b) by the probe, against a server that logs UA, every header, JA4 and the H2 fingerprint | Are they distinguishable? (open questions 3, SC-003) |

**A probe that cannot be run is a probe that failed.** Nothing below starts on
an assumption from this document.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| A | Core: the allow-set type, the deny-list, `allowed(hash) → verdict` with deny winning, the content-addressed URL builder, the stored shapes | core tests: deny beats allow beats build-set |
| B | The page becomes ONE file with `default-src 'none'`, published under `/b/<sha256>/` | the page's existing suites, plus a CSP test that a matching page cannot reach the network |
| C | The verifier: hidden WebView, navigation, blocked subresources, digest from the cache, fail-closed | per shell; SC-002 and SC-004 on a real server |
| D | The schedule: background, unpredictable, cached result; the re-check of FR-011 | a test that no verification request is issued within N seconds of an open |
| E | Settings: the version in force, block it, trust a custom one, the standing warning | per shell |

Order: P0 first and alone. Then A and B in parallel; C after A and P0; D after C;
E last.

## Risks

- **Probe 1 fails.** Then the bytes can only be read by intercepting, which
  costs the fingerprint — and the honest fallback is "catch attack A, say so
  plainly, lean on the Service Worker for B". That is still worth shipping; it
  just must not be described as more.
- **A single-file page fights the page's own maintainability.** The build step
  that inlines it becomes load-bearing: if it is not reproducible, the hash is
  not reproducible, and nobody can independently verify a published version.
  Reproducibility of that build is part of phase B, not an afterthought.
- **The off switch gets used.** FR-009 confines it to custom addresses and keeps
  a standing warning. If it ever reaches the official page, the whole spec is
  decoration.
- **Publishing discipline.** Every allowed hash must stay reachable at its path
  for ever. A version garbage-collected from the bucket bricks every client that
  allows only it.
