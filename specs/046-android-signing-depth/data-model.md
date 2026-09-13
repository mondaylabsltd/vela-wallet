# Data Model — 046
- **SimCall** `{to, value, data}` (from `SignExecutor.callsOf`).
- **TrustAssetDelta** `{kind: native|erc20, token?, delta}` (signed base units, decimal string).
- **TrustSimJudgment** (core) → **BalanceDeltaRow** `{symbol, delta, tone}`.
- **Eip681Request** `{chainId?, recipient, tokenAddress?, amountBaseUnits?, isNative}`.
- **SendScan** `Request{…}` | `Text{data}` (core wire, unchanged).
- **ScanState** (shell) `{permission: Unknown|Granted|Denied, torch: Boolean, decoded: String?}`.
