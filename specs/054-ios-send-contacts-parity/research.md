# Research — 054 iOS Send & Contacts Parity

---

## D0 — No Rust, for the third cut running

`SendCore`, `BatchImportCore` and `ContactsCore` are all `bridge_object!`
-exported and already in the committed `vela_core_uniffi.swift`. `contacts_io`
is not a machine at all — 503 lines of pure helpers that `contacts.rs` calls,
so its behaviour reaches iOS through `contacts` events and needs no export of
its own.

`build-ios-xcframework.sh` is not on this cut's critical path. A bindings diff
at closeout is a signal.

## D1 — One document layer, three verbs

`Features/Documents/DocumentPorts.swift`, a protocol with three methods and one
UIKit implementation:

```swift
protocol DocumentPorts {
    /// Let somebody choose a file. `nil` is a cancel, which is not a failure.
    func pick(types: [UTType]) async -> PickedDocument?
    /// Write bytes somewhere they choose to keep them.
    func create(name: String, type: UTType, bytes: Data) async -> Bool
    /// Hand the bytes to whatever they want to do with them.
    func share(name: String, type: UTType, bytes: Data) async -> Bool
}
```

**Three verbs because iOS has three different sheets**, and they mean different
things to a person: `UIDocumentPickerViewController(forOpeningContentTypes:)`
reads, the same class `forExporting:` saves into Files, and
`UIActivityViewController` sends. Collapsing save and share would put "AirDrop
to a colleague" and "keep a copy" behind one button.

Presentation goes through the foreground active window scene — the resolution
`PasskeyExecutor.presentationAnchor` already performs, reused rather than
rewritten. A delegate bridges to a continuation; **cancel resumes with
`nil`/`false`**, never leaves the continuation hanging (051's lesson about
`RpcPool`, applied to UIKit).

`UTType` for XLSX is `UTType("org.openxmlformats.spreadsheetml.sheet")` with
`UTType(filenameExtension: "xlsx")` behind it — the identifier is declared by
the system when an app that handles it is installed, and is not guaranteed.

## D2 — XLSX with no dependency, ~150 lines

A workbook is a zip of XML. Read it through the **central directory**, not the
local headers:

1. find the end-of-central-directory signature `0x06054b50` from the tail;
2. walk `0x02014b50` entries for each name, method, sizes and local offset;
3. at each `0x04034b50` skip the name and extra fields and read the compressed
   bytes.

**The local header's sizes cannot be trusted**: writers routinely set the
data-descriptor flag (bit 3) and leave the local sizes zero. The central
directory always has them.

Inflate with `(data as NSData).decompressed(using: .zlib)` — Apple's `.zlib`
is raw DEFLATE (RFC 1951), which is exactly what a zip stores. Method 0 is
taken as-is.

No CRC check (parsing the XML is the integrity check) and no Zip64 (a payroll
sheet is not four gigabytes). Both written down in the file rather than
silently absent.

The fixture is real: `app-desktop/vela-wallet/tests/fixtures/payroll-sample.xlsx`
— five deflate entries, inline strings, no `sharedStrings`, and a deliberate
row with an address and **no amount**, which is the gap case the matrix must
preserve rather than drop.

**Why not "CSV only".** Saving 150 lines by refusing a format the other three
clients read would mean a person who exported from Excel gets a file this
wallet alone cannot open — on the one platform where they are least likely to
have another copy of the wallet to hand.

## D3 — Draw before wiring (founder ruling, 2026-09-14)

C7 (add, empty), C8 (edit, filled), C9 (detail with the star lit) are built as
gallery states from the **018 vocabulary only** — `addTitle`, `editTitle`,
`nameLabel`, `namePlaceholder`, `addressLabel`, `addressPlaceholder`, `save`,
`cancel`, `invalidAddress`, `delete`, `deleteTitle`, `deleteBody`,
`sectionFavorites` — and from components that already exist. Zero corpus
change: every word is already there in fourteen languages.

They go through the screenshot sweep before a single event is dispatched.

## D4 — Split rows: one whole-list event, three pure helpers

`RecipientsChanged { recipients }` carries the **whole list**. Three pure
functions rebuild it:

