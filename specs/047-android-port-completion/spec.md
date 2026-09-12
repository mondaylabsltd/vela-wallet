# Feature Specification: Android Port Completion and First-Run Parity

**Feature Branch**: `047-android-port-completion` (stacked on `046-android-signing-depth`)
**Created**: 2026-09-12
**Status**: Draft
**Input**: The program doc's 047 (mirrors web 028 + 038): the residue, then
the rulers. Settings rows that still read fixtures (RPC health and latency
pills, storage figures, the relayer panel, language and format sheets, the
accounts sheet's rows, per-network RPC override selection); the receive share
card as a PNG through the share sheet; `/pay` and `velawallet://` deep links;
erase device; the first-run items 038 settled (launch gate, panic sheet,
offline and version line). Then the two rulers re-run — the "web as the
checklist" event sweep and the "dropped judgement" grep — and a full device
pass of 040–047, `docs/KNOWN-BUGS.md` and the takeover docs updated, the
install/verify loop written down.

## User Scenarios & Testing

### US1 — Settings that do what they say (P1)
Every settings row that today shows a fixture either does the thing or says
its unknown state. Language: the sheet switches the app's language and
follows the system when asked. Number, date and time formats: the presets
the web offers (comma/dot, dot/comma, space/comma, Indian; y/m/d, m/d/y,
d/m/y, d.m.y, ISO; 24h/12h; "auto" follows the locale) are kept and applied
to the home total, the feed's dates and the send's figures. Text scale: the
six levels apply. Accounts: the sheet lists this device's accounts, the tap
switches, "create" and "sign in" go where they say. Storage: the figures are
this device's (records and bytes per item, from the store's own keys), "clear"
per item and "clear all caches" remove exactly those keys. Erase device:
every key the store holds goes, verified by re-enumeration; on a partial wipe
the person stays signed in with the reason in the sheet. Relayer: the panel
shows the treasury probe's address and shortfall and retries. Feedback: the
preview lines are real (version, platform, language, failed chains) and
"send" opens the prefilled GitHub report. About: the version and commit are
the build's, the network count the wallet's. RPC health and latency pills
come from the settings machine's probes; the RPC banner names the pool's
failed chains and "restore defaults" resets the endpoints.
**Acceptance**: on the Xiaomi, language switches to English and back; a
number format switches the home total's separators; the storage page shows
the device's own counts; clear caches drops the balance cache; erase device
leaves the store empty and lands on first run; the accounts sheet switches.

### US2 — The receive share card as a PNG (P1)
"Save image" on the receive sheet renders the drawn share card (the
account's identicon, name, address, network note, wordmark) to a PNG and
hands it to the share sheet. **Acceptance**: the share sheet opens with a
PNG the size of the card; the file in the app's shared cache decodes as PNG.

### US3 — Deep links (P2)
`velawallet://pay?…` and `https://wallet.getvela.app/pay?…` open the app and
run the `/pay` grammar through the core's `payment_request` machine; a
valid request opens the Send locked on that chain, token and amount (as a
scan does); an invalid one is refused quietly. `velawallet://open?url=…`
opens the in-app browser. **Acceptance**: `adb shell am start -d
'velawallet://pay?to=<founder>&chain=100&amount=0.001'` lands on the locked
Send; a malformed link lands on home.

### US4 — Stable in an unstable environment (P2)
Offline: a line on the home says the device is offline and the pools are
retried when it comes back. Version: About and the feedback preview carry the
build's version and commit. Panic: an uncaught exception is written to the
store and, on the next launch, raised as the ordinary failure sheet
("Something went wrong", details, Report) rather than a silent restart; a
core fault reaches the same sheet. **Acceptance**: airplane mode shows the
offline line and clears on reconnect; a forced crash (debug hook) shows the
sheet on relaunch with Report opening the prefilled issue.

### US5 — The two rulers (P2)
The "web as the checklist" event sweep (per machine, the Event variants the
web dispatches minus those Android dispatches) reaches zero strong diffs or
names each remaining one with its reason; the "dropped judgement" grep (view
fields the core computes that Android's live builders never read) is run and
its residue named. **Acceptance**: both tables in results.md.

### US6 — The pass and the papers (P2)
A scripted device pass over 040–047 (home, receive, send single/split/sweep
pick, batch, contacts, explore + dApp signing, settings, scanner) with
screenshots; `docs/KNOWN-BUGS.md` gains the Android findings; the takeover
docs' Android section and a new `docs/android/install-verify-loop.md`
describe the install/verify loop the next person runs.

## Requirements
- FR-001 Preferences (language, number/date/time formats, text scale, avatar
  style) persist under the web's keys (`vela.language`, `vela.localePrefs`,
  `vela.avatarStyle`) and apply app-wide; no machine is invented (028's rule).
- FR-002 Storage figures come from the store's actual keys mapped to the
  web's item ids; erase sweeps every key (verified), the one keep-list being
  the passkey account records that sign-out already owns.
- FR-003 The relayer panel, the RPC banner and the health pills read the
  relay probe, the pool view and the settings machine's probes.
- FR-004 The share card PNG is rendered from the same composable the gallery
  draws (R4) and handed through the documents port.
- FR-005 Deep links are validated by the core (`LinkOpened` → `pay`), never by
  the shell; the manifest declares the scheme and the host.
- FR-006 Offline state comes from the platform's connectivity callback; the
  panic sheet from a default uncaught-exception handler that persists first
  and rethrows.
- FR-007 Every US verified on the Xiaomi.

## Success Criteria
SC-001 language + number format + storage + clear caches + erase on device;
SC-002 the accounts sheet switches; SC-003 share PNG in the share sheet;
SC-004 the pay deep link locks the Send; SC-005 offline line on airplane
mode; SC-006 the panic sheet after a forced crash; SC-007 the two tables;
SC-008 the pass with screenshots and the docs.

## Out of scope
Play signing/store assets, iOS, WalletPair, the settings pages' redesign.
