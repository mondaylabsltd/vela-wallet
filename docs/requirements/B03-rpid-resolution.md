# B03 · rpId Resolution Across Native / Web / Subdomains

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 |
| **Related** | B04, A04 |

## 1. Summary

WebAuthn binds a passkey to an **rpId (domain)**. Vela resolves the rpId so that the **same passkey
works across `getvela.app` and `wallet.getvela.app`** and across native and web. Native uses
`getvela.app`; web reduces the current host to its registrable domain; the proxy extension (B04) can
override the rpId for localhost/preview domains.

## 2. Background & context

If the rpId differs between the marketing domain and the wallet subdomain, the same user would get
*different* passkeys (and thus different wallets) — a silent footgun. Consistent rpId resolution keeps
one identity across all official surfaces, and the override hook makes development and domain-loss
recovery possible.

## 3. Users & stories

- As a **user**, I want the same wallet whether I open `getvela.app` or `wallet.getvela.app`, so that my identity is stable.
- As a **developer**, I want to run against localhost using the production rpId, so that I test the real passkey (via B04).

## 4. Functional requirements

- **FR-1** — Native resolves rpId to `getvela.app`.
- **FR-2** — Web reduces the current host to the **registrable domain** so subdomains share passkeys (`wallet.getvela.app` ≡ `getvela.app`).
- **FR-3** — If `window.__VELA_WEBAUTHN_PROXY_RPID__` is set (by the proxy extension B04), it overrides the resolved rpId.
- **FR-4** — Registration (B01) and every signing `get()` (B02) use the *same* resolution logic.

## 5. Non-functional requirements

- **NFR-1** — Deterministic: same host → same rpId, so the same passkey/wallet resolves every time.
- **NFR-2** — Only the two canonical domains (A02) are treated as production rpIds.

## 6. UX / flow notes

No direct UI; invisible when on a canonical domain. On non-canonical domains without the extension, WebAuthn will fail — B04 documents the escape hatch.

## 7. Acceptance criteria

- [ ] **AC-1** — A passkey created on `getvela.app` is usable on `wallet.getvela.app`.
- [ ] **AC-2** — Setting the proxy override changes the effective rpId.
- [ ] **AC-3** — Native consistently uses `getvela.app`.

## 8. Out of scope / non-goals

- The proxy extension implementation — **B04**.

## 9. Dependencies, risks & open questions

- **Risk:** a domain change would invalidate all passkeys — B04 is the mitigation; treat any rpId change as auto-High-risk (O02).
- **Open question:** None.

## 10. Source anchors

- `src/modules/passkey/index.ts:33-54` — rpId resolution + proxy override.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 21, 22.
