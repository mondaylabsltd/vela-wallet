# iOS wire parity

`node app-ios/scripts/check-ios-wire-parity.mjs --report`, measured against
`app-web/vela-wallet/src/lib/core/generated` (the ts-rs mirror of the core).

| Machine | Ops | Unanswered ops | Results | Never produced | Required fields missing | Optional fields missing | Payload fields missing |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Balance | 7 | — | 8 | — | — | — | — |
| Batch | 3 | — | 6 | template_saved | — | — | — |
| Bhist | 3 | — | 2 | — | — | — | — |
| Clear | 6 | — | 5 | — | — | — | — |
| Contact | 7 | — | 8 | — | — | — | — |
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
| Net | 18 | — | 14 | — | — | — | store_loaded.shown_networks.version (opt), store_loaded.shown_networks.shown (opt) |
| PaymentRequest | 2 | — | 2 | — | — | — | — |
| ReceiveWatch | 3 | — | 5 | — | — | — | — |
| Rpc | 7 | — | 7 | — | — | — | — |
| Send | 24 | — | 23 | proposed | — | — | — |
| Session | 12 | — | 14 | pending_uploads_unavailable | — | — | — |
| Sign | 10 | — | 8 | — | — | — | pre_check.funding.deposit_address (opt), pre_check.funding.safe_address (opt), pre_check.funding.chain_id (opt), pre_check.funding.native_symbol (opt), pre_check.funding.threshold_wei (opt), pre_check.funding.recommended_wei (opt), pre_check.funding.current_balance_wei (opt) |
| SigningPages | 2 | — | 2 | — | — | — | — |
| Track | 10 | — | 14 | receipt | — | — | — |
| Trust | 8 | — | 8 | — | — | — | — |

unanswered ops: 0 · results never produced: 8 · required fields missing: 0 · optional fields missing: 0 · payload fields missing: 12 · unmapped machines: 0

## --strict: ok

