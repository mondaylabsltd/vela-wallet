# Shell operations — 046
| Machine / port | What the shell does |
| --- | --- |
| `token_trust` `SimDeltasComputed` (event) | after `eth_simulateV1`; read `sim.judgments` when `sim.ready` |
| `sign_request` / `clear_signing` | `eth_sign` now reaches `RequestArrived` + `MessagePresented{EthSign}` |
| `send` `OpenScanner` / `CloseScanner` / `ScanResolved{scan}` | the camera or photo decode, tokenized by `Eip681` |
| `send` op `AddNetwork{chain_id}` | `SettingsController.addNetworkByChainId` → `NetworkAdded{Added|NotFound}` |
