# Data Model — 045

- **SendRecipientDraft** `{id, address, amount, name?}` — the whole-list
  payload; ids core-minted.
- **Multi selection** — `multi_select_mode`, `multi_selected_ids`,
  `multi_valuable_ids`, `multi_chain_id`, `multi_specs[]{token_address?,
  decimals, amount}`; the shell's `sweepPicking` flag.
- **BatchView** — `opened, unit, fiat_code, raw_text, file_name, busy,
  file_error, template_saved, priced, rate_status, rate_input,
  rate_edited, preview[]{line, name?, address, valid, dup, raw_amount,
  token_amount, ok}, over_cap, rejected, recipient_count, total_token,
  total_fiat?, over_balance, can_apply, recipients[]{address, amount,
  name?}, applied` (`line`, `rejected`, `recipient_count` are `u32`;
  `decimals` `u32`; `price_usd` `f64`).
- **ContactSaveInput** `{address, name?, note?, favorite?, kind?,
  resolved_name?, resolved_source?}`; **ContactExportFile** `{filename,
  mime, content, contacts}`; **ContactImportReport** `{added, skipped,
  invalid, groups_created}`; **ContactImportFailure** `{MalformedJson,
  NoAddressColumn, Empty, UnknownGroup}`; **ContactRecipientView**
  `{address, saved, verified, display_name?, identity?, kind}`.
- **ContactFormModel** (drawn) `{title, nameLabel, namePlaceholder,
  addressLabel, addressPlaceholder, name, address, error?, save, cancel,
  saveEnabled}`.
