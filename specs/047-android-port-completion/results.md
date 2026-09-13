# Results — 047 Android Port Completion and First-Run Parity

**Branch**: `047-android-port-completion` (stacked on 046) · **Device**: Xiaomi
alioth `9d5f42fb`, parallel space, Gnosis, Safe `0x88cC…6894`.

## What landed, phase by phase

| Phase | Delivered | Device evidence |
| --- | --- | --- |
| 0 | `Preferences` (the web's keys), `Formats` (locale-format.ts ported; every live figure, day header and detail date draws through it), the theme's text scale, the store's `allKeys` | — |
| 1 settings | Language/number/date/time/text-scale/avatar sheets and controls live; the accounts sheet from the session; `DeviceStorage` (the web's key→row mapping, bytes and records, clear per item, clear caches, erase = verified sweep with 028's keep-list); the relayer panel from the treasury probe; the RPC banner from the pool; About and Feedback from the build (`GIT_COMMIT` via a config-cache-safe provider) and the log's recent failures; the founder's ruling applied: no 通讯录 / 反馈 rows on the settings home | SC-001: English ↔ 跟随系统 switched the whole app; the storage page read 14 KB · 34 条 with per-item counts (交易记录 21 条 · 11 KB, 联系人与分组 7 位 …); text scale xlarge photographed. SC-002 and the number-format figure: see below |
| 2 share card | `ShareCardCapture` (graphics layer → PNG), `FlowLive.shareCard`, the receive sheet's 保存图片 through the documents port | SC-003: the share sheet opened with `vela-receive-266894.png` (86,196 bytes in `cache/shared`) |
| 3 deep links | `PayLink` (scheme + two hosts, `open?url=`), manifest filters + `singleTop` + `onNewIntent`, `WalletController.validatePayLink` (`LinkOpened` → `pay_valid`/`pay`), the send opened locked from `pendingSendParams` | SC-004: see below |
| 4 stability | `ConnectivityWatch` + the hero's offline line + refresh on reconnect; `CrashReport` (written first, rethrown; core faults via `VelaLog.onFault`) + `CrashSheet` on the next launch; `vela.testPanic` debug hook | SC-005/006: see below |
| 5 rulers, papers | `scripts/check-android-event-parity.mjs`, `scripts/check-android-dropped-judgement.mjs`; `docs/android/install-verify-loop.md`; `docs/KNOWN-BUGS.md` ANDROID-1..3; takeover 02 pointer | the two tables below |

## The rulers

### Web as the checklist (event variants the web dispatches that Android does not)

| Machine | Variants | Web dispatches | Android dispatches | Web-only (strong) | Android-only |
| --- | --- | --- | --- | --- | --- |
| BalanceEvent | 10 | 10 | 7 | fix_chain_resolved, switcher_opened, switcher_closed | — |
| FeeEvent | 6 | 5 | 2 | select_fee_asset, requote, leave_confirm | — |
| DpermEvent | 10 | 8 | 9 | popup_request | consent_rejected, revoke_requested |
| NetEvent | 20 | 19 | 18 | provider_test_requested | — |
| BatchImportEvent | 9 | 7 | 9 | — | set_fiat_code, apply |
| BhistEvent | 4 | 1 | 3 | — | visit_recorded, clear_all |
| ClearSigningEvent | 4 | 3 | 4 | — | cleared |
| ContactEvent | 17 | 12 | 15 | — | toggle_favorite, add_group_members, remove_group_member |
| CurrencyEvent | 2 | 2 | 2 | — | — |
| DpermPageEvent | 3 | 2 | 3 | — | disconnect |
| DsessEvent | 14 | 0 | 0 | — | — |
| ExploreEvent | 15 | 1 | 11 | — | favorite_added, favorite_removed, group_created, group_deleted, group_hidden_set, system_group_hidden_set, tab_opened, tab_navigated, tab_selected, tab_closed |
| ExtCacheEvent | 4 | 1 | 1 | — | — |
| FeedEvent | 7 | 7 | 7 | — | — |
| GuardEvent | 10 | 3 | 5 | — | grant_deliberately_chosen, revoke_chosen |
| MtokEvent | 5 | 5 | 5 | — | — |
| PaymentRequestEvent | 6 | 2 | 6 | — | mode_changed, asset_picked, amount_changed, acknowledge |
| ReceiveWatchEvent | 1 | 1 | 1 | — | — |
| RpcEvent | 6 | 6 | 6 | — | — |
| SendEvent | 39 | 26 | 37 | — | display_changed, refresh_tokens, toggle_fiat_input, open_batch_import, close_batch_import, edit_amount, fee_updated, fee_busy_changed, signing_started, cancel_signing, retry_after_error |
| SessionEvent | 6 | 6 | 6 | — | — |
| SignEvent | 12 | 6 | 9 | — | dismiss_tapped, swipe_dismissed, funding_cancelled |
| TrackEvent | 5 | 3 | 3 | — | — |
| TrustEvent | 7 | 7 | 7 | — | — |

strong diffs: 8 across 24 machines

The eight strong diffs, each with its reason:
- `BalanceEvent.fix_chain_resolved / switcher_opened / switcher_closed` — the web's RPC-fix flow and account switcher over the home; on the phone the fix lives in settings (RpcFix sheet) and the switcher is the settings' accounts sheet (047) — different surfaces, same machine facts.
- `FeeEvent.select_fee_asset / requote / leave_confirm` — the phone drives the fee session through the send machine's own events (`ChooseFeeToken`, `FeeUpdated`, the stale re-quote of 045); the fee machine's direct events are the web's shape of the same thing.
- `DpermEvent.popup_request` — the extension popup's path; no popup on the phone.
- `NetEvent.provider_test_requested` — the provider-key test button: not drawn on the phone (owed to a later settings pass).

### Dropped judgement (view fields the core computes that Android never reads)

| View | Fields | Not on the Kotlin wire | Carried but never read |
| --- | --- | --- | --- |
| BalanceView | 8 | — | banner_chain_ids |
| BatchView | 11 | — | file_error, priced, total_token, total_fiat, applied |
| BhistView | 1 | — | — |
| ClearMessageView | 5 | — | is_hex |
| ClearSigningView | 4 | — | danger_haptic |
| ContactRecipientView | 6 | — | verified, identity |
| ContactsView | 4 | — | — |
| CurrencyView | 2 | — | — |
| DpermConsentView | 1 | — | methods |
| DpermPopupView | 1 | — | — |
| DpermView | 2 | — | popup |
| ExploreGroupView | 1 | — | — |
| ExploreView | 4 | — | favorites_full, tabs_full |
| FeeOptionView | 2 | — | — |
| FeeView | 4 | — | — |
| FeedView | 4 | — | transactions, new_item_id, toast |
| GuardBatchView | 2 | — | any_to_own_token |
| GuardEditorView | 5 | — | — |
| GuardIncreaseTotalView | 2 | — | — |
| GuardView | 3 | — | — |
| MtokView | 4 | — | custom_tokens |
| NetEndpointView | 1 | — | — |
| NetWizardView | 1 | — | — |
| PaymentRequestView | 3 | — | qr_value, copy_payload, has_amount |
| ReceiveWatchView | 1 | — | deposits |
| RpcPoolView | 3 | — | — |
| SendReceiptView | 3 | — | submitted_at_ms |
| SendView | 13 | — | amount_locked, denom_toggle_shown, denom_toggle_enabled, denom_toggle_reason, split_over_balance |
| SessionSignOutView | 1 | pending_upload_warning | — |
| SessionView | 3 | sign_out | — |
| SignRequestView | 1 | — | — |
| SignView | 2 | — | reconcile_pending |
| TrackView | 1 | — | — |
| TrustIncomingView | 2 | — | — |
| TrustSimView | 1 | — | — |
| TrustView | 2 | — | — |

carried but never read: 30 fields across 48 views

Named residue: `BatchView.file_error/priced/total_token/total_fiat/applied` (the sheet says "no rows"/"rate unavailable" through other fields; totals not drawn), `SendView.denom_toggle_*` (the ⇄ is drawn from the fixture, its enabled/reason not shown), `FeedView.toast/new_item_id` (no toast on the phone), `PaymentRequestView.qr_value/copy_payload/has_amount` (the QR is drawn from `eip681_uri`), `BalanceView.banner_chain_ids` (the RPC banner reads the pool view instead), `ContactRecipientView.verified/identity` (the detail shows the contact's own name), `ClearMessageView.is_hex`, `ClearSigningView.danger_haptic` (no haptic on the sheet yet), `SessionSignOutView.pending_upload_warning` (the wire does not carry it; the sheet shows the warning from the view it has).

## Device pass and SC-002/004/005/006

Xiaomi `9d5f42fb`, parallel space, fixture Safe `0x88cC…6894`, the debug build of 2026-09-12 (508 unit tests green). Every row below was driven over adb (`ui.py`: uiautomator dumps, taps, screenshots) and read back from the dump or the screenshot.

| SC | What was done on the phone | Seen |
|---|---|---|
| SC-001 language / number / text scale | language switched in 设置, number preset changed, text scale raised (earlier in the phase) | the settings page re-rendered in the new language; `£0,40` on the asset rows; the page grew with the scale |
| SC-002 accounts switch | the header's name line (spec 047's switcher) → sheet「账户 · 2 个账户 · 总计 $5.65」with 觉得九点半 $5.10 and Parallel space $0.55 → tap the other → back again | the header, the address and the total switched both ways; `BalanceEvent.switcher_opened/closed` now dispatched |
| SC-003 share PNG | 收款 → 保存图片 → the share sheet (earlier) | an 86,196-byte PNG handed to the sheet |
| SC-004 pay link | `velawallet://pay?to=<safe>&chain=100&amount=0.001&sym=XDAI&dec=18&net=Gnosis` through `am start -a VIEW -d '…'` on a fresh process | 发送 XDAI opened with 0.001, the Safe as recipient, 网络费 0.01 xDAI; log `paylink validated chain=100 amount=0.001` |
| SC-005 offline | airplane mode on → HOME → relaunch (the focus-driven refresh) → off → relaunch | 离线 under the balance after three unreached calls (`net offline misses=3`); gone again after the first answered call (`net reachable again`), balances refreshed |
| SC-006 crash sheet | the debug panic hook → relaunch (earlier) | the failure sheet with the report |
| Erase device | NOT run on this phone: beside the fixture it holds the founder's own account and contacts; `DeviceStorageTest` covers the sweep and the verify | — |

The founder's notes of 2026-09-12, done and seen on the same phone:

- 设置 home without the 通讯录 and 反馈 rows.
- Logos as the web draws them: the home's XDAI row shows the Gnosis logo with the chain badge hidden (a native coin on its own chain); the receive list shows every chain's logo; the Send form's token card and fee mark show Gnosis. The drawn glyph is the whole fallback — with the endpoint down the phone looks as it did before.
- 首页钱包切换: the header's name line and chevron open the switcher (the web's header switcher of 028 phase 9), reusing the settings sheet body.
- 「点击探索没反应，或者很久才出现」: found by a subagent to be two navigation defects, not the explore feature — 探索 was only wired on the wallet route (dead on 通讯录/设置), and a second 钱包 tap during the 700 ms exit fade popped the wallet itself and left an empty NavHost (the blank app under the badge). Fixed by hoisting the section and popping to the wallet, never past it; 探索 now opens from 设置 and 通讯录 within a frame, and the double tap lands on the home.

