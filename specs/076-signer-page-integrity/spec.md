# Feature Specification: the signer page is checked before it is opened

**Feature Branch**: `076-signer-page-integrity` (on `075-clear-signer-channel`)
**Created**: 2026-09-23
**Status**: Phase A landed (`vela_core::clear_signer::integrity`, 13 tests) —
the decision, shared by every shell. Phases B–E and the P0 probes are not
started, and the decision is **not wired into the launch path** until the page
is published (see tasks.md).
**Input**: Owner, 2026-09-23, immediately after cutting the Clear Signer's
cross-device channels:

> 而且还能实现防范分发攻击，就是客户端打开这个外部清晰签名器页面前，先请求目标
> html 看看 sha256 是否匹配链上发布的记录列表，如果不匹配，就不打开。…一旦发现
> 有不对的，要在客户端明显的提示用户更换，疑似出现分发攻击

and, over the conversation that followed:

> 我觉得要简单一点，让攻击者无法定向识别
>
> 然后 sha256 直接编译进客户端，一直累加，必须是历史中的一个才认为可用
>
> 也要支持用户自己添加黑名单来拉黑某个版本
>
> sha 验证可以作为一个开关，默认必须验证 … 用户自己部署了一个版本

## Why this exists

The Clear Signer's whole claim is **you see what you sign**. It rests on the
page being the page it is supposed to be. Spec 075 cut every cross-device
channel for exactly this reason (075 spec.md, "What the owner cut"): a page on
somebody else's device is one this device never fetched, and what it never
fetched it cannot check.

So the remaining channel — the page in this device's own browser — is the only
one where the check is possible, and this spec is that check.

## Threat model, stated before anything else

Two attacks, and they are not the same difficulty. Every mechanism below is
judged against both, and **nothing here prevents the second**.

| | **A — a replaced build**: the server serves bad bytes to everyone | **B — a discriminating server**: bad bytes only to the real navigation, or only to one victim |
|---|---|---|
| Client fetches once, compares to a known list | ✅ catches it | ❌ |
| Fetching N times, random N | ✅ | ❌ — the server answers by request *type*, not by counting |
| Fetching in a shape the server cannot tell from a navigation, at a random time | ✅ | ⚠️ forces a guess; each wrong guess is a detection |
| Third-party observers fetching the same URL | ✅ (public evidence) | ❌ — they are neither the victim nor the navigation |
| **Service Worker pinning** | ✅ | ✅ **after first visit** — the browser stops asking the server at all |
| Post-hoc re-checks | ✅ | ❌ for that load, but the person learns they were served something else |

**A is the realistic attack** — a hijacked bucket, a bad CDN config, a poisoned
release pipeline — and it hits everyone at once. B is targeted and expensive.

**Therefore the copy must say "this page does not match the published list",
never "a distribution attack was prevented."** The first is true; the second is
a claim this design cannot make.

### What is ruled OUT, and why it matters

**The app must not serve the page itself.** Serving it from the wallet's own
loopback would make the verified bytes and the executed bytes identical, which
sounds like the whole problem solved. It is not: WebAuthn binds the keys to the
page's ORIGIN, and a page served from `http://127.0.0.1:<port>` belongs to
loopback, not to `sign.getvela.app`. A wallet that can serve the page can serve
a lying page and sign with the same key. That destroys the property the Clear
Signer exists for, so it is a hole, not a trade-off.

**A page that reports its own hash is worth nothing.** A tampered page reports
whatever it likes. Self-reported build ids may be shown as diagnostics, never as
a security signal, and never worded as one.

## Requirements

### FR-001 · The allow-set is compiled into the client
Each build ships the set of page hashes it accepts. `sha256` over the page's
bytes.

**It can shrink.** "一直累加" is the wrong invariant: a version found to be
compromised must be removable, or every client already shipped would accept a
replay of it for ever. The set is *this build's accepted set* — usually growing,
occasionally losing an entry.

