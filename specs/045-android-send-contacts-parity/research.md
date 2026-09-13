# Research — 045

## D1 — Split rows are one whole-list event, edited by three pure helpers
The core's `RecipientsChanged { recipients }` carries the whole list. Three
helpers (`splitAmountEdited`, `splitRowRemoved`, `splitRowAppended`)
rebuild it: untouched rows byte-identical, ids and names preserved, a new
row with an EMPTY id so the core mints its own (`rcpt_{n}`) — the desktop's
033 phase 3 rule and test. Seeding (a group, a batch) goes through
`SeedSplitRecipients`, as the web and the desktop do.

## D2 — The sweep pick's flag is the shell's; the rest is the core's
`multi_select_mode` flips only when the selection is confirmed, so "are the
tick boxes showing" lives in the shell (`sweepPicking`, as web and
desktop). First tick with `multi_chain_id == null` → `SetMultiNetwork` then
`ToggleMultiToken`; other chains' rows are drawn dimmed and not tappable
(the core would refuse; a tappable row is an invitation it will not
honour); "select all valuable" → `ToggleAllMultiTokens { visible_ids }`;
the CTA counts `multi_selected_ids`; Continue → `ConfirmMultiSelection`.
The sweep form (SD2D) reads `multi_specs` (exact amounts per token) and
the shared recipient.

## D3 — batch_import crosses the bridge; files through the platform
`BatchImportCore` (Event 10, Operation 3, ShellResult 6). `FetchUsdFiatRate`
→ the wallet's own Chainlink/fiat port (043's display-currency rate path;
the core prices); `PickFile` → `OpenDocument` (text or spreadsheet; xlsx
read as a matrix by a small reader, CSV/TXT as text) → `FilePicked{name,
content}` / cancelled / failed; `SaveTemplateFile{name, contents, mime}` →
`CreateDocument`. `Apply` → the send controller dispatches
`SeedSplitRecipients` with the core's `recipients` (address, amount, name),
as the web's wallet page does. The sheet is SD2C (drawn), modelled by the
web's `liveBatchImport` mapping.

## D4 — The treasury pause's exits and the stale quote
`DismissTreasurySheet` exists in the core and the phone never sent it
(043's confirm notice offers only the retry): "not now"
(`componentsUi.funding.cancel`) beside the core's retry, facts kept.
`FeeView.stale` → re-quote while the confirm page is open and idle (044
did the same for the signing sheet).

## D5 — The contact form and the star are drawn first
018's vocabulary: a sheet with `nameLabel`/`namePlaceholder`,
`addressLabel`/`addressPlaceholder`, `save`/`cancel`, `addTitle`/
`editTitle`, `invalidAddress`; the favourite control on the detail's
header (star icon, `sectionFavorites` semantics). Fixture states C7 (add,
empty), C8 (edit, filled), C9 (detail with the star on) in the gallery
before wiring. Validation is the core's: `Save{input, now_ms}` answers
through the view (`recipient`/`last_import`-style outcome — the save
outcome field), and the form shows the core's words.

## D6 — The contacts wire is behind the core; it grows
The Kotlin wire lacks `ImportFile`, `ImportAcknowledged`, `ExportRequested`,
`ExportTaken`, `AddGroupMembers`, `RemoveGroupMember`, `SetContactGroups`
and the view's `sections`, `import_failure`, `export`, plus
`ContactExportFile{filename, mime, content, contacts}`,
`ContactExportScope{All, Group{id}}`, `ContactFileFormat{Json, Csv}`,
`ContactImportFailure`. Added in phase 0, drift-gated.

## D7 — The book travels through the platform
Export: `ExportRequested{scope, format, exported_at_iso}` → the view's
`export` file → written to the app's cache → `ACTION_SEND` with a
`FileProvider` URI (the share sheet; "save to Files" is the scripted
choice) → `ExportTaken`. Import: `OpenDocument` → the file's text →
`ImportFile{content, filename, into_group, now_ms}` → the core parses
(JSON/CSV, existing wins) → `last_import` report / `import_failure` →
shown → `ImportAcknowledged`.

## D8 — Inspection on open
Opening a contact dispatches `InspectRecipient{chain_id, address}` (the
browser's chain, Gnosis by default, as the desktop uses its current
chain); the view's `recipient` carries `kind` (contract/wallet/…), `saved`,
`verified`, identity; the detail draws the tag with
`componentsUi.signing.contractTag` / `walletTag` and first-time with
`firstTimeTagNeutral` when the send history has no row for the address.

## D9 — Device harness
Dust split: founder + the Safe itself (two rows, 0.001 each). Sweep: the
pick's rules on the device; the sweep itself in a test with two scripted
tokens. Batch: `adb push two-rows.csv /sdcard/Download/` then the system
picker (DocumentsUI) driven by `uiautomator` (Downloads → the file). Share
sheet: the "保存到文件"/Files target. Contacts: the form through the
keyboard, the star, force-stop.
