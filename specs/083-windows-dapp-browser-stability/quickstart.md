# Quickstart — the Windows device pass for 083

## Setup

```powershell
# builds (release; the parallel space needs dev-fixtures and signs with the fixed keyset)
cd app-desktop/vela-wallet
cargo build --release --locked                       # the installer's payload
cargo build --release --features dev-fixtures        # parallel space (copy the exe out first)
./scripts/build-windows-installer.ps1 -Architecture x64 -SkipBuild   # run WITHOUT *> redirection (PS 5.1 stderr quirk)

# test dApp on two origins; port 80 so the address can be TYPED (posted keys cannot hold Shift for ':')
cd app-android/vela-wallet/dev/testdapp; python -m http.server 80 --bind 127.0.0.1; python -m http.server 81 --bind 127.0.0.1
# fault proxy, upstream = the machine's own proxy (v2rayN here)
$env:CHAOS_UPSTREAM='127.0.0.1:10808'; python scripts/device/chaos-proxy.py chaos.log
```

Launch a test instance (never the owner's process), off-screen, its own state and profile:

```powershell
$env:VELA_STATE_DIR='<scratch>\st'; $env:VELA_PARALLEL_SPACE='1'; $env:VELA_NO_ACTIVATE='1'
$env:VELA_BROWSER_URL='http://127.0.0.1/'; $env:VELA_SECTION='explore'
$env:HTTPS_PROXY='http://127.0.0.1:8899'; $env:NO_PROXY='127.0.0.1,localhost'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9337 --disable-features=CalculateNativeWinOcclusion,msWebOOUI,msPdfOOUI,msSmartScreenProtection --proxy-server=http://127.0.0.1:8899'
```

The remote-debugging port works on the release build; drive the page over CDP (`Runtime.evaluate`,
`Page.captureScreenshot`, `Page.crash`), the wallet window with posted mouse/keys (logical px = physical
/ 1.75) and PrintWindow (it captures the WebView2 child even off-screen). Faults:
`curl "http://127.0.0.1:8899/__chaos?mode=drop&match=<host>"` (also `blackhole`, `latency=6000`, `pass`).
Note: the app's own traffic falls back to Direct around the fault proxy (and this machine's TUN makes
Direct work), so relay faults cannot be held on the app's own calls here.

Read after every run: stderr of the instance, and
`Get-WinEvent -FilterHashtable @{LogName='Application';Id=1000,1001}` filtered on `vela|msedgewebview2`.

## A — Engine (US1)

| # | Do | Expect |
|---|---|---|
| E1 | exe in a folder the user cannot write (ACL deny Write), open the test dApp | page loads; profile under the state dir; nothing beside the exe |
| E2 | `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=C:\nope` | panel "无法加载此页面 · host · WebView2 0x80070002", 在系统浏览器中打开 first; one log line; ~0 CPU |
| E3 | installed build from `C:\Program Files` (owner approves UAC) | Explore opens a site; `%LOCALAPPDATA%\VelaWallet\WebView2` exists |

## B — Address bar (US2)

| # | Do | Expect |
|---|---|---|
| B1 | type 3 addresses with Enter | at 0.35 s and 4 s the bar shows 🔒 + the NEW host, not editing |
| B2 | click the bar once, type, Enter ×4 | `location.href` is exactly each typed site |

## C — Loads (US3)

| # | Do | Expect |
|---|---|---|
| C1 | `drop match=uniswap`, open app.uniswap.org | Vela panel from 0.3 s; 0 Edge frames over 25 s; log "WebView2 status 8" |
| C2 | drop a new host, `pass` after 4 s | page loads by itself at the next scheduled retry |
| C3 | `blackhole match=pancakeswap` | panel from ~12 s, still up at 80 s (Edge's status 7 at ~40 s does not replace it) |
| C4 | expired.badssl.com | "网站证书有问题。Vela 已阻止打开。", no interstitial, open lock, no auto retry |
| C5 | kill this instance's `--type=renderer` process | crash panel; 重新加载 restores the page |

## D — Signing (US5) and the rest

| # | Do | Expect |
|---|---|---|
| S1 | Sign, press Esc ×3 | column stays; page has no answer; ✕ → one 4001 |
| S2 | Send dust (parallel space) | "正在准备交易…" then submitting; never "等待生物识别…" without a prompt; reads as a transfer |
| S3 | owner's phone-key account on the installed build, Sign | QR card in the column; phone scan signs; ✕ stops the scan |
| N1 | `target=_blank` link, `window.open` from a click | new tab on the address |
| N2 | `mailto:` link | handed to Windows once (or logged with `VELA_OPEN_URL_LOG`); timers' popups refused |
| N3 | ⋯ menu over a page | the page stays visible around the menu |

## F — Installer: for everyone, or for me only (H9, D5)

Build with `./scripts/build-windows-installer.ps1` (it prints "Visual C++ runtime: needs 14.x or newer").
Run F1–F3 on a machine or VM with no Vela; `/LOG=<file>` keeps Setup's log.

| # | Do | Expect |
|---|---|---|
| F1 | run the installer, pick **Install for me only** | no UAC; app in `%LOCALAPPDATA%\Programs\Vela Wallet`; `HKCU\Software\Classes\velawallet` opens it; a Trusted Signer answer reaches the running app |
| F2 | `/CURRENTUSER /VERYSILENT /LOG=f2.log`, then the same again (an unattended upgrade) | both finish with no prompt; the log says the runtime is present, or "not installed: per-user and silent" — never runs `vc_redist` |
| F3 | a standard (non-admin) account on a machine with an older runtime, **Install for me only** | after the files, Windows asks for an administrator; Cancel → Setup finishes with the warning naming `aka.ms/vc14`, not an error |
| F4 | over the owner's per-machine install: a normal run; then (on the owner's D5 go-ahead) uninstall it as an administrator and install `/CURRENTUSER` | no mode question, UAC, upgrade in `C:\Program Files\Vela Wallet`, `HKLM\Software\Classes\velawallet` exists; after the move, a `/VERYSILENT` upgrade needs no UAC and the Trusted Signer answer still arrives |