| | |
|---|---|
| `amountEdited(rows, id, amount)` | guarded on `!=`, so an untouched row is the **same value** |
| `addressEdited(rows, id, address)` | **nulls the name** — it belonged to the old address |
| `removed(rows, id)` / `appended(rows)` | append with an **empty id**; the core mints `rcpt_{n}` |

Seeding — from a group, from a batch — goes through `SeedSplitRecipients`.

## D5 — The sweep's picking flag is the shell's, and nothing else is

The core's `multi_select_mode` flips only at **confirm**, so "are the tick
boxes showing" cannot come from it. One `@State` flag owns that; every other
question is the core's.

The tap rules, ported verbatim:

```
selected && only one selected  → ToggleMultiToken, then SetMultiNetwork(nil)
selected                       → ToggleMultiToken
nothing pinned                 → SetMultiNetwork(chain), then ToggleMultiToken
pinned ≠ this row's chain      → nothing at all
otherwise                      → ToggleMultiToken
```

The first arm is the one worth naming: **unticking the last ticked token
releases the chain**, so the next tick can pin a different one. Without it a
person who mis-picks is stuck on that chain until they leave the screen.

Dimmed rows are **not tappable**. A tappable row is an invitation the wallet
will not honour.

## D6 — The batch executor's three arms

| operation | answer | cannot find out |
|---|---|---|
| `fetch_usd_fiat_rate { code }` | `rate_resolved { code, rate }` | `rate_resolved { rate: null }` |
| `pick_file` | `file_picked { name, content }` · `file_pick_cancelled` | `file_pick_failed` |
| `save_template_file { name, contents, mime }` | `template_saved` | `template_save_failed` |

Three rules that are not obvious:

1. **The rate is `nil` unless it is finite and above zero.** The display
   currency's `?? 1` is a presentation fallback and would arrive here as "the
   rate really is 1" — which is the exact thing the core's guard exists to
   refuse.
2. **The file's extension decides text or matrix**, not the MIME the system
   guessed. A CSV exported from Excel is routinely typed
   `application/vnd.ms-excel`.
3. **A missing document layer is a failure, not a cancel.** A cancel means a
   person changed their mind; if the ports were never attached, nobody was
   asked anything.

## D7 — The book travels, and the file is released either way

Export: `ExportRequested { scope, format, exported_at_iso }` → wait for the
view's `export` → hand it to the share sheet → **`ExportTaken` regardless**.
The share sheet reports only that it was presented; leaving the file pinned on
the view because a person tapped Cancel would strand the next export.

Import: pick → `ImportFile { content, filename, into_group, now_ms }` → the
core parses (JSON or CSV, **existing wins**) → `last_import` or
`import_failure` → shown → `ImportAcknowledged`.

**An imported group does not gain contacts that already existed.** The core
creates a group only for the members it newly added. Android found this on a
device and recorded it as the core's rule; it will look like a bug to whoever
meets it next, so it is in the spec's edge cases too.

## D8 — Numbers, and the drift gate's blind spot

`tx_count` is `u32` → `Int`; `last_used_ms` and `first_seen_ms` are `f64` →
`Double`. `ts-rs` writes both as TypeScript `number`, so a generated-types
check cannot see the difference — **serde can**: a `0.0` sent for a `u32` fails
with *"invalid type: floating point `0.0`, expected u32"*. Read the Rust
struct, not the TS mirror.

## D9 — Two device lessons that are about driving, not about the product

- **The confirm CTA is a button, not a slider.** A swipe along it registers as
  a tap and the test proves nothing. (053 met the mirror image: a
  slide-to-confirm that must be dragged.)
- **A freshly saved contact must not inherit the fixture's activity rows.**
  Android showed a brand-new contact "+50 USDC received yesterday" for a
  payment that never happened, because the fallback model's rows were left
  alone. 052 fixed the same class of thing in `ContactsLive.detail`.

## D10 — Typing is echoed locally

A text field bound straight to a machine drops characters on the round trip —
found on a device in 043 and again in 045. The field owns what is on screen;
the value still goes to the core, and a core value the field did not send
replaces it.
