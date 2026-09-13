# Results — 030 desktop-live-shell

Delivery ledger in the 019 format. Baselines first, because "did this grow?" has no
answer after the fact.

## Baselines — recorded 2026-09-04, branch point `dede5a4c` (tip of `029-native-repair`)

**Stacked on 029, not on `main`, deliberately.** Both cuts heavily edit
`app-desktop/vela-wallet/src/wallet/page.rs` — 029 added `Section::Explore` and a
379-line render body, 030 adds the live settings and contacts bindings to the same
file. Branching from `main` would guarantee a conflict, and 030's gate is the desktop
CI job 029 introduced.

### Desktop source at the branch point

| File | Lines |
|---|---|
| `src/**/*.rs` (57 files) | **32,050** |
| `wallet/page.rs` | **4,172** ⚠ |
| `settings/fixtures.rs` | 564 |
| `settings/components.rs` | 874 |
| `contacts/fixtures.rs` | 509 |
| `contacts/components.rs` | 564 |
| `executor/storage.rs` | 479 |
| `executor/mod.rs` | 391 |
| `core_host.rs` | 157 |
| `session.rs` | 230 |

⚠ `page.rs` is the risk this cut has to manage: it is 4,172 lines and every phase
touches it. The model/live seam exists precisely to keep the *decisions* out of it —
if a phase's `page.rs` delta exceeds ~400 lines, the panel wiring splits into
`settings/panels.rs`, for which `flows/panels.rs` (1,230 lines) is the precedent.

### Test baseline

`cargo test`: **93 passed · 0 failed · 5 ignored** (93 compiled; the 5 ignored need
hardware or the live registry). Inherited from 029, which took it from 88.

### The three machines (core side, already written and tested)

| Machine | Lines | Operations | Events |
|---|---|---|---|
| `network_admin` | 2,900 | 15 | 21 |
| `contacts` | 1,385 | 7 | 11 |
| `display_currency` | 413 | 4 | 3 |

**4,698 lines of rules this cut does not write.** The desktop links them directly —
no bridge, no JSON — so the entire cost is executors and rendering.

### Port provenance — web 024, at `f9bcb278`

| Source | Lines |
|---|---|
| `settings/core/network-admin-executor.ts` | 568 |
| `settings/live.ts` | 404 |
| `contacts/core/contacts-executor.ts` | 276 |
| `contacts/live.ts` | 216 |
| `settings/core/network-admin.svelte.ts` | 87 |
| `settings/core/currency-executor.ts` | 73 |
| `settings/core/currency.svelte.ts` | 67 |
| `services/storage.ts` | 95 |

Not ported, and the reason is the whole shape of this cut: `core/effect-loop.ts` (126)
and `core/json-shell.ts` (44) have **no desktop counterpart** — `CoreHost<A>` already
does that job in-process, in Rust enums, with no serialization at all.

## Phase 1 — the road, proven by the smallest machine

`src/resident.rs` (the generic gpui host), `executor/storage.rs` (+7 keys, generic
accessors, and a fixed bug), `executor/display_currency.rs`, `settings/live.rs`, and
the 货币 row bound to the core.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **104 passed · 0 failed · 5 ignored** (baseline 93) |
| `cargo fmt --all --check` | ✅ clean |
| warnings | ✅ **10** — exactly the inherited baseline, none added |
| `scripts/sweep-gallery.sh` | ✅ every state rendered |
| `*fixtures.rs` diff | ✅ **empty** — not one constant touched |
| `rust/` diff | ✅ empty |

### Why the road ships with a machine rather than alone

The plan had phase 1 land the plumbing behaviour-neutrally. Done literally that adds
**18 dead-code warnings** for one commit — nothing consumes a road nobody drives on —
and a phase boundary whose artifact is a warning spike is a bad boundary. So the road
ships proven by `display_currency`: 4 operations, 413 lines of core, the smallest of
the three. Everything is live, and the warning count came back to the baseline 10.

