# Tasks: 090 — Receive code: opt-in network (ERC-681)

**Input**: [spec.md](spec.md), [plan.md](plan.md)

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [x] T001 Branch `090-receive-network-qr` from `origin/main`. Feature dir and `.specify/feature.json`.
- [x] T002 Record the owner rulings in `specs/086-issue-sweep/results.md`:
  - #312 → ERC-681 opt-in → this spec.
  - #333 → refusing GBK is kept.

## Phase 2: Foundational (core + corpus)

- [x] T003 [US1][US2] `payment_request.rs`: `IncludeNetworkChanged`, `include_network` (reset on
  `Start`), and the view fields `network_switch` / `include_network` / `network_hint`. `qr_value` switches
  in address mode; `copy_payload` stays the bare address.
- [x] T004 [US1][US2] `tests/app_payment_request.rs` covers:
  - default bare;
  - on → `@100`;
  - a token code names only its chain;
  - follows the pick, then off again;
  - `Start` resets the switch;
  - no switch in request mode;
  - no switch before a recipient;
  - scanner-grammar → send round trip (`request_chain_id == 100`).
- [x] T005 Corpus: `receive.includeNetwork` and `receive.includeNetworkHint` in 15 locales.
  - Bump the path pin to 1788 in `scripts/gen-i18n.mjs`.
  - Raise `SC005_BUDGET` from 138,800 to 139,800.
  - Run gen / lint / verify / dump:vectors.
- [x] T006 Regenerate the artefacts: wasm + `pkg-web`, TS mirrors, Swift bindings (unchanged: JSON
  bridge), xcframework, Kotlin bindings, dev fixtures.

## Phase 3: Shells (US1 + US2)

- [x] T007 [P] Desktop:
  - `NetworkSwitch` model and `FlowStrings`;
  - `switch_row`, drawn in `receive_qr`;
  - `PanelActions.include_network`;
  - `keep_receive_asset`, and forget the resident per visit.
  - Tests: live mapping with a real core and `eip681::parse`; the share card PNG decodes to the URI;
    fixtures; no-echo.
- [x] T008 [P] Web and extension:
  - keys, model, fixture;
  - `receiveSubject` / `receiveAssetPicked`; `liveReceiveQr` encodes `qr_value` on screen and in the
    share card;
  - `Switch.svelte` and the `ReceiveQr` prop, threaded through both hosts;
  - the route's session and asset sync.
  - Tests: node with a real wasm core, browser component, e2e at 390 and 1440 px.
- [x] T009 [P] iOS:
  - wire fields and `PaymentRequestStore.includeNetwork`;
  - `NetworkSwitchModel`, `VelaSwitchRow`, `FlowsLive.receiveCode`, `shareCard(pay:)`;
  - `enterReceive` as the per-visit start;
  - the share card follows the machine's asset.
  - Tests: Swift Testing with the real core; a UI test with screenshots.
- [x] T010 [P] Android:
  - wire event and fields, `WalletController.includeNetwork`, I18nKeys;
  - `ReceiveQrModel.code` / `network`, `FlowSwitchRow`;
  - `FlowLive.receiveQr` / `shareCard(code)`, threaded through FlowHost and VelaNavHost.
  - Tests: JUnit with the real core.

## Phase 4: Polish

- [x] T011 Run every suite: core, web, desktop, Android, iOS, and the CI scripts.
- [x] T012 Screenshots:
  - iOS simulator, zh, off and on;
  - web at 390 and 1440 px, off and on.
- [x] T013 `results.md`.
