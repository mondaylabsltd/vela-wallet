# K06 · SIWE Parsing & Domain-Binding Defense

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K05 |
| **Related** | I01, I06 |

## 1. Summary

Vela's **Sign-In-With-Ethereum (EIP-4361)** parser is conservative — it **requires the canonical anchor
line** and **rejects userinfo spoofs** like `uniswap.org@evil.com`. `checkSiweDomainBinding` compares
the message's stated domain to the request origin: a match shows a green **"Sign in to {domain}"**; a
mismatch shows a **danger banner**. This is a targeted phishing defense on the login surface.

## 2. Background & context

SIWE is a common phishing vector: a malicious site asks you to sign in "to uniswap.org" while served
from an evil origin. Binding the message domain to the actual request origin — and refusing to be fooled
by URL userinfo tricks — turns a login into a checkable claim.

## 3. Users & stories

- As a **user**, I want a clear "Sign in to {domain}" when the domain matches, so that logins feel safe.
- As a **user**, I want a danger warning when the domain doesn't match the origin, so that I don't sign into a phishing site.

## 4. Functional requirements

- **FR-1** — Parse SIWE (EIP-4361) conservatively: require the canonical anchor line; reject malformed messages.
- **FR-2** — Reject userinfo spoofs (`domain@evil.com`-style) — don't be fooled by URL tricks.
- **FR-3** — `checkSiweDomainBinding`: compare message domain vs request origin.
- **FR-4** — On match → green "Sign in to {domain}"; on mismatch → **danger banner** (I08 danger).
- **FR-5** — Sign via the EIP-1271 SafeMessage path (K05) after user confirmation (B02).

## 5. Non-functional requirements

- **NFR-1** — Parser errs toward rejecting ambiguous messages (fail-safe).
- **NFR-2** — Domain comparison is exact/normalized, resistant to homograph/userinfo tricks.

## 6. UX / flow notes

`SigningRequestModal` renders the SIWE result: match = green sign-in; mismatch = red danger banner. Copy names the exact domain.

## 7. Acceptance criteria

- [ ] **AC-1** — A SIWE message whose domain matches the origin shows "Sign in to {domain}."
- [ ] **AC-2** — A domain/origin mismatch shows a danger banner.
- [ ] **AC-3** — A `uniswap.org@evil.com` userinfo spoof is rejected/flagged, not treated as uniswap.org.

## 8. Out of scope / non-goals

- Signature encoding — **K05/G02**; general typed-data decoding — **I06**.

## 9. Dependencies, risks & open questions

- **Risk:** dApps with legitimately unusual SIWE messages — conservative parser may reject; acceptable trade-off.
- **Open question:** None.

## 10. Source anchors

- `src/services/siwe.ts`, `src/components/SigningRequestModal.tsx:1019-1061` — parse + `checkSiweDomainBinding`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 33.