**This moves the SC-004 probe to `contacts`, and costs nothing.** The probe's power is
being *third*, not being smallest — it measures whether the road was paved, and any
third machine measures that. It arguably improves: `network_admin` is now second, and
it is the one machine here that must do HTTP, so if the road needs changing that is
discovered at machine two rather than hidden behind an easy third.

### The bug the storage extension had to fix first

`save_registry_endpoint` wrote `json!({ "registry": url })` — a **whole-value write**
to `vela.serviceEndpoints`, under a **desktop-only field name**. Two defects, both
live the moment `network_admin` shares that key:

1. The write is destructive. Saving the index endpoint would erase
   `ethereumDataURL` / `bundlerServiceURL` / `fiatRatesURL`, and saving those would
   erase the index — a self-hosted stack quietly un-configuring itself.
2. Every other client spells it **`passkeyIndexURL`** (web's `services/endpoints.ts`,
   the Expo client). The desktop's "registry" *is* the passkey index —
   `executor/registry.rs` defaults to `p256-index-v2.getvela.app`. So a record written
   on desktop was invisible everywhere else.

Now: `merge_value` writes field-by-field, reads prefer `passkeyIndexURL` and fall back
to the legacy name, and a save drops the legacy field in the same write so the two
cannot disagree. Three tests pin it, including the pre-existing sign-out scope test,
which still passes unchanged.

### What `rate: null` bought, concretely

`settings/live.rs::currency_row_value` has two cases and the second is the point. A
priced currency renders `USD · $1,234.56` — byte-identical to what the DST3 mock
draws, so going live does not silently redraw a reviewed screen. An **unpriced** one
renders the code **alone**: not the code beside a USD figure wearing its symbol.
Rendering `¥1,234.56` for an unpriced JPY would assert an exchange rate nobody
obtained, which is exactly the claim `rate: None` exists to refuse. There is a test
whose only job is that the string contains no figure.

Two smaller things the tests pinned rather than assumed:
- CLDR separates a fallback alphabetic symbol from its digits with a **non-breaking
  space** (U+00A0), not a plain one. My first assertion used a plain space and failed;
  the character is now pinned literally, because the difference is invisible until it
  shows up as a bad line break.
- `read_device_currency` answers `None` **by choice**, not by inability. A desktop has
  a region (`Loc::from_env` resolves `LC_ALL`/`LANG`), unlike the browser the core's
  comment was written about. But the core persists a seed only after a real rate
  resolves, because "a seeded currency rendering at the rate-1 fallback (₫78 instead
  of ₫2,000,000) is strictly worse than staying on USD" — so with no rate source this
  cut, seeding buys a label that cannot commit. Owed to 031, with its region table.

## Phase 2 — `network_admin` can talk to the world

`executor/network_admin.rs`: fifteen operations — four stored ledgers with their
camelCase codecs, six live probes, a debounce timer, and two acknowledged no-ops.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **111 passed · 0 failed · 6 ignored** (phase 1: 104/0/5) |
| `cargo fmt --all --check` | ✅ clean |
| `cargo build` warnings | ✅ **1**, and it is `BLE_CHANNEL_SUPPORTED`, pre-existing |
| `cargo clippy --all-targets` | ✅ **10**, all in `ctap/cable.rs`(7), `cable/l2cap.rs`, `hardware.rs`, `onboarding.rs` — **zero in any file this cut wrote** |
| live probes vs. a real chain | ✅ see below |

**A measurement gotcha, since these numbers are evidence.** `cargo build` only
re-emits warnings for crates it actually recompiled, so an incremental build reports
a number that means nothing. Every count above is after `touch src/main.rs`.

### The probes are live, and they answer

`cargo test …the_probes_answer -- --ignored`, against Gnosis mainnet:

