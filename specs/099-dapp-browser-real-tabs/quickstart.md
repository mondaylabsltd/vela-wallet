# 099 — Quickstart (how to see it work)

## Core

```sh
cd rust && cargo test -p vela-core --test app_browser_tabs_099 --test app_dapp_browser_099 \
  && cargo test -p vela-core tx_tracker sign_request
```

## Desktop (macOS: a signed bundle — a bare binary gets no passkey)

```sh
VELA_SIGN_IDENTITY=<Developer ID SHA-1> VELA_PROVISION_PROFILE=~/Downloads/VelaWallet_Developer_ID.provisionprofile \
  ./app-desktop/vela-wallet/scripts/build-macos-app.sh --arch arm64 --no-dmg
"dist/macos/arm64/Vela Wallet.app/Contents/MacOS/vela-wallet" 2> /tmp/vela.log
```

1. **Real tabs (SC-001)**: open app.uniswap.org and app.aave.com in two tabs; type an amount in
   Uniswap; switch 20×. The amount stays; `grep 'dapp tab=' /tmp/vela.log` shows no new
   `page=loading` line for either tab after the first load.
2. **Request across tabs (SC-002)**: start a swap in tab A, switch to B while the sheet is up,
   sign; tab A shows the result.
3. **Cap (SC-003)**: open 10 tabs, visit each; at most 6 engines (`ps` shows ≤ 6
   `com.apple.WebKit.WebContent` for the app); come back to the oldest — it reloads and says
   "reloaded to save memory".
4. **Layers (SC-005)**: one fault each —
   - offline page (Wi-Fi off) → page failed, *browser*;
   - `http://` public site → wallet not offered, *provider*;
   - `eth_sign` from a test page → *wallet / unsupported method*;
   - an RPC set to a blackhole (`http://10.255.255.1`) → read ends ≤ 30 s, *network / timed out* (SC-004);
   - the relay refusing (local rig `LIE=0`, plain transfer below the floor) → *relay*;
   - cancel the passkey → *signer*.

   Each fault shows its words in the tab's status entry, its row in the inspector, and one log line.
5. **Crash (SC-006)**: `kill` one WebContent process; that tab shows its crash panel, the others
   still answer `eth_chainId`.
6. **Landing (SC-007)**: swap on a chain whose relay is funding (rig or Arbitrum): within 5 s
   the landing names the relay's state, and "taking longer than usual" never shows before the
   relay reports the bundle sent.

## Phones

- iPhone 11 (`xcodebuild -destination id=00008030-001A75961445802E`) and the Xiaomi
  (`./gradlew :app:installDebug`):
  - SC-002, SC-004, SC-005 and SC-007 as above;
  - memory warning: Xcode's Debug → Simulate Memory Warning, or `adb shell am send-trim-memory
    app.getvela.wallet RUNNING_CRITICAL`, suspends background tabs that are not busy.
