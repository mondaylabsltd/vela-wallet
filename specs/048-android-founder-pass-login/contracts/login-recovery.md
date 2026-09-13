# Contract — login recovery

## 1. The core reads both spellings (serde aliases)

| Struct | Field | Alias accepted |
|---|---|---|
| `Account` | `public_key_hex` | `publicKeyHex` |
| `Account` | `created_at_iso` | `createdAt` |
| `AccountKey` | `credential_id` | `credentialId` |
| `AccountKey` | `public_key_hex` | `publicKeyHex` |
| `PendingUpload` / `PendingUploadMember` | their camelCase names | same rule |

Writes: snake_case only. Unknown fields: ignored.

Old-shaped fixture (with keys):
```json
[{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
  "publicKeyHex":"04ab","createdAt":"2026-08-25T10:00:00.000Z",
  "keys":[{"credentialId":"cred-1","publicKeyHex":"04ab","name":"Ann"}]}]
```
Without keys: the same record minus `keys` → `keys = []`, and the account's
own `id`/`public_key_hex` are its key.

## 2. A refused shell answer becomes the machine's failure

`effect-loop.ts` (web) and `CoreDriver.kt` (Android): when `core.resolve`
throws for effect `e`, call `onError(error)` (now required) AND feed
`toFailure(e, error)` once for that effect id:

| Machine | `toFailure` |
|---|---|
| login | `{ type: "storage_failed" }` → `SignInFailed` prompt |
| session | `{ type: "accounts_unavailable" }` → `Empty` |
| others | the machine's existing `*_failed` result where one exists; otherwise log only |

The prompt (web): title "This browser's wallet data can't be read", actions
"Sign in again" (retry) and "Reset this browser's copy" (clears
`vela.accounts`, `vela.activeAccountIndex`, `vela.pendingUploads` then
retries). Corpus keys added under `onboarding.storage.*` in all locales.

## 3. Tests

- core: `an_expo_era_record_still_opens_the_wallet` (login, with and without
  keys), `an_expo_era_record_restores_the_session` (session), `Account`
  equality across spellings.
- web: `storage.test.ts` (normalise, keys kept, rewrite once, `findAccountByCredentialId`),
  `effect-loop.test.ts` (refusal → `toFailure` fed once + `onError`),
  `e2e/login-old-shape.spec.ts`.
- android: `SessionOldShapeTest` (FakeStore seeded → view leaves `loading`,
  `onFault` called on an unreadable store).