No chain is read. The trust root is the app binary, which the person already
trusts completely, and it works offline. (A chain-published list can be added
later as a second source for out-of-band revocation; it is not in scope.)

### FR-002 · The page is requested by hash, and an index says which are there
The client opens a content-addressed URL:

```
https://sign.getvela.app/b/<sha256>/sign.html
```

It asks for a version it already knows. There is no "which version is current"
to get wrong, a mismatch is attributable and reproducible by anyone, and old
clients keep working because the server keeps every published version.

**Publishing at the root was considered and rejected** (owner, 2026-09-23, both
directions in one sitting). Serving only the current page at
`https://sign.getvela.app/` is simpler to operate, and it breaks the moment the
page is updated: every client whose allow-set lacks the new hash refuses to
open it. That would make an ordinary release look exactly like an attack, and a
warning spent on the routine case is not there on the real one. Content
addressing is what lets a new page ship without locking anyone out.

**The index** (「需要有一个索引不然的话,不知道端点支持哪些版」). A client
knows which hashes it trusts but not which the endpoint still keeps, so a
well-known path lists what is published.

Its one rule: **the index narrows the choice and never makes it.** The
candidates are this build's set and then the person's own trusted hashes, in
that order, and the first the endpoint still serves wins (`choose_version`). So
a lying index can only hide versions — a loud, fail-closed refusal — or list
versions this wallet does not trust, which are ignored. It cannot steer anyone
on to a particular version, and it cannot introduce one. That is why it needs
no authority of its own and is not signed.

It is an optimisation, not a dependency: the URL is derivable from the hash, so
a client that cannot fetch the index can still ask for its own preferred
version directly.

**The deployment is a directory, and it is in git** (owner, 2026-09-23):

```
app-web/trusted-signer/dist/
  index.json                 what is published
  b/<sha256>/sign.html       a version, for ever
```

Publishing is copying that directory to the signer host's root. Two properties
come free, and both are the reason:

- **"a published path never goes away" stops being a discipline.** Losing a
  version means deleting a committed file, in a commit, in review. The risk
  plan.md names — a version garbage-collected from a bucket bricking every
  client that allows only it — is no longer a thing anyone can do by accident.
- **the index cannot lie about what is there**, because it is generated from
  the directory rather than maintained beside it. `--check` also re-hashes
  every published path and refuses one whose bytes do not hash to its own name.

The build APPENDS. It never overwrites and never removes.

**The builder is zero-dependency too**, like the page: `node:crypto`,
`node:fs`, `node:path`, `node:url` and nothing else — no npm, no lockfile,
nothing to audit. Bun is the runtime, and Node runs it unchanged. **They
produce the same bytes and the same hash** (measured: Bun 1.4.2 and Node 22
both give 312257 bytes, sha256 810db7c5…). A build whose output depended on
which runtime ran it would be one nobody else could reproduce, and an
unreproducible hash verifies nothing.

### FR-003 · One file, and no network of its own
The page is a SINGLE file: HTML, CSS and JS in one document, no subresources.
Two things follow, and the second is the bigger prize:

- one hash covers every executable byte, with no manifest to maintain;
- its CSP (`default-src 'none'`, and that CSP is *inside the hashed bytes*) means
  a page that matches the hash **structurally cannot exfiltrate what it sees** —
  the transaction, the address, the amount. It can display and it can answer
  over the channel it was opened on. Nothing else.

This is a hard constraint on the page, not an optimisation.

**Built and measured, 2026-09-23** (`samples/build-single.mjs`, 312KB: one
style block, one script block, 19 sources). In a real browser its scripts run
under their own CSP hash, the page asks the network for nothing but its own
document, and `fetch`, a remote image, `sendBeacon` and a `WebSocket` from
inside it all fail to reach a listening server —
`samples/single-file-test.mjs`, 10/10.

What the constraint COSTS, found by building it:

