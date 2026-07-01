# B04 · WebAuthn Proxy Extension (Domain-Loss / Dev Passkeys)

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B03 |
| **Related** | B01, B09, A02 |

## 1. Summary

An open-source **Chrome extension** (`chrome-ext-webauthn-proxy/`, v1.3.0) proxies
`navigator.credentials` so that localhost / preview domains can share the `getvela.app` rpId. It is
both a **developer tool** and a **disaster-recovery escape hatch**: if the `getvela.app` domain ever
changes or is lost, users can still reach their passkeys by pinning the original rpId. This pre-empts
the "what if you shut down?" objection.

## 2. Background & context

WebAuthn's domain-binding is a security feature that becomes a liability if the domain disappears.
Because Vela is fully self-hostable (A01) but passkeys are rpId-bound, there must be a way to present
the original rpId from any origin. The extension does exactly that, in the open.

## 3. Users & stories

- As a **developer**, I want to test the production passkey from localhost, so that I don't need a fake dev identity.
- As a **long-term user**, I want assurance my wallet survives Vela's domain going away, so that I'm not trusting a single point of failure.

## 4. Functional requirements

- **FR-1** — The extension intercepts `navigator.credentials.create/get` and injects the configured rpId (default `getvela.app`) via `window.__VELA_WEBAUTHN_PROXY_RPID__` (consumed by B03).
- **FR-2** — rpId is **configurable** so a self-hoster can pin their own or the original domain.
- **FR-3** — Works on supported preview/localhost domains listed in the extension.
- **FR-4** — v1.1.0+ self-heals the Chrome site-access toggle with an in-popup "Grant access" prompt when Chrome resets it.

## 5. Non-functional requirements

- **NFR-1** — Open source (MIT) and auditable, consistent with the "verify, don't trust" stance.
- **NFR-2** — No data exfiltration — it only rewrites the rpId locally.

## 6. UX / flow notes

Extension popup shows current rpId + a grant-access prompt when needed. Documented in the root README's WebAuthn Proxy section.

## 7. Acceptance criteria

- [ ] **AC-1** — With the extension active on localhost, a `getvela.app`-rpId passkey is usable.
- [ ] **AC-2** — Changing the configured rpId changes the injected value.
- [ ] **AC-3** — After Chrome resets site access, the popup surfaces a working "Grant access" prompt.

## 8. Out of scope / non-goals

- rpId resolution in the app — **B03**; recovery model — **B09**.

## 9. Dependencies, risks & open questions

- **Risk:** users must install a browser extension for the recovery path — document clearly as an escape hatch, not a daily requirement.
- **Open question:** a non-Chromium equivalent for other browsers (not built).

## 10. Source anchors

- `chrome-ext-webauthn-proxy/manifest.json` and extension source (v1.3.0).
- `README.md:163-219` — proxy setup & notes.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 21, 22; memory `reference_webauthn_proxy_extension`.
