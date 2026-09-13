# Data Model — 048

## Account record (persisted, `vela.accounts`)

| Field (current) | Old spelling (Expo, read only) | Notes |
|---|---|---|
| `id` | `id` | credential id of the first key |
| `name` | `name` | |
| `address` | `address` | |
| `public_key_hex` | `publicKeyHex` | |
| `created_at_iso` | `createdAt` | |
| `keys[]` | `keys[]` (may be absent) | default empty; absent = the single key is the account's key |
| `keys[].credential_id` | `keys[].credentialId` | |
| `keys[].public_key_hex` | `keys[].publicKeyHex` | |
| `keys[].name` | `keys[].name` | |
| `keys[].transports` | — | default `""` |

Invariant: only the current spelling is ever written; the web rewrites the
list once after reading an old record. Unknown fields are ignored.

`vela.pendingUploads` follows the same rule (`PendingUpload`, `PendingUploadMember`).

## Refused-answer failure (per machine)

| Machine | Failure event fed when the core refuses a shell answer | Visible result |
|---|---|---|
| login | `storage_failed` | `SignInFailed` prompt: sign in again / reset this browser's copy |
| session | `accounts_unavailable` | `Empty` → welcome screen + the same prompt |
| Android session | escaped failure → `SessionController.onFault` | logged, visible fault state, never `loading` forever |

## Chain filter (shell render state, one per list)

`ChainFilter { chainId: Int? }` — `null` = all. Applied to the assets list,
the send pick and (through `chain_filter_changed`) the activity feed. Shown
on the 全部网络 pill as the network's name.

## Class filter (send pick)

`SendClassFilter = all | stable | gas | other`, decided per token by the
web's `sendTokenClass` rule (stablecoin registry / native or gas-token /
other), ported verbatim.

## Haptic class

`Detent | Select | Success | Reject` — the only four the app performs; each
call logs `haptic <class>`.

## Share card

`ShareCardModel` + `code: String` (the address the QR encodes), `chainLogoUrl`.

## Contacts group

`ContactGroupInput(id?, name, color?, members?)` — new (no id), rename (id +
name), members (id + members). Delete by id with confirmation.
