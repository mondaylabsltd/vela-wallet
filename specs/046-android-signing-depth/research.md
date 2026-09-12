# Research — 046

## D1 — Simulation is the shell's RPC, the core's judgment
`eth_simulateV1([{blockStateCalls:[{calls:[{from,to,value,data}]}], validation:false, traceTransfers:true}, "latest"])`
through the pool; only calls with `status 0x1` contribute logs; Transfer
logs (3 topics) touching the wallet are netted per token, the
`0xeeee…` sentinel is the native coin — the desktop's `sim.rs` verbatim.
Deltas go to the wallet's `token_trust` host as `SimDeltasComputed{address,
chain_id, deltas}`; the view's `sim.ready` gates the read of `judgments`
(`Native{delta}`, `Erc20Trusted{token, delta, symbol, decimals}`,
`Erc20Unverified{token?, delta}`). The web's `judgeSimDeltas` waits the same
way. A refusal → `null` → the unavailable warning; nothing is written.

## D2 — Where the block sits
The signing sheet (SigningBlock.Balances: `balanceChangesTitle`, rows
`symbol · delta`, note). Empty judgments → `simResultNoChange`. The desktop
(037) put it on the signing sheet only; Send confirm keeps 043's facts.

## D3 — eth_sign
`DappRpc` routes `eth_sign` to Sign; the clear machine's `MessagePresented
{EthSign}` (already sent by 044's kickoff) yields the danger class the sheet
draws; `SignExecutor` hashes `[address, data]` as the EIP-191 message of
`data` (the same envelope as personal_sign, params swapped). The parity
test records the deviation from protocol.js with the desktop precedent.

## D4 — Scanner
CameraX `Preview` + `ImageAnalysis` (YUV_420_888 → ZXing
`PlanarYUVLuminanceSource` → `MultiFormatReader` with QR hint), one
background executor, first hit wins and the analysis stops. A picked photo:
`DocumentPorts.pick(image/*)` → `BitmapFactory` → `RGBLuminanceSource`.
Permission through an activity-result launcher registered in `onCreate`
(as Bluetooth's). The drawn `ScanSurface` gains a preview slot behind its
brackets; torch through `CameraControl.enableTorch`.

## D5 — What a decoded text becomes
`Eip681.parse(text)` → `SendScan.Request{recipient, chain_id, token_address,
amount_base_units}`; otherwise `SendScan.Text{data}` (the core keeps a bare
address as the recipient, refuses the rest by leaving the field). The core's
`scan_resolved` re-opens the Send locked for a full request; when the chain
is unknown it issues `AddNetwork{chain_id}` — the send executor asks
`SettingsController.addNetworkByChainId` and answers `Added` once
`last_added_chain_id` matches, `NotFound` on timeout.

## D6 — Device harness
QR PNGs generated on the Mac (`qrcode`), pushed to Downloads, picked through
the scanner's photo tool; the camera preview opened and closed under
`uiautomator`; SIWE/eth_sign through the local test dApp (three buttons).
