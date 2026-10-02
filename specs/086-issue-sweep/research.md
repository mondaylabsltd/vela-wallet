# Research: 086 — root causes

Filled in as each cluster reports. File:line references are on `main` @ `67e2d193d` unless noted.

## #318 — iOS Trusted Signer "does not work" (cluster F)

- **Decision**: already fixed on `main`; add an opt-in round-trip device test and evidence (PR #341).
- **Root cause**: the report is against v0.9.4 (`3383b8d`, 2026-09-21). That build had no URL channel on
  iOS, so the page's answer had no way back to the app. `741b74263` (2026-09-24) added
  `velawallet://sign-result` → `RootView.onOpenURL` → `TrustedSignerCallbacks.deliver`.
  `209b92f70` and `229b4a5bd` fixed it after that. Android was device-proven then; iOS was unit-tested only.
- **Evidence (2026-10-01)**:
  - iOS 18.0 simulator: create → the real page → passkey created → member proof → both answers back → key
    list `1 / 7`. Passed in 32.5 s.
  - iPhone 11 on iOS 26.5.2: the page opens and draws, the slide works, and the system passkey picker
    appears. The final Face ID needs a person.
  - iOS 26.2 simulator: no local passkey provider (only "Other Devices"), so it cannot create a key.
    This is a simulator limit.
- **Alternatives considered**: a stand-in page via `-vela.trustedSignerUrl`. Rejected: ceremonies always
  use the official page, because the passkeys belong to getvela.app.
- **Found along the way, for spec 087**:
  - On a Face ID iPhone, "这台设备" is described as "Touch ID 或 Windows Hello".
  - The sign-in chooser's "手机或平板" says "扫码，用附近设备**创建**".
  - Home's 资产 section is blank, with no empty state, when there are no assets.