- **the favicon** — decoration, and a subresource;
- **remote chain and token logos** from `ethereum-data.getvela.app`. The page
  already falls back to a drawn letter when an image fails, and the identicon —
  the actual anti-poisoning signal — is computed locally, so this is a cosmetic
  loss, not a safety one;
- **the `memberProof` ceremony's registry fetch**. Transaction signing never
  touches the network, but that one ceremony fetches a challenge from
  `p256-index-v2.getvela.app`, and a `connect-src` for it would hand every page
  matching the hash a way out. The fix is for the WALLET to fetch the challenge
  and pass it in over the channel: the page already refuses any challenge it
  cannot recompute itself, so nothing is weakened by where it arrives from.
  **Until that lands, memberProof does not work in the single-file build**, and
  it is the one thing FR-003 breaks rather than merely dims.

The CSP omits `frame-ancestors` on purpose: a `<meta>` CSP cannot carry it, so
writing it there would be a claim the file cannot keep. It belongs on the
response header where the page is served — which is FR-002's publishing
discipline, not the page's.

### FR-004 · The check is a plain HTTPS request

**Owner, 2026-09-23: 「我们先用 http 请求 html 来判断吧,后续在用更加复杂的判断
… 先跑通」.** The app asks for the content-addressed URL over its own HTTP
stack, hashes the bytes, and rules with [`decide`]. No WebView, no navigation,
no cache archaeology.

This keeps exactly the property this spec claims. It catches **attack A** — a
replaced build, served to everyone: a hijacked bucket, a bad CDN config, a
poisoned release. That is the realistic attack and the one that hits everyone
at once. It does not catch **attack B**, a server that serves bad bytes only to
the real navigation or only to one victim — and neither did the elaborate
version, which merely forced B to guess (see the table above; only FR-008's
Service Worker answers B).

**And the probes made the simple version look better, not merely cheaper.**
Verifying by navigation means LOADING the attacker's page, which means RUNNING
the attacker's code, and P2 measured what that code can still do inside a
WebView whose subresources are all intercepted: a WebSocket handshake goes
straight out (`shouldInterceptRequest` never sees one) and WebRTC leaves over
UDP (a content blocker governs HTTP; STUN is not HTTP). Closing those needs the
JS environment stripped of `WebSocket` and `RTCPeerConnection` — which a page
can notice. A plain request never runs a line of the page, so none of that
exists.

What the simple version gives up is that the bytes HASHED are not, literally,
the bytes EXECUTED: they are two requests. The content-addressed URL (FR-002)
is what narrows that gap — the path names the hash, so serving something else
there is detectable by anyone, reproducible by anyone, and the page is opened
at the same URL that was checked — and FR-008 closes it afterwards.

### FR-004b · The hidden-WebView check — NOT being built, and why it is written down
The original design, kept because the day it is wanted again the measurements
are already here. It would not be a `fetch()` from the app's HTTP stack.

- `WKWebView` (iOS) is Safari's WebKit; Android's WebView is Chromium. Loading
  by NAVIGATION gives the same TLS fingerprint, the same HTTP/2 fingerprint and
  the same `Sec-Fetch-Mode: navigate` / `Sec-Fetch-Dest: document` /
  `Upgrade-Insecure-Requests: 1` as the real thing.
- The UA giveaways are set away: Android WebView's `; wv)`
  (`WebSettings.userAgentString`), and WKWebView's missing `Safari/` token
  (`applicationNameForUserAgent`).

**The bytes are read from inside the page's own JS context**, not from any
interception layer:

```js
const r = await fetch(location.href, { cache: 'only-if-cached', mode: 'same-origin' });
const hash = await crypto.subtle.digest('SHA-256', await r.arrayBuffer());
```

`only-if-cached` reads the HTTP cache and issues no second request, so what is
hashed is what the navigation actually received.

### FR-005 · The page cannot speak while it is being checked — **moot under FR-004**

