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
