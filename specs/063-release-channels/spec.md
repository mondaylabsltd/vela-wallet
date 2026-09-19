# 063 — Release channels: what each shell ships where, and nothing unusable on GitHub Releases

**Status**: channel policy RULED 2026-09-18 (§0); built and merged (#243); the existing
releases cleaned up the same day. macOS publishing is **local by ruling** (§3a) and waits on
three things only the account holder can make (§6).
**Origin**: founder, 2026-09-18, after the first week of tagged releases (0.9.0 → 0.9.2).
Three of the six things those tags published could not be installed by the person who
downloaded them.
**Shells**: Android, iOS, desktop (macOS · Windows · Linux), browser extension, and the
download page on getvela.app. No product code; one Info.plist key and one entitlement.

> 「发现一个问题 release android 版本安装时报错，package info is null」
> 「还有一个问题就是 mac 版本也是无法打开的，通过 github release 的 mac 版本」
> 「我现在感觉 github release 不应该存在没法用的安装包」

## 0. The founder's ruling (2026-09-18)

> 「1. 如果是 android ios 不在 github release 发布，必须在应用市场下载或自己编译
> 2. 如果是 desktop，首先提供一份在 github release 发布，必须全部可用，然后尽量上应用市场，
> 如果上不了也没关系
> 3. 如果是 chrome extension 和 desktop 一样」

| Shell | GitHub Releases | Stores | Build it yourself |
|---|---|---|---|
| Android, iOS | **never** | the channel (paid) | yes — with the difference §4 names |
| Desktop | **the first channel; every package attached must be usable** | best effort, not a gate | yes |
| Browser extension | same as desktop | Chrome Web Store, best effort | yes |

Three further rulings from the same conversation:

- **Windows ships without a code-signing certificate.** 「虽然有 SmartScreen 会弹警告，但是
  基本可用，只是需要提示用户怎么使用，我觉得没必要再买 windows 证书」— "usable" on Windows
  means *installs and runs*; the release notes say how to get past the SmartScreen sheet.
- **"Must be usable" is a mechanism, not a habit.** A workflow attaches a package only when
  the conditions that make it usable were met in that run. It must be impossible to publish
  the unusable thing by forgetting.
- **A self-built phone app is a working wallet, with a different front door.** 「虽然不能用平台
  认证器创建，但是我们实现了扫码和硬件密钥啊，这样用户其实还是能用的，只是要告知用户用法上
  的差异」— verified in §4; the first draft of this analysis had it wrong.

## 1. Why this matters

The download page sends people to GitHub Releases. For a week, what they found there was:

| Package | What happened to a person who downloaded it |
|---|---|
| Android `.apk` / `.aab` | "package info is null" — the APK carries no signature at all (`apksigner verify`: *Missing META-INF/MANIFEST.MF*), and an unsigned APK cannot be parsed by the installer |
| iOS `.ipa` | cannot be installed on any device: unsigned, and iOS has no sideloading to install it with |
| macOS `.dmg` | "is damaged and can't be opened" — ad-hoc signed, not notarized, so Gatekeeper refuses the quarantined download. Opened by force, it still has **no platform passkeys**: an ad-hoc signature cannot carry `associated-domains` |
| Windows `Setup.exe` | installs; SmartScreen warns first |
| Linux `.rpm` `.deb` `.flatpak` | installs |
| Extension `.zip` | loads unpacked in developer mode |

None of this was a regression. It is what "不用考虑签名" (2026-09-11) produces when the
packages are then *published*: the decision made the packages buildable, and nobody had
yet asked whether they were usable. It is the same shape as the three pipeline defects
0.9.0 exposed — the path had never been walked by the person it was built for.

There is a second reason mobile leaves GitHub that is not about signing: **the store
listing is paid.** A free installable package next to a paid listing is the project
undercutting its own price by accident. Desktop is different *by ruling*: there the store
price buys convenience (one-click install, updates), and the free package is the promise
the download page already makes — "you pay for convenience, not access".

## 2. What "usable" means, per package

A package may be attached to a GitHub Release only if a person with no developer tools can
install it and reach a wallet.

| Package | Usable when | Today |
|---|---|---|
| Linux rpm / deb / flatpak | installs with the distribution's own tool | ✅ |
| Windows Setup.exe | installs and runs; SmartScreen accepted by ruling; notes explain it | ✅ once the notes exist |
| Extension zip | loads unpacked; notes explain it | ✅ once the notes exist |
| macOS dmg | **Developer ID signed + hardened runtime + embedded Developer ID provisioning profile + notarized + stapled** | ❌ — §3 |

## 3. macOS: what a usable package needs

Each item is there for a reason a person would feel:

1. **Developer ID signature, hardened runtime, secure timestamp.** Notarization refuses
   anything less. The script signs with `--timestamp=none` today, which is right for an
   `Apple Development` identity on the founder's own Mac and wrong for distribution.
2. **An embedded Developer ID provisioning profile** for `app.getvela.VelaWallet` with the
   Associated Domains capability. `associated-domains` is a restricted entitlement: under a
   team signature it is honoured only with a profile that grants it, and newer macOS kills
   a team-signed bundle that claims it without one. Without it there are no platform
   passkeys — on the platform where "This device" is the front door.
3. **Notarized and stapled**, the app *and* the dmg. Stapling is what lets a first launch
   succeed offline; the dmg is what the person actually downloads.
4. **`NSCameraUsageDescription`.** Missing from `Info.plist.in` today. The scanner's live
   camera (spec 036 phase 2) has only ever run from `cargo run`, where the *terminal* is the
   process TCC holds responsible. In the packaged app, asking for the camera without a
   usage string is not a denied permission — macOS terminates the process.
5. **`com.apple.security.device.camera`** in the signed entitlements. The hardened runtime
   denies capture without it. The ad-hoc branch never enabled the hardened runtime, which
   is the only reason this has not been seen.

Items 4 and 5 are the same trap as 0.9.0's three: a path that only a published package
walks. They are fixed here because a signed dmg that crashes on "Scan" is not usable.

**The gate (FR-003).** The macOS workflow attaches dmgs to a release only when that run
signed *and* notarized them. With the credentials absent it still builds all three
architectures and still verifies them — the build check 0.9.0 proved the worth of — and
attaches nothing, saying why in the job summary.

**Where the credentials live.** A GitHub *Environment* named `release`, deployable only
from `desktop-v*` tags — not repository secrets. This repository is public; pull-request
and branch workflows must not be able to reach a signing identity. A second, empty
environment `build-check` serves every non-tag run, so a `workflow_dispatch` from a fix
branch (how 0.9.0's fixes were verified before merging) keeps working and simply has no
secrets to find.

## 3a. Where the signature is made — the founder's ruling

> 「quickstart.md §4 操作中的操作我感觉好危险呀，能不能直接本地打包好我上传到 github release 去？」

Yes — and for a wallet it is the better answer, not a concession. The signing key never
leaves the founder's Mac and GitHub holds nothing that can sign as us; the price is one
command per release. `scripts/release-macos-local.sh <tag> [--upload]` is that command: it
refuses to build from anything but the tag, finds the Developer ID profile, picks the
certificate *the profile names* (by SHA-1 — the keychain has same-named ones), proves the
notary credentials before compiling, and will not overwrite published packages unless told.

The gate in CI is unchanged and now permanent rather than provisional: the macOS workflow
has no credentials, so it builds, verifies and attaches nothing. CI signing stays available
(`quickstart.md` §C) and unconfigured.

## 4. A self-built phone app: what works and what does not

Verified in the code, because the first version of this analysis claimed a self-built app
"cannot create a wallet". That was wrong, and the code says why in its own words.

| Way in | Self-built | Why |
|---|---|---|
| Platform authenticator (Credential Manager · AuthenticationServices) | ❌ | The OS checks the app's signing identity against `getvela.app` — `assetlinks.json` fingerprints on Android, the `webcredentials` team id on iOS. A self-signed build is in neither |
| GMS FIDO2 security-key path (Android) | ❌ | Same check, made by Google Play services |
| **Scan with another phone (hybrid / caBLE)** | ✅ | `HybridCeremony`: the Noise handshake and CTAP framing are `vela_core::cable`; the app is its own WebAuthn client and consults no association |
| **USB security key, Android** | ✅ | `UsbSecurityKeyCeremony` — "the GMS-free security-key path", `vela_core::ctap` over USB HID |
| **Security key over CCID, iOS** | ✅ | `SmartCardCtapCeremony` — "consults no association and no Apple service; it is the escape hatch (FR-009c)". Needs a key that speaks FIDO over CCID (YubiKey firmware 5.8+) |

So the honest sentence for the README and the download page is: *a self-built phone app is
the same wallet; "this device" is the one way in it does not have, and scanning with
another phone or plugging in a security key both work.* The escape hatch built for "our
domain is down" is also what makes "build it yourself" true.

## 5. Requirements

- **FR-001** `android-package.yml` and `ios-package.yml` never create or upload to a GitHub
  Release. They keep building on their tag and on `workflow_dispatch`, keep the tag-equals-
  version assertion, and keep the workflow artifact — that is the file the founder signs
  and submits to the store, and the build is the check that the release configuration still
  compiles.
- **FR-002** `build-macos-app.sh` gains a distribution mode: secure timestamp under a
  `Developer ID` identity; the dmg itself signed; `--notarize` submits app then dmg,
  staples both, and fails the build if Apple rejects either. A distribution signature
  without a provisioning profile is an error, not a warning.
- **FR-003** `desktop-macos-packages.yml` signs and notarizes when the `release`
  environment supplies credentials, and attaches dmgs only when it did (§3, the gate).
- **FR-004** `Info.plist.in` carries `NSCameraUsageDescription`;
  `entitlements-signed.plist` carries `com.apple.security.device.camera`.
- **FR-005** Every desktop and extension release opens with install notes — SmartScreen on
  Windows, the package tool per Linux format, "Load unpacked" for the extension — from one
  file per release line, so the three desktop workflows that race to create the release
  write the same words whichever wins.
- **FR-006** The download page stops sending phone users to GitHub. A store is a link when
  its listing is live and a "coming soon" chip when it is not; the GitHub call to action
  belongs to desktop and the extension only. Sixteen locales.
- **FR-007** The release runbook records the policy and the three operational traps 0.9.0
  paid for: push release tags one per `git push`; never delete-and-repush a tag that has a
  release (GitHub turns the release into a draft and the workflow creates a second one);
  a green pull request says nothing about packaging.
- **FR-008** The README's build-from-source section says §4 in plain words.
- **FR-009** *(founder-confirmed, destructive)* the existing `android-v*` / `ios-v*`
  releases and tags are removed; the ad-hoc dmgs and `SHA256SUMS-macos` are removed from
  `desktop-v0.9.1` and `desktop-v0.9.2`.

**Not in this spec**: store submissions themselves (MSIX for Microsoft Store, the Mac App
Store feasibility spike — `research.md` §R5–R6 records what stands in the way), Android
signing in CI (there is no GitHub package to sign any more), marking 0.x releases
"pre-release" automatically, and the flaky Android unit suite.

## 6. What waits on the founder

Nothing below can be done from a checkout. Each is small; together they are the whole
distance between "the dmg is built" and "the dmg is published".

| # | What | Why it cannot be done for you |
|---|---|---|
| N1 | A **Developer ID provisioning profile** for `app.getvela.VelaWallet` with Associated Domains (developer.apple.com → Profiles → + → *Developer ID* → App ID `app.getvela.VelaWallet`) | Issued by the portal to the account holder. This Mac has the *development* profile for that App ID (expires 2027-07-28) and a Developer ID profile for a different app — not this one |
| N2 | The **Developer ID Application certificate as a `.p12`** | The private key is in the login keychain. Note the keychain holds **two** identities both named `Developer ID Application: Qin Xie (F9W689P9NE)` — signing by name is ambiguous on this Mac; export the one that is not expired/revoked |
| N3 | **Notary credentials**: an App Store Connect API key (`.p8` + key id + issuer id), or an Apple ID with an app-specific password | Account-level secrets |
| ~~N4~~ | **NOT NEEDED** — signing is local by ruling (§3a); no secret goes to GitHub | — |
| ~~N5~~ | **ANSWERED 2026-09-18: not on Play yet** (「还没有呀」) — the page's "coming soon" is right as it stands. *Original:* **Store facts for the download page.** `play.google.com/store/apps/details?id=app.getvela.wallet` answers **404 in seven regions** and the App Store lookup for `app.getvela.VelaWallet` returns nothing (both checked 2026-09-18). If the Play listing is in a testing track or in review, the page must keep saying "coming soon"; if it is live under another id, the link needs that id | Only the consoles know |
| ~~N6~~ | **RESOLVED 2026-09-18: it is an organization** — the provisioning profile's `TeamName` is `MONDAY LABS LTD` (the certificates predate the upgrade and still carry a person's name; `quickstart.md` A1 replaces them). The first analysis read the certificate and not the profile. *Original:* **Is the Apple account an organization?** The certificates are issued to a person's name, which is how an *individual* enrollment looks. App Store Review Guideline 3.1.5(i) allows wallet apps only from developers enrolled as an organization. If the enrollment is individual, this blocks the iOS and Mac App Store listings regardless of anything technical — and "iOS: stores only" (§0) then has no store | Only the account holder can see or change the enrollment |
| N7 | **`assetlinks.json` fingerprint `A3:8E:36:FE:…`** — compare with Play Console → App integrity → App signing key certificate. If it matches, it is Google's key and must stay; the debug keystore (`24:EA:D0:…`, password `android`, a file on a laptop) and the retired EAS key (`BE:1E:CA:…`) should leave production | Play Console |
| N8 | ~~A go-ahead for FR-009's deletions~~ (given and done 2026-09-18), and ten minutes with the first signed dmg: open it from a browser download, create a wallet with "This device", open the scanner | Destructive, and a camera |