With a plain request nothing of the page runs, so there is nothing to silence.
This section applies only to FR-004b, and P2 measured that it does not hold as
written: see open question 2. It is kept for the same reason FR-004b is.
It is the attacker's page; loading it runs its code. A page that can detect a
verification context (hidden, no interaction, odd viewport) could simply tell
its server.

So everything but the document itself is blocked:

- iOS: a `WKContentRuleList` blocking `.*`, with an exception anchored to the
  exact document URL (a query-string variant does not match, so `?leak=…` is
  refused);
- Android: `shouldInterceptRequest` returns `null` for the MAIN frame — letting
  the engine fetch it, which is what preserves the fingerprint — and an empty
  response for everything else.

### FR-006 · Fail closed
**A check that cannot complete is a check that failed.** No page opens —
whether the request timed out, the TLS handshake failed, the status was not
200, or the body could not be read. `Verdict::CouldNotCheck`, and nothing
opens on it.

(Under FR-004b this had a sharper edge: a server sending `Cache-Control:
no-store` made `only-if-cached` throw, which is exactly what a server defeating
that check would do. Measured on both engines — Android WebView and headless
Chromium both threw `TypeError: Failed to fetch`.)

### FR-007 · The check is decoupled in TIME from signing
It runs in the background at an unpredictable moment — app launch, once a day,
after a random delay — and the result is cached. Never in the seconds before a
page opens.

This removes the attacker's best signal: "a verification request just arrived,
so the next navigation is the target". Without it, no amount of disguise helps;
with it, the disguise of FR-004 is what makes the two indistinguishable.

### FR-008 · The page pins itself with a Service Worker
After the first visit the SW serves the page from its own cache and accepts an
update only when its hash is in the allow-set. The browser stops asking the
server, which is the ONLY mechanism here that answers attack B.

First visit remains trust-on-first-use. FR-004 + FR-007 are what harden that one
visit, which is the right place to spend them.

### FR-009 · A self-hosted page: pin the hash, do not turn the check off
Settings already lets a person name their own page. Verification must not lock
them out — but "turn it off" is the wrong door.

- First open of an unknown address shows the hash and asks: **trust this
  version?** Yes adds it to a **per-device allow-list**. The invariant holds —
  the wallet only ever runs bytes whose hash was decided in advance — and the
  self-hoster is protected too, because their own server being compromised still
  changes the bytes.
- A real off switch exists, but ONLY for a custom address, and never for the
  official one. There is no legitimate reason to accept unverified bytes from
  `sign.getvela.app`, and an off switch there is a social-engineering door
  ("just turn verification off to fix that error").
- While it is off, the signing screen carries a **standing** warning — not a
  dialog dismissed once, because the state outlives the dismissal.

### FR-010 · A per-device deny-list, and it outranks everything
A hash in the deny-list never opens, even if this build ships it as allowed, and
even if the person allow-listed it earlier. **Deny wins.** It is the answer to
"version X was just found compromised" without waiting for an app release.

- Settings shows the version in force (hash + when it was checked) with a
  **block this version** action — a person cannot block what they cannot see.
