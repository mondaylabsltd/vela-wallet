# Data model — 081

Five entities. Each lives in `vela-core` and crosses the wire to all four shells, so each one's shape is also a contract ([contracts/](contracts/)).

## 1. SelfCallBlock (new)

The core's verdict on a dApp-originated request.

| Field | Type | Notes |
| --- | --- | --- |
| `function` | enum `SelfCallFunction` | `AddOwner, RemoveOwner, SwapOwner, ChangeThreshold, EnableModule, DisableModule, SetGuard, SetModuleGuard, SetFallbackHandler, Setup, ExecTransaction, ExecTransactionFromModule, SafeTxTypedData` |
| `selector` | `String` | `0x…` 4 bytes, or empty for the typed-data case |
| `leg_index` | `Option<u32>` | which call in a batch caused it (1-based for display) |
| `nested` | `bool` | true when found inside a MultiSend or an `execTransaction` payload |

Rules: produced only for requests whose origin is a dApp; `to == the signing account's address` (case-insensitive) **and** the selector is in the list, or the request is typed data with `primaryType == "SafeTx"` and `domain.verifyingContract` equal to a Safe the user's key owns. Empty calldata to self is **not** a block. Depth cap 4 when recursing.

State: a request carrying a block never becomes signable — `SignView.confirm_gate_open = false`, `SignErrorKind::SelfCallBlocked` returned to the dApp as `-32603`.

## 2. ClearProvenance (new, on `ClearSignResult`)

| Value | Meaning | `verified` |
| --- | --- | --- |
| `BuiltIn` | descriptor compiled into the app | true |
| `PinnedMatch` | fetched descriptor equal to the built-in one for that address | true |
| `Fetched` | fetched, unauthenticated | **false** |
| `Standard` | token-standard shape (ERC-20/721/1155/4626) | false |
| `SelectorDb` | public 4-byte database, best effort | false |

`verified` is derived, never set independently. Risk assessment is unchanged; only the label changes.

## 3. NetworkReadiness (extends `NetCompatibility`)

| Field | Type | Notes |
| --- | --- | --- |
| `contracts` | `Vec<ContractStatus>` | required set becomes the 12 the wallet uses: deterministic deployment proxy, Safe singleton factory, Multicall3, EntryPoint v0.7, SafeL2, Safe proxy factory, Safe 4337 module, Safe module setup, WebAuthn shared signer, MultiSend, **SafeWebAuthnSignerFactory**, **its singleton**; `CompatibilityFallbackHandler` is removed |
| `p256_available` | `bool` | unchanged |
| `compatible` | `bool` | single-key readiness: everything except the two signer-factory entries |
| `multi_key_ready` | `bool` | **new**: `compatible` plus the factory and its singleton |

A network that is `compatible && !multi_key_ready` is usable by a one-key wallet and must say so rather than showing a green tick.

## 4. VerifiedName (rule around `ContactIdentity`)

| Field | Type | Notes |
| --- | --- | --- |
| `name` | `String` | as returned by the reverse record |
| `source` | enum | ENS, Basename, .bnb, .arb, .g, Vela registry |
| `forward_state` | enum `Unchecked \| Verified \| Mismatch \| Unavailable` | **new** |

Display rule: a name is shown only when `forward_state == Verified`; `Mismatch` and `Unavailable` show the address alone. Cache rule: only `Verified` entries are cached (24 h, as today); caches from before this change are dropped by a version bump. Vela registry wallet names are labels, not resolvable records — they keep `Unchecked` and are shown, labelled as wallet names.

## 5. EraseScope (shell-owned, one contract)

Per shell, the complete list of local stores to clear, with one keep-list.

| Shell | Must clear | Keep |
| --- | --- | --- |
| web | `vela.` keys in localStorage, IndexedDB `vela/kv`, `chrome.storage.local` | `vela.pendingUploads` |
| desktop | every `vela.` key in the state document, the wry webview's browsing data | `vela.pendingUploads` |
| Android | DataStore `vela_onboarding` **and** `settings`, `vela_crash` prefs, WebView data (cookies, DOM storage, databases), log files, logo and shared caches | accounts, active index, `vela.pendingUploads` |
| iOS | every `vela.` key in UserDefaults, `WKWebsiteDataStore.default()`, the logo `URLCache` | `vela.pendingUploads` |

Post-condition everywhere: re-read finds nothing but the keep-list, the session ends (sign-out **confirmed**), and the user is told that passkeys held by their passkey provider are untouched.
