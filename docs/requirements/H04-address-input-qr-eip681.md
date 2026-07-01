# H04 · Address Input, QR Scan & EIP-681 Parsing

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | H01, H09, K01 |

## 1. Summary

Recipients can be entered by typing/pasting an address or by **scanning a QR code**. Vela parses
**EIP-681** payment URIs (`ethereum:` addresses with optional chain/amount/token), validates addresses,
and feeds the result into the send flow (H01). The QR scanner is a fullscreen camera experience that
also works on web (via a WASM decoder).

## 2. Background & context

QR is the dominant way to move an address between devices. Supporting EIP-681 means a scanned payment
request can pre-fill chain, token, and amount, not just the address. Camera is the app's **only**
permission (A03), so the scanner must be well-behaved.

## 3. Users & stories

- As a **user**, I want to scan a QR to fill in a recipient, so that I don't mistype an address.
- As a **user**, I want a scanned payment request to pre-fill amount/token, so that paying an invoice is one step.

## 4. Functional requirements

- **FR-1** — Accept typed/pasted addresses with checksum/format validation.
- **FR-2** — Fullscreen QR scanner (native camera; web via WASM/`zbar-wasm`/`jsqr`) that decodes address and EIP-681 URIs.
- **FR-3** — Parse **EIP-681**: extract address, optional chainId, token (for ERC-20 transfers), and amount; pre-fill the send flow (H01).
- **FR-4** — The same scanner reads WalletPair pairing QRs for dApp connect (K01).
- **FR-5** — Handle malformed/invalid QRs gracefully (clear error, no crash).

## 5. Non-functional requirements

- **NFR-1** — Camera permission is the only permission (A03); scanner releases the camera when dismissed.
- **NFR-2** — Web decode path works without native modules (A04).

## 6. UX / flow notes

`QRScanner` full-screen with an emerging Scan FAB on home (WaveDock, M04). Web QR handling documented in `docs/qr-scanner-web.md`.

## 7. Acceptance criteria

- [ ] **AC-1** — Scanning an address QR fills the recipient.
- [ ] **AC-2** — An EIP-681 URI pre-fills chain/token/amount where present.
- [ ] **AC-3** — A WalletPair QR routes to dApp connect (K01), not the send flow.

## 8. Out of scope / non-goals

- Receive-side QR generation — **H09**; dApp pairing logic — **K01**.

## 9. Dependencies, risks & open questions

- **Risk:** ambiguous QR payloads (address vs pairing vs EIP-681) — disambiguate by scheme/prefix.
- **Open question:** None.

## 10. Source anchors

- `src/components/QRScanner.tsx`, `src/services/eip681.ts`, `src/services/image-decode.ts`, `src/services/qrcode.ts`.
- `docs/qr-scanner-web.md`.
