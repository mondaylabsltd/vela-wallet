# iOS event parity — after Phase 2 (P2.1–P2.6)

`node app-ios/scripts/check-ios-event-parity.mjs`, measured against
`app-web/vela-wallet/src/lib/core/generated` at 8a71f5f46.

Before Phase 2: **19** web-only (strong) variants. After: **6**, none of them a
P2 item:

| Machine | Web-only (strong) | Why iOS does not send it |
| --- | --- | --- |
| DpermEvent | popup_request, popup_approved, popup_account_switch | The extension's popup window; iOS has the in-app browser and no extension target yet (AUDIT §3: the Safari Web Extension is Phase 3). |
| ContactEvent | use_shared_store, store_changed | Browser tabs sharing one store (set-1). The core's own doc: "a shell with a single copy (iOS, Android) never sends it". |
| SendEvent | send_queued | The web's cross-tab send lock. One copy of the app has no lock to queue behind. |

Closed by Phase 2 (13): Send `add_network_tapped`, `submit_slow`,
`chain_answered`, `send_again`; Balance `account_noted`, `holdings_restored`,
`space_changed`; Contact `history_changed`; Net `only_mine_changed`,
`dapp_add_approved_unverified`; Guard `leg_grant_deliberately_chosen`; Sign
`landing_closed`; Track `submit_facts`.

The wire ruler (`check-ios-wire-parity.mjs --strict`) is unchanged and ok:
0 unanswered ops, 0 required fields missing (`wire-parity-after.md`).

## The table

| Machine | Variants | Web dispatches | iOS dispatches | Web-only (strong) | iOS-only |
| --- | --- | --- | --- | --- | --- |
| DpermEvent | 3 | 3 | 0 | popup_request, popup_approved, popup_account_switch | — |
| ContactEvent | 19 | 16 | 14 | use_shared_store, store_changed | — |
| SendEvent | 53 | 47 | 52 | send_queued | refresh_tokens, open_batch_import, close_batch_import, edit_amount, signing_started, cancel_signing |

All other machines: no web-only variant (`strong diffs: 6 across 27 machines`).
