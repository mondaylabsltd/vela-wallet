# Implementation Plan: Android Send Parity, Batch and Contacts Parity

**Branch**: `045-android-send-contacts-parity` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/045-android-send-contacts-parity/spec.md`

## Summary

Turn the send machine's two other modes live on the phone (split rows
through the single whole-list event; the sweep pick with the desktop's
first-tap-pins-the-chain rules), bridge `batch_import` and drive it with
the platform's document picker/creator, give the treasury pause its second
exit and the stale quote its refresh; draw the two contact controls the
phone never had (the add/edit form, the favourite star) from 018's
vocabulary, then wire them and the rest of the contacts machine the phone
does not drive yet — import/export through the picker and the share sheet,
groups from both sides, recipient inspection on open. Every phase on the
Xiaomi, in the parallel space.

## Technical Context

**Language/Version**: Kotlin 2.x / Compose (minSdk 29), Rust (one bridge
object added: `BatchImportCore`).
**Primary Dependencies**: 043's `SendController`/`SendLive`/`UserOpSpine`,
`FlowFixtures`' drawn SD1B/SD2B/SD2C/SD2D, `ContactsController`/
`ContactsLive`, `CoreHost`/`JsonShell`; AndroidX Activity result contracts
(`OpenDocument`, `CreateDocument`), the system share sheet
(`Intent.ACTION_SEND` with a `FileProvider` URI).
**Storage**: unchanged (`vela.contacts`, `vela.contactGroups`, the feed).
**Testing**: JUnit (host uniffi), `CoreWireDriftTest` (+ the batch family,
+ the contacts events the Kotlin wire lacks), the device loop.
**Performance/Constraints**: the batch parse and rate are the core's; the
bridge grows by one small machine (batch_import, ~1,200 lines); no Kotlin
judgement (FR-010).
**Scale/Scope**: ~20 new/changed Kotlin files, one new drawn surface
(contact form + star) with gallery states, two executors (batch, contacts
file I/O), seven contacts events added to the wire.

## Constitution Check

The program's standing rules (rules in the core; one implementation; drift
gate; device verification; no fixture on a live route; corpus words) hold.
Deviation recorded: the contact form is DRAWN in this feature (018 named
it, the phone never built it) — a drawing task inside a wiring spec, kept
to the gallery-first discipline (states in `ContactsFixtures`, then wired).

## Project Structure

### Documentation (this feature)

```text
specs/045-android-send-contacts-parity/
├── plan.md  research.md  data-model.md  quickstart.md  tasks.md
└── contracts/shell-operations.md
```

### Source Code (repository root)

```text
rust/crates/vela-core-uniffi/src/onboarding_bridge.rs   # + BatchImportCore

app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── feature/send/core/BatchWire.kt         # batch_import wire
├── feature/send/core/BatchExecutor.kt     # rate via Chainlink (the wallet's own port), pick/save through ports
├── feature/send/core/SendController.kt    # + split/sweep/batch intents, treasury dismiss, stale re-quote
├── feature/send/core/SplitRows.kt         # the three whole-list edits (pure, tested)
├── feature/send/SendLive.kt               # + pick multi (selection/dim/notice/cta), split form, sweep form, batch sheet
├── feature/flows/FlowHost.kt              # + callbacks for rows, ticks, batch sheet
├── feature/contacts/ContactsModels.kt     # + ContactFormModel, favourite control, inspection block
├── feature/contacts/ContactsFixtures.kt   # + C7 (add form) / C8 (edit form) / C9 (favourite) states
├── feature/contacts/components/ContactForm.kt   # drawn from 018's vocabulary
├── feature/contacts/core/ContactsWire.kt  # + ImportFile/ImportAcknowledged/ExportRequested/ExportTaken/AddGroupMembers/RemoveGroupMember/SetContactGroups, view fields, types
├── feature/contacts/core/ContactsController.kt  # + import/export/inspect/form intents
├── feature/contacts/core/DocumentPorts.kt # SAF picker/creator + share sheet, Activity-bound
├── feature/contacts/ContactsLive.kt       # + form model, inspection block, import report
└── navigation/VelaNavHost.kt              # wiring
```

**Structure Decision**: the send additions stay in `feature/send` beside
043's controller; document I/O is one Activity-bound port class shared by
the batch sheet and the contacts book (both pick and create files); the
contact form is a component in `feature/contacts/components` with fixture
states, per 018.

## Complexity Tracking

| Deviation | Why | Alternative rejected |
| --- | --- | --- |
| A drawn component inside a wiring spec | 040 found the form and the star undrawn; without them contacts cannot be added by hand | Waiting for a design pass would leave the book history-only for another spec |
