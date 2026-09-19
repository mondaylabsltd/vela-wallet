## Installing

Every package on this page has been built from the tagged source and is meant to be usable as downloaded. `SHA256SUMS*` files list the checksum of each one.

**Windows** — `VelaWallet-Setup-<version>-x64.exe` for most PCs, `-arm64.exe` for Snapdragon / Surface Pro X class machines.
The installer is not code-signed, so Windows SmartScreen stops it once with *"Windows protected your PC"*. Click **More info**, then **Run anyway**. This is expected for this release; it is the same file the checksum describes.

**macOS** — if a `.dmg` is attached, it is signed with our Developer ID and notarized by Apple: open it and drag *Vela Wallet* to *Applications*. `universal` runs on every Mac; `arm64` (Apple silicon) and `x86_64` (Intel) are smaller.
If no `.dmg` is attached, this version's macOS build has not been published — we only attach one that opens without warnings. Use the [web wallet](https://wallet.getvela.app/) in the meantime.

**Linux** — `x86_64`/`amd64` for most PCs, `aarch64`/`arm64` for ARM.
- Debian, Ubuntu, Mint, Pop!_OS: `sudo apt install ./vela-wallet_<version>_amd64.deb`
- Fedora, RHEL, openSUSE: `sudo dnf install ./vela-wallet-<version>-1.fc*.x86_64.rpm`
- Anything else: `flatpak install --user ./app.getvela.VelaWallet-<version>-x86_64.flatpak`

Your wallet is not in the app: it is the passkey you sign in with. Installing, reinstalling or switching platforms never moves or risks funds.

**Phones** — the iOS and Android apps are distributed through the App Store and Google Play, not here. They can also be built from this source; see the repository README for what differs in a self-built app.

---
