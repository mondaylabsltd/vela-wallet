# iOS wire parity

`node app-ios/scripts/check-ios-wire-parity.mjs --report`, measured against
`app-web/vela-wallet/src/lib/core/generated` (the ts-rs mirror of the core).

| Machine | Ops | Unanswered ops | Results | Never produced | Required fields missing | Optional fields missing | Payload fields missing |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Balance | 7 | — | 8 | — | fetch_settled.own_node_chain_ids, fetch_settled.fallback_chain_ids | — | — |
| Batch | 3 | — | 6 | template_saved | — | — | — |
| Bhist | 3 | — | 2 | — | — | — | — |
| Clear | 6 | rpc_get_code | 5 | — | — | — | — |
| Contact | 7 | — | 8 | write_failed, store_unreadable | — | — | — |
| Currency | 4 | — | 4 | — | — | — | — |
| Dbr | 13 | — | 4 | — | — | — | — |
| Dperm | 4 | (not an iOS machine) | 1 | | | | |
| Dsess | 17 | (not an iOS machine) | 11 | | | | |
| Explore | 2 | — | 2 | — | — | — | — |
| ExtCache | 5 | (not an iOS machine) | 5 | | | | |
| Fee | 9 | — | 9 | — | — | — | in_band_quotes.quotes.native_usd_floor_price (opt) |
| FeeSpeed | 0 | (not an iOS machine) | 0 | | | | |
| FeeTierPref | 2 | — | 2 | — | — | — | — |
| Feed | 8 | — | 9 | delete_failed | — | — | store_loaded.records.calldata (opt), store_loaded.records.token (opt) |
| FeedDapp | 4 | (not an iOS machine) | 0 | | | | |
| Guard | 3 | — | 3 | — | — | — | — |
| Intent | 0 | (not an iOS machine) | 0 | | | | |
| Mtok | 5 | — | 7 | saved, save_failed, remove_failed | — | — | — |
| Net | 18 | retire_provider_keys, write_shown_networks | 14 | provider_keys_retired | store_loaded.multisig_service | store_loaded.shown_networks | store_loaded.custom_networks.is_testnet (opt), store_loaded.network_configs.only_mine, store_loaded.endpoints.genesis_service_url (opt), store_loaded.shown_networks.version (opt), store_loaded.shown_networks.shown (opt), search_index.chains.vela, chain_info.data.slip44 (opt) |
| PaymentRequest | 2 | — | 2 | — | — | — | — |
| ReceiveWatch | 3 | — | 5 | — | — | — | — |
| Rpc | 7 | — | 7 | — | pool_config.own_only | — | — |
| Send | 24 | propose_safe_tx | 23 | proposed, propose_failed | — | — | risk_resolved.risk.history_unread, risk_resolved.risk.known_name (opt), risk_resolved.risk.own |
| Session | 12 | publish_pending_upload | 14 | pending_uploads_unavailable | — | — | — |
| Sign | 10 | — | 8 | — | — | — | pre_check.funding.deposit_address (opt), pre_check.funding.safe_address (opt), pre_check.funding.chain_id (opt), pre_check.funding.native_symbol (opt), pre_check.funding.threshold_wei (opt), pre_check.funding.recommended_wei (opt), pre_check.funding.current_balance_wei (opt) |
| SigningPages | 2 | — | 2 | — | — | — | — |
| Track | 10 | read_nonce | 14 | receipt, nonce_read | — | — | — |
| Trust | 8 | — | 8 | — | — | — | — |

unanswered ops: 6 · results never produced: 13 · required fields missing: 4 · optional fields missing: 1 · payload fields missing: 20 · unmapped machines: 0

## --strict: 14 failure(s)

- Balance: required field missing fetch_settled.own_node_chain_ids
- Balance: required field missing fetch_settled.fallback_chain_ids
- Clear: unanswered op rpc_get_code
- Net: unanswered op retire_provider_keys
- Net: unanswered op write_shown_networks
- Net: required field missing store_loaded.multisig_service
- Net: payload field missing store_loaded.network_configs.only_mine
- Net: payload field missing search_index.chains.vela
- Rpc: required field missing pool_config.own_only
- Send: unanswered op propose_safe_tx
- Send: payload field missing risk_resolved.risk.history_unread
- Send: payload field missing risk_resolved.risk.own
- Session: unanswered op publish_pending_upload
- Track: unanswered op read_nonce
