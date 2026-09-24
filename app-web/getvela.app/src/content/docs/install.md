---
title: Install Vela
description: "Every way to run Vela — web, browser extension, desktop and phone — what each costs, what each can do, and what your device needs."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Install Vela

The same wallet runs in several places, and they all open the same address with
the same keys. Pick by what you need; you can use more than one. Downloads are on
[Get Vela](/get-started).

| | What it is | Cost | Status |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) in any recent browser | Free | Live |
| **Browser extension** | The wallet in your toolbar; connects to dApps | Free | Download and load by hand; not yet on the Chrome Web Store |
| **Desktop** | Native app for macOS, Windows and Linux | Free | Download from Get Vela or GitHub |
| **iPhone, Android** | Native apps | One-time purchase in the stores | Not in the stores yet; you can build them from source |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Open the web wallet →</a>

## Web

Nothing to install. Open [wallet.getvela.app](https://wallet.getvela.app/), create
a wallet or sign in, and it is there. Your account list is kept in this browser;
on another device you simply sign in again with one of your keys.

## Browser extension

Chromium browsers: Chrome, Edge and Brave (Chrome 116 or later). It puts the wallet
in the toolbar and lets dApps connect to it directly. Until it is on the Chrome Web
Store:

1. Download the extension from [Get Vela](/get-started) and unzip it into a folder
   you will keep — the browser runs it from there.
2. Open `chrome://extensions` and turn on **Developer mode**.
3. Click **Load unpacked** and choose that folder.

It is the same wallet: extension and web wallet use the same `getvela.app`
passkeys, so the same keys open the same address.

## Desktop

A native app, not a web page in a window: **Windows** 10 and 11 (x64 and ARM),
**macOS** 11 or later, and **Linux** (.deb, .rpm or Flatpak, x64 and ARM).

- **Windows** will warn that it "protected your PC", because the installer isn't
  code-signed yet. Choose **More info**, then **Run anyway**.
- **macOS** builds are signed and notarized by Apple in a separate step, so they
  can lag behind the other platforms. When the Mac button says "coming shortly",
  the most recent notarized Mac build is on the GitHub releases page.
- **Linux**: to use a USB security key, your system must give the app access to
  it — the .deb and .rpm packages install the rule for you.

On macOS and Windows the desktop app has a built-in browser for dApps. Checksums
for every package are on the
[GitHub releases page](https://github.com/mondaylabsltd/vela-wallet/releases) —
and you can check more than a checksum, see below.

## iPhone and Android

Native apps for iOS 17.4 or later and Android 10 or later. They will be sold as a
one-time purchase in the App Store and Google Play; they are **not in the stores
yet**. The code is open, so you can build them yourself for free — with one
difference: a build you sign yourself can't use your phone's own passkeys for
getvela.app wallets, though scanning with another phone and USB security keys
work. See [build the apps yourself](/docs/self-hosting#web-app).

## Verify what you downloaded

A checksum tells you that two files are identical. It cannot tell you who made
the file — and the list of checksums sits on the same page as the download. So
every package we attach to a release is also **attested**: the workflow run that
built it signs a statement naming the file, the commit and the run, and GitHub
keeps it. Checking it takes one command with the
[GitHub CLI](https://cli.github.com) (sign in once with `gh auth login`; the
check is free):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

It prints who built the file and from which commit, or it fails. Nothing on your
machine has to trust us for that answer: the signature is GitHub's, made at build
time, and it cannot be produced by someone who merely re-uploads a file
somewhere.

Mac images are signed with our Developer ID and notarized by Apple, which macOS
checks for you when you open one. To ask it yourself:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="The Windows prompt stays">
Attestation is not code signing. The Windows installer is not code-signed, so
SmartScreen still stops it once with "Windows protected your PC" — choose
<strong>More info</strong>, then <strong>Run anyway</strong>. Verifying the
attestation is the check that tells you the file is really ours; the prompt is
about a certificate we haven't bought.
</Callout>

Packages published before this was turned on carry their checksums only.

## Using Vela with dApps

<span id="dapps"></span>

dApps connect to Vela the way they connect to any browser wallet (EIP-1193 and
EIP-6963):

- in a desktop browser, through the **Vela browser extension**;
- inside the **desktop app** (macOS, Windows), the **iPhone app** and the
  **Android app**, through their built-in browser.

The web wallet at wallet.getvela.app doesn't connect to dApps, and there is no
WalletConnect. Every request a dApp makes is decoded and shown to you before you
sign — see [clear signing](/docs/clear-signing).

## What your device needs

Vela signs with **passkeys**, which nearly every device from the last few years
supports:

| Device | Supported |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16+, macOS with a recent Safari or Chrome |
| Android | A recent Android with Google Play services, or a USB security key |
| Windows | Windows Hello with Chrome or Edge, or a security key |
| Linux | A security key, or a phone nearby (scan the QR code) |

If your device can't hold a passkey itself, use another phone or a hardware
security key. [Signers & security keys](/docs/signers) lists which kinds of key
each app supports.

## The only official addresses

- **getvela.app** — this site, and the downloads
- **wallet.getvela.app** — the web wallet
- **github.com/mondaylabsltd** — the code and release packages

<Callout type="warning" title="Check before you install">
If anything sends you elsewhere to "install Vela" or to "verify your wallet",
stop. Vela never asks for a seed phrase — it doesn't have one.
</Callout>

Next: [create your wallet](/docs/create-wallet).