- A hash can also be pasted in, for a version not yet used.
- Each entry records when and why (the person's own sentence).
- **Per device.** The `vela.` preferences do not sync, and the UI says so.
- Blocking everything locks the Clear Signer out, so clearing the list exists —
  behind a confirmation.

### FR-011 · Keep checking afterwards
The client re-checks the same address from time to time after the page has been
used. It cannot undo a signature, but **knowing you were served something else**
is worth a great deal: the person can move their funds. The warning names the
expected and the actual hash.

## Success criteria

- **SC-001** A page whose bytes do not match any allowed hash does not open, and
  the screen names the expected and actual hashes.
- **SC-002** With `Cache-Control: no-store` on the response, nothing opens
  (FR-006) — verified against a server that sets it.
- **SC-003** The verification navigation and a real navigation are
  indistinguishable in the request the server sees: same UA, same `Sec-Fetch-*`,
  same TLS/H2 fingerprint. Measured against a server that logs both.
- **SC-004** A page loaded for verification makes no network request other than
  its own document — including `fetch`, XHR, `sendBeacon` and WebSocket.
- **SC-005** A deny-listed hash does not open even when it is in the build's
  allow-set and the person's allow-list.
- **SC-006** After a first visit the Service Worker serves the page and a server
  that changes the bytes cannot reach the browser at all.

## Open questions — to be MEASURED, not assumed

1. **`cache: 'only-if-cached'`** — does it return the navigation's response body
   on WKWebView and on Android WebView?

   **Android WebView: YES.** Measured 2026-09-23 on a real device (M2012K11AC,
   Android 13) by `SignerPageProbeTest`, against a counting server inside the
   test process:

   ```
   max-age=300 → OK status=200 bytes=12603 secure/subtle=true/true sha256=86810537…
                 the DOCUMENT was asked for exactly once
   no-store    → THREW TypeError: Failed to fetch
   ```

   `12603` is the page's own length to the byte, and the digest is the same one
   headless Chromium computes for the same page — so what came back is the
   navigation's bytes and not a re-fetch. `secure/subtle=true/true`: http on
   loopback is a secure context, so `crypto.subtle` is there to hash with. And
   `no-store` throws, which is precisely the fail-closed case FR-006 needs —
   the server that wants to defeat this check cannot do so silently.

   One correction to the probe itself: a browser asks for `/favicon.ico` on its
   own, so "asked once" must be counted per PATH. Counting every request would
   have reported a second fetch that never happened.

   **WKWebView: being measured** (`SignerPageProbeTests.swift`). WebKit's cache
   semantics are not Chromium's, and this is the answer that can still cost
   FR-004 its "keep the fingerprint" position.
2. **WebRTC / STUN** — content blockers govern HTTP; a page could try to leak
   over UDP.

   **Android: FR-005 as written has TWO holes.** Measured 2026-09-23 on an
   emulator (Pixel 7, API 34) by `SignerPageEscapeProbeTest`: a hostile page in
   a WebView whose `shouldInterceptRequest` returns an empty response for every
   subresource, against TCP and UDP servers in the test process.

   | way out | reached the server? |
   |---|---|
   | `fetch` | no — intercepted |
   | `XMLHttpRequest` | no — intercepted |
   | `navigator.sendBeacon` | no — intercepted |
   | image | no — intercepted |
   | **`WebSocket`** | **YES** — `GET /ws` arrived, and the interceptor never saw it |
   | **WebRTC** | **YES** — four 20-byte STUN packets arrived over UDP |

   `shouldInterceptRequest` never sees a WebSocket handshake, and WebRTC does
   not go over HTTP at all, so neither is governed by it. A page that can tell
   it is being verified could say so over either.

   A nice side effect of the part that DOES work: an intercepted `fetch` gets
   an empty 200, so the page's own code reports success. It cannot tell
   "blocked" from "the server sent nothing".

   **The candidate fix is not a network one.** The only place to stop an API
   that does not touch the HTTP stack is the environment the page runs in:
   `WebViewCompat.addDocumentStartJavaScript` removes `WebSocket` and
   `RTCPeerConnection` before any page script runs (the dApp browser already
   uses that API, so it is proven in this codebase). Being measured.

   That trade must be stated where the spec claims indistinguishability: a page
   CAN notice that `WebSocket` is missing. It cannot tell anyone — which is the
   point — but it can choose to behave during the check and misbehave later,
   and that is attack B, which only the Service Worker (FR-008) answers.
3. **Client Hints** — Chromium sends `Sec-CH-UA`, and Android WebView's brand
   list may name itself. Does it, on current versions, and can it be overridden?
4. **The publishing discipline** — the page must be up at its content-addressed
   path BEFORE a build that allows it ships, and old paths must stay for ever.
   Whose process is that, and what enforces it?
