# Phase 1 Data Model — 050 iOS Live Shell

Three machines, eight storage keys, two alphabets. Everything here is a **contract**
rather than a design: the stored bytes are read and written by four clients, and the
wire shapes are the core's.

---

## 1. The two alphabets

| | Casing | Who defines it | Example |
|---|---|---|---|
| **Wire** — events, operations, results, views | `snake_case` | the Rust machines (no `rename_all`, verified) | `rpc_url`, `added_at_iso`, `display_name`, `first_seen_ms` |
| **Stored** — the JSON under a `vela.*` key | `camelCase`, with capitalised initialisms | the Expo client's services, kept by every client since | `rpcURL`, `addedAt`, `resolvedName`, `firstSeen` |

Translating between them is the **executor's** job and nobody else's. A live builder
never sees stored shapes; a screen never sees either.

Two traps in the initialisms, both already load-bearing in shipped data:
`rpcURL`, `explorerURL`, `bundlerURL`, `logoURL`, `passkeyIndexURL`,
`ethereumDataURL`, `bundlerServiceURL`, `fiatRatesURL` — **URL, not Url**.

---

## 2. Storage keys

`UserDefaults.standard`, value = JSON **text** (not a plist array/dict). This is what
`AccountStore.readList/writeList` already does, and it is what makes the value
byte-identical to web's `localStorage` and Expo's `AsyncStorage` entry.

| Key | Owner in this cut | Shape |
|---|---|---|
| `vela.contacts` | `contacts` | array of `StoredContact` |
| `vela.contacts.dismissed` | `contacts` | **object** `address → epoch ms`, not an array |
| `vela.contactGroups` | `contacts` | array of `StoredGroup` |
| `vela.customNetworks` | `network_admin` | array of `StoredCustomNetwork` |
| `vela.networkConfig` | `network_admin` | array of `StoredNetworkConfig` |
| `vela.rpcProviders` | `network_admin` | object of optional API keys |
| `vela.serviceEndpoints` | `network_admin` **and** `AccountStore` — see §5 | object, partial |
| `vela.displayCurrency` | `display_currency` | a bare currency code string |

Already owned by onboarding and untouched here: `vela.accounts`,
`vela.activeAccountIndex`, `vela.pendingUploads`.

**Corrupt reads answer empty.** Absent, unparseable, or wrongly-typed contents all
produce the empty value — never a throw. The core's own doc-comments assume it
(`ContactOperation::ReadStore`: *"Unreadable/corrupt answers as empty"*), and a
rejected `StoreLoaded` would strand the machine unloaded forever, silently dropping
every subsequent write.

---

## 3. Stored shapes

### `vela.contacts` — `StoredContact`

```jsonc
{
  "address": "0x…",              // lowercased on read
  "name": "…",                   // optional — OMITTED when absent, never null
  "resolvedName": "…",           // optional
  "resolvedSource": "…",         // optional
  "kind": "eoa" | "account" | "unknown",
  "favorite": true,
  "note": "…",                   // optional
  "txCount": 0,
  "lastUsed": 0,                 // epoch ms
  "firstSeen": 0,                // epoch ms
  "source": "manual" | "auto"
}
```

Wire counterpart `Contact` uses `resolved_name`, `resolved_source`, `tx_count`,
`last_used_ms`, `first_seen_ms`.

### `vela.contacts.dismissed` — `StoredDismissed`

```jsonc
{ "0xabc…": 1756944000000 }
```

An **object**, not a list. The wire is a list of `ContactTombstone { address,
dismissed_at_ms }`, so the executor pivots in both directions. Writing it as an array
makes every other client's tombstones vanish and history-derived contacts resurrect —
which is US1 acceptance scenario 2.

### `vela.contactGroups` — `StoredGroup`

```jsonc
{ "id": "…", "name": "…", "color": "…", "members": ["0x…"] }
```

### `vela.customNetworks` — `StoredCustomNetwork`

```jsonc
{
  "id": "…", "displayName": "…", "chainId": 100,
  "iconLabel": "…", "iconColor": "…", "iconBg": "…", "logoURL": "…",
  "isL2": false,
  "rpcURL": "…", "explorerURL": "…", "bundlerURL": "…",
  "nativeSymbol": "…", "addedAt": "2026-09-05T…Z"
}
```

Wire `NetCustomNetwork`: `display_name`, `chain_id`, `icon_label`, `icon_color`,
`icon_bg`, `logo_url`, `is_l2`, `rpc_url`, `explorer_url`, `bundler_url`,
`native_symbol`, `added_at_iso`.

### `vela.networkConfig` — `StoredNetworkConfig`

```jsonc
{ "chainId": 100, "rpcURL": "…", "explorerURL": "…", "bundlerURL": "…" }
```

### `vela.rpcProviders` — `NetProviderKeys`

```jsonc
{ "alchemy": "…", "drpc": "…", "ankr": "…" }
```

A **cleared key is removed**, not stored as an empty string — the core's invariant ⑦,
and the executor must honour it when encoding.

### `vela.serviceEndpoints` — partial `ServiceEndpoints`

```jsonc
{ "ethereumDataURL": "…", "passkeyIndexURL": "…",
  "bundlerServiceURL": "…", "fiatRatesURL": "…" }
```

Fields **absent** rather than null when unset, so the core applies its own defaults
merge (`NetStoredEndpoints` is all-`Option`, and its doc says so).

### `vela.displayCurrency`

A bare code — `"CNY"`. Absent **always** means "the person never chose", which is
different from "chose USD"; the core's seed decision depends on that difference.

---

## 4. Wire types the shell decodes

Only what a screen renders. Everything else stays opaque JSON.

| Machine | View | Feeds |
|---|---|---|
| `contacts` | `ContactsView { loaded, contacts[], groups[], last_import?, recipient? }` | `ContactsScene` — home / detail / group |
| `network_admin` | `NetView` (the networks list, detail, wizard candidate, verdicts, endpoint + provider panels) | `SettingsModel` pages `.networks`, `.networkDetail`, `.addNetwork`, `.rpcProviders`, `.endpoints` |
| `display_currency` | `CurrencyView` (chosen code, list, rate state) | `SettingsModel` overlay `.currency`, and every fiat figure |

Decoded with `JSONDecoder` and `.keyDecodingStrategy = .convertFromSnakeCase`.
`loaded: false` is a **real state**, not an absence — it is what FR-009's neutral
surface renders from.

---

## 5. `vela.serviceEndpoints` has two writers

`AccountStore.saveRegistryURL` merges `passkeyIndexURL` into this key today, for the
passkey-index endpoint override the onboarding sheet offers. `network_admin`'s
`WriteServiceEndpoints` writes all four fields to the same key.

Two independent writers on one key means the person's custom passkey-index endpoint
disappears the first time they open 设置 → 端点 — and, worse, silently, because both
writes succeed.

**Contract**: the `network_admin` executor reads and writes this key **through
`AccountStore`**, never through `VelaStore` directly. Web reached the same conclusion
and recorded it at `network-admin-executor.ts:411`. A regression test asserts that a
saved passkey-index override survives a `WriteServiceEndpoints`.

---

## 6. What is not modelled here

`LoadSendHistory` (a transaction store — 052), `ResolveIdentity` and
`ClassifyRecipient` (the RPC pool — 051), `ResolveRate` and `FetchFiatRates`
(the price path — 051). Their storage shapes belong to the cut that fills them.