## Deviations from the plan

- Offline is not the platform's connectivity callback (plan T015). The manifest's standing rule refuses `ACCESS_NETWORK_STATE` (a connectivity check lies behind captive portals and on VPNs), and without it the callback never fires. `NetHealth` answers from what the calls did: three consecutive `Network` outcomes in the RPC executor = offline, the first answered call = online again. FR-006 reworded.
- The pay-link effect consumed its trigger first and so cancelled itself (`LaunchedEffect(payLink)` re-keyed to null while waiting for the core's verdict). It now consumes the link when done, and the verdict read is the one this dispatch produced, not the model's last.
- The logo loader derives its client from `VelaHttp.client` (SC-107's `NoStrayHttpClientTest` refuses a second `OkHttpClient`), with a 16 MB disk cache under `cache/logos`.

## Gates

- Unit suite: 508 run, 0 failed (`:app:testDebugUnitTest` — includes `CoreWireDriftTest`, `NoStrayHttpClientTest`, `MarksTest`, `NetHealthTest`).
- Rulers re-run after the phase: strong diffs 8 → 6 across 24 machines (the balance switcher pair closed); 30 view fields still carried unread across 48 views — the tables above.
- wasm check and `.so` size: no Rust edits in 047, artefacts unchanged.
- Build: Kotlin-only via `-PvelaSkipRustBuild`; the debug APK installed and driven as above.
