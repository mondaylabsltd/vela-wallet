# Data Model — Android Live Shell (040)

Three layers, and the whole feature is the arrows between them:

```text
  Rust wire type          Kotlin wire class            drawn display model
  (vela-core)      ──►    (@Serializable)        ──►   (feature/*/…Models.kt)
  the rule                 the transcription             the picture
  ts-rs mirror            CoreWireDriftTest              already exists
```

Nothing in the middle column decides anything. Nothing in the right column is
new.

---

## 1. Wire classes Kotlin declares

One file per machine (`feature/<x>/core/…Wire.kt`), each a transcription of the
Rust enum/struct, checked by `CoreWireDriftTest` against
`app-web/vela-wallet/src/lib/core/generated/<Type>.ts`.

### contacts

| Kotlin | Mirrors | Notes |
| --- | --- | --- |
| `ContactsView` | `ContactsView.ts` | `loaded`, `contacts`, `groups`, `lastImport`, `recipient` |
| `Contact` | `Contact.ts` | epoch ms as `Double` — the Rust doc says `f64`, "no u64 crosses the wire" |
| `ContactGroupView` | `ContactGroupView.ts` | members are whole `Contact`s, in membership order |
| `ContactRecipientView` | `ContactRecipientView.ts` | `verified` = saved **and** starred; nothing else earns the check |
| `ContactKind` / `ContactSource` | enums | `eoa｜account｜unknown`, `manual｜auto` |
| `ContactOperation` | `ContactOperation.ts` | sealed, 7 variants |
| `ContactShellResult` | `ContactShellResult.ts` | sealed, 6 variants |
| `ContactEvent` | `ContactEvent.ts` | only the variants the drawn surface can raise |

### network_admin

| Kotlin | Mirrors | Notes |
| --- | --- | --- |
| `NetView` | `NetView.ts` | `loaded`, `networks`, `wizard`, `endpoints`, `providers`, `lastAddedChainId` |
| `NetNetworkRow` | `NetNetworkRow.ts` | `rpcHealth`/`explorerHealth` are `null` for the whole of 040 |
| `NetWizardView` | `NetWizardView.ts` | the add-network form's own state — phase, candidate, checks, error |
| `NetEndpointView`, `NetProviderView`, `NetProviderTestView` | same names | |
| `NetCustomNetwork`, `NetNetworkConfig`, `NetServiceEndpoints`, `NetProviderKeys` | same names | carried **in** operations, so they must round-trip exactly |
| `NetOperation` | `NetOperation.ts` | sealed, 16 variants |
| `NetShellResult` | `NetShellResult.ts` | sealed |
| `NetEvent` | `NetEvent.ts` | only the reachable variants |

### display_currency

| Kotlin | Mirrors | Notes |
| --- | --- | --- |
| `CurrencyView` | `CurrencyView.ts` | `code`, `rate: Double?`, `committed` |
| `CurrencyOperation` / `CurrencyShellResult` / `CurrencyEvent` | same names | 4 / 4 / small |

**`rate: Double?` is the type that must never become `Double`.** The Rust doc
comment spells out the consequence of defaulting it to 1: "a real 7× mispayment".
Kotlin's nullability is the enforcement, and `explicitNulls = false` must not be
allowed to hide it — the drift test asserts this field stays nullable.

---

## 2. Stored shapes (the cross-client compatibility contract)

All in one DataStore file (`vela_onboarding`), all `String` values holding JSON,
all camelCase — these are the bytes the web and desktop already read and write.

| Key | Shape | Owner |
| --- | --- | --- |
| `vela.accounts` | array of account records, **whole** | `AccountStore` (existing) |
| `vela.activeAccountIndex` | integer as string | `AccountStore` (existing) |
| `vela.pendingUploads` | array | `AccountStore` (existing) |
| `vela.serviceEndpoints` | `{ ethereumDataURL, passkeyIndexURL, bundlerServiceURL, fiatRatesURL }` | **shared** — `AccountStore` today, `network_admin` from now on |
| `vela.contacts` | array of `{ address, name?, resolvedName?, resolvedSource?, kind, favorite?, note?, txCount, lastUsed, firstSeen, source }` | `contacts` |
| `vela.contacts.dismissed` | **map** `address → epoch ms` | `contacts` |
| `vela.contactGroups` | array of `{ id, name, color?, members: string[] }` | `contacts` |
| `vela.displayCurrency` | a bare 3-letter code | `display_currency` |
| `vela.customNetworks` | array of custom-network records | `network_admin` |
| `vela.networkConfig` | array of per-network config records — **singular key name**, verified against `network-admin-executor.ts:54` | `network_admin` |
| `vela.rpcProviders` | provider-key record | `network_admin` |

**The one trap in this table**: `vela.serviceEndpoints` now has two readers.
That is why `VelaStore` exists (research D9) — two DataStore files holding the
same key is how the settings page and the onboarding endpoint sheet start
disagreeing about where the bundler is.

**Coercion rule, everywhere**: a value that cannot be parsed reads as *absent*,
never as a partially-populated record. A half-read address book is worse than
an empty one, because the person cannot tell it is half.

---

## 3. Display models (unchanged) and who fills them

The right-hand column already exists and is not modified. What is new is a
`…Live.kt` builder per feature, the sibling of the existing `…Fixtures.kt`.

| Display model | Built from | Presentation judgements the builder owns |
| --- | --- | --- |
| `ContactsHomeModel` | `ContactsView` + UI state (query, tab) | **letter sectioning** and **list search** — both are display-side narrowing/grouping of core-ruled rows, exactly as `app-web/.../contacts/live.ts:5-13` documents |
| `ContactDetailModel` | one `Contact` + its groups | the identity of the contact travels **with** the model |
| `GroupDetailModel` | one `ContactGroupView` | |
| `SettingsScreenModel` | `NetView` + `CurrencyView` + session + i18n strings | which page/overlay is open stays the screen's own state, seeded by the model |

**Parity, not reinvention.** Letter sectioning and search matching are rules
this shell duplicates from the web shell — so `ContactsLiveTest` runs the same
cases as `app-web/vela-wallet/src/lib/contacts/live.test.ts`, including the
ones that pin `#` sorting last and the core's relative order surviving inside
each letter. A drifted `sectionLetter` is a bug no compiler can see.

**Identity travels with the model.** `ContactModel` already carries
`addressFull`; every action (delete, copy, send) takes its target from the
rendered model and never from a separately-held selection index. The desktop
sibling shipped a detail page that displayed contact A while deleting contact
B, and it survived three review passes — the shape of that bug is a target
looked up twice.

---

## 4. Runtime objects

| Object | Lifetime | Why |
| --- | --- | --- |
| `VelaStore` | process | one DataStore, many readers |
| `CoreHost<Bridge>` | process, lazy | driver + `StateFlow<view>`; created on first use, never torn down (research D5) |
| `ContactsController` | process, lazy | its host + its executor + the events the screens raise |
| `SettingsController` | process, lazy | the network and currency hosts, presented as one screen state |
| `SessionController` | process | unchanged; the route guard predates this feature |

Each controller exposes exactly two things upward: a `StateFlow` of a display
model, and named intents. No screen ever sees a `CoreHost`, a `JSONObject`, or
an operation.