```
eth_chainId    -> 100 in 620ms
eth_getCode    -> 172 bytes of runtime code   (the golden Safe, genuinely deployed)
eth_call(P256) -> 0x…0001                      (the precompile IS supported)
```

That last value is the point of the whole phase: it is what `add_confirmed`
hard-gates on. `#[ignore]`d, because a CI runner is not promised a network and a gate
that fails on a flaky connection is a gate people learn to re-run rather than read.

### A bug the live run found, and a host that found it

The first live run failed, and chasing it turned up two separate things.

**One real bug, in this cut's code.** `ureq` 3 treats a non-2xx response as an
**`Err`**, not an `Ok` carrying a status — `registry.rs:112` already says so in its
own words: *"StatusCode is the ONLY variant that means the server answered."* Reading
the status off an `Ok`, as the first version did, made the `HttpError` arm
**unreachable**: every 4xx and 5xx reported as `Failed`. The core renders those
differently ("HTTP 502" against "Connection failed"), so the settings screen would
have said a wrong true thing about every service that answered with an error. Fixed
with an `http_error` splitter.

**One host, not a bug.** `https://rpc.gnosischain.com` answers curl with 200 and this
client with **403** — with and without a proxy, measured by running the same request
through a bare `ureq::Agent` and through `proxy::agent`, against all three Gnosis
URLs. The other two answer normally. It is a property of that host.

Worth recording beyond this cut: **`registry.rs:362` pins that host FIRST** for
onboarding's legacy-name lookup, so every such lookup burns a request on an endpoint
that will refuse it before falling through. Not this feature's to fix, and not
invisible any more. It is also, incidentally, the argument for the core routing a
*pool* rather than one URL.

### Three things the warnings caught that mattered

Chasing the warning count to zero found a real gap rather than tidying:

1. **`drop_all` was never called.** `resident.rs` documents it as running on
   sign-out; nothing invoked it. A resident outliving a sign-out shows the previous
   person's address book to the next one. Now wired into `main.rs`'s
   `SessionRoute::Onboarding` arm, beside the existing `self.wallet = None`.
2. **`Machine::LABEL` was decoration.** It now prints one line per boot, in the
   file's existing `[vela-wallet]` voice.
3. **Three contacts keys were a phase early.** Moved to the phase that uses them —
   a constant nothing reads is a claim nothing checks.

### Deviations, recorded

- **A corrupt record is dropped, where web coerces it to zeros and keeps it.** Both
  keep the load alive, which is the invariant that matters (a rejected `StoreLoaded`
  strands the core unloaded forever and every later write is dropped). Dropping is
  safer here: a network whose `chainId` did not parse has chain 0, and chain 0 is the
  key the core dedups and routes on, so keeping it puts a colliding ghost in the
  ledger.
- **`probe_reachable` reads the real status.** On the web it must be a `no-cors`
  request whose only honest signal is "resolved without throwing", because the browser
  hides the status. A desktop has no CORS, so it answers 2xx/3xx — strictly better
  information into a field the core already types as a bool.

## Phase 3 — the 网络 panel is the core's, and the seam that made it possible

`settings/model.rs` (new), `settings/live.rs::network_rows`, a fixture adapter,
`page.rs`'s panel, and a widened `network_row` signature.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **115 passed · 0 failed · 6 ignored** |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced rebuild) | ✅ 1 real, pre-existing |
| `scripts/sweep-gallery.sh` | ✅ every state rendered |
| `*fixtures.rs` | ✅ **0 deleted lines** — purely additive |
| `rust/` | ✅ untouched |

### The seam this cut had to invent

Web's 024 dropped `live.ts` in beside an existing `model.ts`. The desktop had **no
model layer at all** — `page.rs` read `settings_fixtures::network(id)` and rendered
its fields directly — so there was nothing for a live builder to plug into. Phase 3's
real work was introducing one, narrowly: `NetworkRowModel`, plus a builder on each
side of the seam producing it.

