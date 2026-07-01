# A06 · Local Persistence & Storage Model

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | A03, C05, F04, H05, N02, E06 |

## 1. Summary

All app state is persisted **locally on the device** via AsyncStorage — accounts, selected network,
custom tokens, contacts & groups, RPC bans, price/metadata caches, display currency, and service
endpoints. There is **no server-side user store** (A03). Private keys are never stored anywhere (they
live in the OS secure enclave, B01). This doc defines the storage contract and its safety properties.

## 2. Background & context

Because Vela has no backend user account, the device *is* the source of truth for everything except
the on-chain public-key index (B08). Storage must be namespaced, resilient to corrupt/partial data,
and never a place a secret could leak into.

## 3. Users & stories

- As a **user**, I want my accounts, networks, tokens, and contacts to persist across launches, so that I don't reconfigure each time.
- As a **user**, I want caches to speed up load without ever showing stale-as-fresh data, so that the app is fast but honest.

## 4. Functional requirements

- **FR-1** — Persist, under stable namespaced keys: accounts & active account (C05), selected/added networks (F01/F02), custom tokens (D03), contacts & `vela.contactGroups` (H05/H06), RPC ban lists (F04), token metadata & price caches (D04/E01), display currency (E06), service endpoints (N02).
- **FR-2** — **No private key or signing secret is ever written to storage** — keys stay in the WebAuthn/secure-enclave domain (B01/A03).
- **FR-3** — Reads tolerate missing/corrupt JSON: fall back to a safe default (e.g. empty list) rather than throwing.
- **FR-4** — Caches record their own freshness so a consumer can distinguish "cached" from "fresh"; a cache miss never renders as a confident value (e.g. price `null`, not `0`).
- **FR-5** — Cache keys that depend on a swappable input (e.g. FX endpoint URL, E05) are keyed by that input so a config change refetches instead of serving the old value.

## 5. Non-functional requirements

- **NFR-1** — Storage access is async and off the render path; boot (A05) must not block on a slow read.
- **NFR-2** — In-memory layer in front of AsyncStorage for hot reads (metadata, contacts, groups).
- **NFR-3** — Forward-compatible: unknown fields are preserved, not dropped, on read/rewrite where feasible.

## 6. UX / flow notes

No direct UI. Corrupt-data recovery is silent (default + repair). "Restore whole setup on a new device" is a roadmap item (🔜) that will build on this contract plus passkey sync (C04).

## 7. Acceptance criteria

- [ ] **AC-1** — Killing and relaunching the app restores accounts, network, tokens, and contacts unchanged.
- [ ] **AC-2** — A deliberately corrupted stored value loads as its safe default without crashing.
- [ ] **AC-3** — No storage key ever contains a private key or raw signature.
- [ ] **AC-4** — Swapping the FX endpoint (E05) causes an immediate refetch (endpoint-keyed cache).

## 8. Out of scope / non-goals

- Cross-device sync of this state — roadmap (🔜), tracked in C04/H05/E06.

## 9. Dependencies, risks & open questions

- **Risk:** schema drift between app versions; needs defensive reads and, eventually, explicit migrations.
- **Open question:** whether to add a formal migration/versioning layer before store launch (O03).

## 10. Source anchors

- `src/services/storage.ts` — storage wrapper.
- `src/services/contacts.ts:63` (`GROUPS_KEY = 'vela.contactGroups'`), `src/services/rpc-pool.ts` (persisted bans), `src/services/fiat-fx.ts` (endpoint-keyed cache).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 30, 56, 75, 78.
