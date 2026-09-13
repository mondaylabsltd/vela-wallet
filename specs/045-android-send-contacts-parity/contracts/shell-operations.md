# Shell operations — 045

## batch_import (`BatchExecutor`)
| Operation | Answer |
| --- | --- |
| `FetchUsdFiatRate{code}` | `RateResolved{code, rate?}` — the wallet's fiat-rate port (Chainlink through the pool, as the display currency does) |
| `PickFile` | `FilePicked{name, content: Text{text} \| Matrix{rows}}` / `FilePickCancelled` / `FilePickFailed` — `OpenDocument` |
| `SaveTemplateFile{name, contents, mime}` | `TemplateSaved` / `TemplateSaveFailed` — `CreateDocument` |

Events: `Open{token, currency_code, max_recipients}`, `SetUnit`,
`SetFiatCode`, `SetRawText`, `PickFileRequested`, `SaveTemplateRequested`,
`EditRate`, `ResetRateToAuto`, `Apply` → then `SendEvent.SeedSplitRecipients`.

## send (existing executor; new events sent)
`EnterSplitMode`, `RecipientsChanged{recipients}`, `SeedSplitRecipients`,
`ToggleMultiToken{token_id}`, `ToggleAllMultiTokens{visible_ids}`,
`SetMultiNetwork{chain_id?}`, `ConfirmMultiSelection`, `OpenBatchImport`,
`CloseBatchImport`, `DismissTreasurySheet`; `FeeEvent.QuoteRequested` on
stale.

## contacts (existing executor; new events + view)
Events: `ImportFile{content, filename?, into_group?, now_ms}`,
`ImportAcknowledged`, `ExportRequested{scope, format, exported_at_iso}`,
`ExportTaken`, `AddGroupMembers{id, members}`, `RemoveGroupMember{id,
address}`, `SetContactGroups{address, group_ids}`, `InspectRecipient`,
`Save`, `ToggleFavorite`. View: `export: ContactExportFile?`,
`import_failure`, `sections`. The executor's arms are unchanged (the file
content is carried by events; the shell reads/writes files outside the
machine, through `DocumentPorts`).