`fixtures.rs` gained an *adapter*, not a fixture. Every value still comes from the
constants above it, and a test asserts the adapter reproduces what `page.rs` drew
before the seam existed, field by field — including the badge rule (a badge for every
non-custom row, none for a custom one). Without that test, "the gallery is unchanged"
would be an eyeball claim.

### Two things the core has no business knowing

A row needs a lettermark and a brand tint; the core knows neither, and should not.

- **The tint is read out of `fixtures::NETWORKS` by chain id**, not duplicated. Those
  colours are design data and the fixture file is where the design lives, so a live
  Ethereum row is tinted by the same constant the mock is. Two colour tables is how
  two renderings of one network start disagreeing.
- **A chain the mocks never drew gets a neutral**, not a generated colour. Deriving a
  hue from the chain id produces a brand-looking colour nobody chose for a network
  nobody designed.

And one thing the core *does* know, which the row must not flatten wrongly:
`Checking`, `Error` and "never probed" all draw **no badge**, and none of them is a
latency of zero. A test pins all three.

### `VELA_SECTION`, and why a new env pin was justified

The live surfaces are reachable only by clicking, so no screenshot pass could ever
see one — exactly the gap `VELA_SETTINGS_STATE` was added for, whose own comment says
it exists because "a left-alignment bug survived review on seven panels it also
broke". `VELA_SECTION=settings|contacts|explore` starts the **signed-in** page on a
section, and `VELA_SETTINGS_STATE` now applies on that path too.

`VELA_PAGE=settings` is deliberately *not* the same thing and must not become it: that
route has no session behind it and renders the mocks on purpose.

### Verified in the real app, by log rather than by picture

```
[vela-wallet] wallet: locale `en`, gallery false, section Settings
[vela-wallet] core: network_admin booting
```

The resident boots **only** when the live panel renders — with the wallet section
showing, or on the fixture route, the count is zero, which is the laziness and the
identity gate both working. (A screen capture was attempted first and proved nothing:
it grabs whatever window is frontmost, not the app. The log line is the honest
signal, which is what `Machine::LABEL` now earns its keep for.)

### A false alarm worth writing down

The first signed-in run landed on **onboarding**, not the wallet, and the seeded
account looked ignored. The cause was my seed, not the code: I wrote the account in
camelCase, and `Account` is plain snake_case serde on **both** clients — web's
generated `Account.ts` says `public_key_hex`, `created_at_iso` too. Accounts are
cross-client compatible exactly as they stand. Recorded because "the desktop stores a
different shape" was a plausible-sounding conclusion that would have been wrong.

## Phase 4 — `contacts`, and the measurement the cut exists to produce

| Gate | Result |
|---|---|
| `cargo test` | ✅ **124 passed · 0 failed · 6 ignored** (030 opened at 93) |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced rebuild) | ✅ 1, pre-existing |
| `scripts/sweep-gallery.sh` | ✅ every state rendered |
| live path | ✅ `section Contacts` → `core: contacts booting` |

### SC-004

```
core_host.rs         untouched
resident.rs          untouched
executor/proxy.rs    untouched
main.rs              untouched
executor/mod.rs      +1        pub mod contacts;
executor/storage.rs  +3        three key constants
```

**Zero lines of shared logic.** No function in the plumbing was added, changed, or
called differently; the road carried the third machine as built.

The literal draft criterion — "zero lines" across all six — is **not** met, and
saying it was would be the kind of quiet mis-scoring this program has already had to
correct once. Rust cannot satisfy it: a module must be declared to exist, so
`pub mod contacts;` is the irreducible cost of adding a file. The three other lines
are key constants in the cross-client contract registry `storage.rs` deliberately
centralises — and scattering the keys to their machines purely to win the number
would have made the contract harder to audit for a better-looking diffstat. SC-004 is
amended in the spec to the thing it was always measuring, with the reason in place.

