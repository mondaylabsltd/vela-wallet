# K01 · WalletPair Pairing Over WebSocket Relay (QR, No BLE)

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | K02, K03, K04, H04 |

## 1. Summary

Vela connects to dApps via **WalletPair over a WebSocket relay** — **not Bluetooth**. Pairing is a relay
URI carried in a **QR code** (scanned via H04); the channel is **end-to-end encrypted** with
out-of-band **fingerprint** verification (K02). This replaces an embedded dApp browser (a deliberate
non-goal, A01) with a delegated, minimal-surface connection.

## 2. Background & context

An in-app dApp browser is a large attack surface; WalletPair keeps the wallet minimal by connecting to
desktop dApps over an encrypted relay. **There is no Bluetooth** — legacy `BLE*` type names are
artifacts and must never be described as Bluetooth pairing.

## 3. Users & stories

- As a **user**, I want to sign for a desktop dApp from my wallet, so that I don't paste keys into a browser.
- As a **user**, I want pairing by scanning a QR, so that connecting is quick and doesn't need Bluetooth.

## 4. Functional requirements

- **FR-1** — Pair by scanning a WalletPair relay URI QR (H04); establish an **E2E-encrypted** WebSocket channel.
- **FR-2** — **No Bluetooth** anywhere; the transport is the relay only.
- **FR-3** — Require out-of-band fingerprint verification before joining (K02).
- **FR-4** — Advertise capabilities to the dApp on session creation (K04).
- **FR-5** — Exactly one session at a time (single-session model, K03).

## 5. Non-functional requirements

- **NFR-1** — E2E encryption; the relay cannot read message contents.
- **NFR-2** — Uses the `walletpair-sdk`; transport concerns isolated in `walletpair-transport.ts`.

## 6. UX / flow notes

Connect tab → scan pairing QR → fingerprint confirm (K02) → connected card with E2E badge. No Bluetooth prompts ever.

## 7. Acceptance criteria

- [ ] **AC-1** — Scanning a valid pairing QR establishes an E2E-encrypted session after fingerprint confirm.
- [ ] **AC-2** — No Bluetooth API is used or requested.
- [ ] **AC-3** — Capabilities are advertised to the dApp on connect (K04).

## 8. Out of scope / non-goals

- Fingerprint/E2E details — **K02**; session lifecycle — **K03**; capabilities — **K04**. Desktop-without-phone connect is 🧭 roadmap.

## 9. Dependencies, risks & open questions

- **Risk:** any doc/UI calling this "Bluetooth" is wrong — guardrail (A02).
- **Open question:** None.

## 10. Source anchors

- `src/services/walletpair-transport.ts:5-9,288-320`, `walletpair-sdk`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 32 (+ guardrails: no BLE).
