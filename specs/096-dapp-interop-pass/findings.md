# 096 real dApp pass — findings

Extension build = main + 090–095 (PR #388 #389 not yet on main), golden Safe
`0x88cC…6894`, BNB Chain, 2026-10-02. The lead's run. Evidence lives in the
session scratchpad (`dapp096/`), not in the repository: screenshots in
`dapp096/shots/`, request JSONs and console in `dapp096/logs/` (`req-*.json`,
`console.log`, `run.log`). Shot names below are those files.

Worked: PancakeSwap WBNB→USDC (batch approve + Permit2 approve + router) and
USDC→BNB; Aave supply/withdraw WBNB; Uniswap capped approve; Sky
`personal_sign` (exact bytes); account switch → dApp `accountsChanged` in 4 s;
~7 s slide → block.

| # | Sev | Finding | Evidence | Part |
|---|-----|---------|----------|------|
| F1 | S2 blocker | Any dApp contract call sending native coin fails before signing on in-band chains (web/extension). `safe-transaction.ts:1900` `BigInt(call.value \|\| '0')` on a `MultiSendCall.value` that `dapp-submit.ts` stripped of its `0x` (`aa87bee538000`); console `Cannot convert aa87bee538000 to a BigInt`, caught as "Could not estimate gas… network may be busy". Introduced in 16139e041 (spec 062). Reproduced on PancakeSwap BNB→USDC, Uniswap BNB→USDC, Aave depositETH. | shots 19-pcs-swap1-sheet, 21-pcs-swap1-dapp-after, 77-uni-bnb-dapp-after, 106-aave-bnb-dapp-after; logs/console.log | A |
| F2 | S2 | Fee coin auto-picked as a token the batch spends in full (USDC→BNB pulls all USDC via Permit2; `fee_policy.rs` `auto_pick` counts only native value and `transfer` calls without a simulation; none ran on BSC) → relay rejected; Activity "Failed" with no reason; the picker offers USDC with no warning. | 53-pcs-usdc-sheet-fee, 62-pcs-usdc2-feepicker, 57-activity-usdc-failed-detail; logs/req-pcs-usdc2.json | C |
| F3 | S3 | For that rejected op `wallet_getCallsStatus` kept answering 100 while the tracker logged `rejected`. | logs/run.log 13:11:12, 13:12:10, 13:12:43 | A |
| F4 | S2 | The native value sent never appears in the readable sheet; generic uint args labelled "Value" (PancakeSwap/Uniswap "Value 1,790,947,024" = the deadline; Aave depositETH "Value 0" = referral code while 0.003 BNB is sent). | 19-pcs-swap1-sheet, 105-aave-bnb-sheet; logs/req-105-aave-bnb.json | B |
| F5 | S3 | Little readable: batch legs show no amounts; unnamed contracts (Uniswap UniversalRouter on BSC, canonical Permit2, PancakeSwap Permit2, Aave Pool, CoW, Sky PSM3); raw numbers; withdraw-all as a 78-digit number; CoW "Set pre signature" with no terms. | 44-pcs-wbnb-sheet-a, 113-aave-supply-tech, 118-aave-withdraw-sheet, 140-cow-sheet, 171-sky-convert-sheet | B |
| F6 | S2 | Token missing on approval-type sheets: Permit2 signature never names WBNB; uncapped approve says "−Unlimited" with no token. | 82-uni-permit-sheet, 79-uni-wbnb-sheet; logs/req-82-uni-permit.json | B |
| F7 | S3 | Clear-signing loading shows "Set a finite amount to continue." (`live.ts:456-458` reuses the approval-cap prompt) under the slide, > 4.6 s on Aave supply. | 112-aave-supply-sheet-fee | B |
| F8 | S3 | A request that fails before signing: the request window closes ~1 s after the slide, no message, no Activity row. | 18b-pcs-no-window, 21-pcs-swap1-dapp-after | A |
| F9 | S3 | Activity rows don't state intent ("Batch on PancakeSwap", "Contract interaction on app.aave.com"); no amounts/fee; naming inconsistent. | 47-activity-pcs1-home, 48-activity-pcs1-detail, 84-activity-uni-detail, 121-activity-aave-detail, 190-activity-final-activity-all | B |
| F10 | S3 | On PancakeSwap Vela is reachable only as "MetaMask" (isMetaMask shim). External; no code change. | 11-pcs-wallet-list | — |
| F11 | S3 | The connect window shows only "Connect to <host>" + one sentence; no account, no network. | 13-pcs-consent-window | A |
| F12 | S3 | Send to the WBNB token contract: no "this is a contract/token address" warning, only "First time sending here". | 33-send-confirm | C |

Untested: 1inch (app.1inch.io redirects to 1inch.com).