The rest of the phase is where a third machine *should* cost something:
`executor/contacts.rs` (new), `contacts/{model,live}.rs` (new), a fixture adapter,
27 lines in `page.rs`, and 16 in `contacts/components.rs` — a signature widened from
`&'static str` to `SharedString`, which is what happens when a fixture-shaped API
first meets runtime data. `settings/components.rs` needed the same in phase 3. That
is a real, repeatable cost of going live, and it is not plumbing.

### What the live builder refuses to do

The core sorts the book favourites-first, then most-recent, and merges
history-derived suggestions under tombstone suppression. `contacts/live.rs` **groups**
those rows into A–Z sections and does not re-sort them — a test builds a Z, Z, A
roster and asserts two sections in that order with `Zoe` before `Zack`. Sorting there
would silently override a product rule the core owns and tests.

It adds exactly two things the core declines to decide: which of three names a row
shows (own label → resolved identity → shortened address), and which letter it files
under (`#` for anything not an ASCII letter).

### Four operations answer unknown, and one of them matters more than it looks

`load_send_history` → empty (no local tx store until 032). `resolve_identity` and
`classify_recipient` → `None` (both need 031's pool).

`classify_recipient` is the one to be careful with. `None` means **unknown**, not
"this address is not a contract" — the core says so and never caches it. A shell that
guessed would put a risk badge nobody measured onto a send screen. There is a test
whose only job is that the answer is `None` and carries the chain id it was asked
about.

### The dismissed store is an object, and that is load-bearing

`vela.contacts.dismissed` is `address → epoch ms`, not a list. A tombstone suppresses
a suggestion **unless the person transacted since the deletion**, so the timestamp is
the whole mechanism. Written as a list it would still round-trip through this file
and quietly stop working — which is why the test asserts `raw.is_object()` rather
than just round-tripping the values.

## Phase 5 — the first write path, end to end

Deleting a contact now dispatches `Event::Delete` and the change reaches the disk.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **125 passed · 0 failed · 6 ignored** |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced) | ✅ 1, pre-existing |
| `scripts/sweep-gallery.sh` | ✅ every state rendered (36) |
| real-network probes | ✅ `chainId 100` · `getCode 172 bytes` · `P256 → 0x…0001` |

The test that matters is `a_deleted_contact_stays_deleted_across_a_relaunch`: an event
mutates the core's ledger, the core asks for a write, the executor persists it, and a
**fresh core over the same directory** agrees. A shell that acknowledged the write
without performing it would pass every in-memory assertion and lose the change on the
next launch — which is the failure a round-trip inside one process cannot see.

Two guards on the delete itself:
- It is `None`-gated on a real session, so the **fixture panel cannot mutate a real
  ledger**. A picture must not delete anything.
- The address comes from the **core's own roster** by row position, not from
  `CONTACTS[index]`. Deleting "whatever the mock had at that index" is a bug that
  would only appear once the two lists diverged.

`executor::now_ms` is new and public: a mutation carries the time the *shell*
observed, which is what keeps the core a pure function of its inputs — the same
argument `now_iso` already makes one function above it.

**A false alarm, recorded.** The sweep failed once with "could not read the state
count", which looks like a rendering regression and is not: at `DWELL=1` immediately
after a rebuild the app had not printed its startup line before the script read the
log. At the default dwell it is green, and the gallery prints `36 states` on demand.

## Status — what is live, and what is not

Three machines are wired and driving real screens. **030 is not finished**, and the
remainder is interaction surfaces rather than plumbing:

