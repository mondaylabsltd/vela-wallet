# Data Model — 047
- **Preferences** `{language: "system"|tag, numberFormat, dateFormat, timeFormat, textScale, avatarStyle}` under the web's keys.
- **DeviceStorageReport** `{items: [{id, records?, bytes}], groups: {user, cache, sessions}}`.
- **CrashReport** `{at_ms, thread, message, stack (trimmed), version}` under `vela.crashReport`.
- **PayLink** → the core's `PayRequest {recipient, chain_id, token_address?, amount?, amount_base_units?, symbol?, decimals?}` via `LinkOpened`.