| Surface | State |
|---|---|
| 设置 → 网络 list | ✅ live (core's rows, probes, health) |
| 设置 → 本地化 → 货币 | ✅ live (committed code, degraded when unpriced) |
| 通讯录 roster | ✅ live (core's book, A–Z grouped) |
| 通讯录 delete | ✅ live, persists across relaunch |
| 通讯录 add / edit / favourite / groups | ⬜ not wired |
| 添加网络 wizard (search → probe → add) | ⬜ not wired |
| RPC override edit + chain-mismatch refusal | ⬜ not wired |
| 端点 / 服务商 panels | ⬜ not wired |

### SC verdicts so far

| SC | Verdict |
|---|---|
| SC-001 network added survives relaunch | ⬜ **not yet** — probes verified live against Gnosis, but the wizard that would add one is not wired |
| SC-002 contacts CRUD survives relaunch | ◐ **partial** — delete proven end to end; add/edit/groups pending |
| SC-003 currency survives, degrades honestly | ✅ |
| SC-004 the paved-road measurement | ✅ as amended (zero shared logic; 4 declaration lines) |
| SC-005 galleries unchanged, fixtures additive | ✅ 0 deleted lines across every `fixtures.rs` |
| SC-006 tests up, fmt and CI green | ✅ 93 → 125 |
| SC-007 zero `rust/`, zero corpus delta | ✅ |

## Phase 6 — SC-001, against the real chain index and real endpoints

`AddByChainIdRequested` is the same pipeline the wizard's Add button runs — resolve
the chain, race its RPCs, probe the P-256 precompile, check eleven contract
deployments, gate on the core's verdict, dedup, persist — with only the UI's
confirmation step removed. Driving it fully against the network:

```
[op] ReadStore
[op] FetchChainInfo { chain_id: 7777777 }
[op] ProbeRpc { url: "https://zora.drpc.org" }
[op] ProbeRpc { url: "https://rpc.zora.energy/" }
[op] RpcCallP256 { url: "https://zora.drpc.org" }
[op] RpcGetCode × 11  (EntryPoint, Multicall3, the Safe set …)
  networks: 12 -> 13
  added: Zora (https://zora.drpc.org)      ← the winner of the core's RPC race
  survived the relaunch
```

**SC-001 is met.** A real chain, resolved from the real index, probed against real
endpoints, admitted by the core's own compatibility rules, persisted, and still there
after a fresh core reads the same directory.

### Two failures on the way, and only one was the code's

**The first was the core being right and my test being wrong.** I picked chain 100.
The count stayed at 12 and the row the test found turned out to be the **built-in**
Gnosis: `AddByChainIdRequested` had hit the dedup gate (invariant ①), which refuses to
add a chain the wallet already ships. Switched to Zora, which is real and is not a
default — and added an assertion that an added chain arrives `is_custom`, so the same
mistake cannot pass silently next time.

**The second was a real bug, and a process failure of mine.** The served
`/chains/eip155-*.json` is the community chain-list format — `chainId`,
`nativeCurrency` nested, `explorers` as objects carrying a `url`. `NetRawChainData` is
flat, and the core's doc assigns the flattening to the shell. I had written
`decode_raw_chain_data` and `decode_search_index` correctly **and then failed to wire
them**: `cargo fmt` had reformatted those two call sites, my string replacement
matched nothing, and — unlike every other substitution in this cut — **I did not
assert that it had applied**. The compiler said so, in a "never used" warning I had
filtered out of my own grep.

So for two runs the executor was still deserializing the raw document straight into
`NetRawChainData`, which does not fail: it silently yields defaults, and the core
correctly reported `NotFound`. Every unit test passed throughout. Only the live test
could see it, which is the argument for having one.

### Diagnostics kept on purpose

The live driver prints each operation as the core asks for it. That trace is what
turned "it did not add" into "it never got past `FetchChainInfo`" in one line, and it
is the only readable record of a live run. Production `get_json` and `json_rpc` are
quiet again: a failed probe is expected, and the core owns what it means.

## Phase 7 — the refusal, and closeout

### The invariant-④ refusal, against real endpoints

Ethereum's RPC field pointed at a **healthy** Gnosis endpoint — healthy is the point:

```
refused: card is chain 1, endpoint reported chain 100
nothing was written
```

A shell that saved on blur and let the core "fix it later" would have written an
override that silently breaks every balance read on that network, and **nothing about
the screen would look wrong**. The refusal is the whole feature.

### Final gate

```
cargo test    125 passed · 0 failed · 8 ignored      (030 opened at 93)
fmt           clean
warnings      1, pre-existing (BLE_CHANNEL_SUPPORTED)
gallery       every state rendered (36)
fixtures      0 deleted lines in 030 — 128 insertions, purely additive
rust/         0 files changed
corpus        0 files changed
```

*(The 20 fixture deletions visible against `origin/main` are 029's rustfmt reflow of
the previously-uncompiled explore/signing files, documented there. 030 deletes
nothing.)*

### SC verdicts

| SC | Verdict |
|---|---|
| **SC-001** network added against a real endpoint survives a relaunch | ✅ Zora added live (12→13) via the real index and a real RPC race; survives a fresh core. Plus the chain-mismatch refusal, proven against a real wrong-chain endpoint with nothing written. |
| **SC-002** contacts CRUD survives a relaunch | ◐ **partial** — delete proven end to end through a fresh core. Add / edit / groups are **blocked on drawn UI that does not exist**, see below. |
| **SC-003** currency survives, degrades honestly | ✅ |
| **SC-004** the paved-road measurement | ✅ as amended — zero shared *logic*; 4 declaration lines |
| **SC-005** galleries unchanged, fixtures additive | ✅ 0 deletions in 030 |
| **SC-006** tests up, fmt and CI green | ✅ 93 → 125 |
| **SC-007** zero `rust/`, zero corpus delta | ✅ |

### Why SC-002 stops where it does — a design gap, not unfinished wiring

Three surfaces cannot be wired without drawing something first, and inventing the
drawing would be designing rather than porting:

1. **The context menus are pictures.** `menu_card` renders `MenuItemModel`s that carry
   an icon, a label and a `destructive` flag — and **no action**. Wiring 删除分组 or
   重命名分组 means giving menu items a click surface, which changes a component every
   gallery board renders.
2. **There is no add/edit form sheet on desktop.** 018's boards never drew one; web's
   024 hit the identical gap and recorded it. Composing one from existing primitives
   is possible and is a *design* decision.
3. **There is no favourite control at all** in the desktop mocks. `ToggleFavorite`
   therefore has no affordance to hang off — the same shape as `session.rs`'s note
   that `SwitchAccount` "is still absent: an event with no control is dead code."

The core is complete for all three; the desktop is missing the picture. That is the
honest boundary between spec 030 and whatever draws them.

### Also not wired, and merely unfinished rather than blocked

The wizard's **search field** (the pipeline behind it is proven end to end by the live
add), the **RPC override field** (the refusal behind it is proven), and the
**endpoints / providers** panels. Each needs a focused editable text field in gpui —
`ui::text_field` exists and `settings/components.rs` grew an editable `url_field`
variant, so this is bounded work, not a gap.

### Carried debts → 031

1. `contacts::resolve_identity`, `contacts::classify_recipient`,
   `display_currency::resolve_rate` and `read_device_currency` all answer the core's
   modelled *unknown* and carry `// live in 031` markers. `read_device_currency` also
   needs a region→ISO-4217 table.
2. `network_admin::invalidate_pools` is an acknowledged no-op until a pool exists;
   `clear_bundler_cache` until 032.
3. `contacts::load_send_history` is honestly empty until 032's transaction store.
4. **`rpc.gnosischain.com` refuses this HTTP client with 403** while curl gets 200 —
   measured through a bare `ureq::Agent` and through `proxy::agent`, against all three
   Gnosis endpoints. `registry.rs:362` pins it **first** for onboarding's legacy-name
   lookup, so every such lookup burns a request on an endpoint that will refuse it.
   Not 030's to fix; 031 owns the pool and should not inherit the assumption.
5. `VELA_SECTION` is new and undocumented outside this file.
