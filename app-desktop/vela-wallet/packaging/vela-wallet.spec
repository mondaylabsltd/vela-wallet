# RPM for Fedora / RHEL / openSUSE, built from an already-linked release
# binary. Drive it through scripts/build-linux-packages.sh — that script stages
# the tree, hands it over as Source0 and passes %%vela_version.
#
# This deliberately does NOT compile from source. `gpui` and `gpui_platform` are
# git dependencies on the full zed-industries/zed repository, so a %%build that
# ran cargo would need network access, which mock and Koji do not grant. The
# Windows installer packages a prebuilt .exe for the same reason.

%global appid app.getvela.VelaWallet

# No -debuginfo subpackage: the binary is stripped before staging, and there is
# no source tree here for a debug package to point at.
%global debug_package %{nil}
# The staged binary may target a different architecture than the host, whose
# `strip` would refuse it. Stripping already happened in the build script.
%global __strip /bin/true

Name:           vela-wallet
Version:        %{vela_version}
Release:        1%{?dist}
Summary:        Self-custodial smart wallet for EVM networks

# TODO: the repository ships no LICENSE file. Replace with the real SPDX
# identifier before publishing; keep it in step with <project_license> in
# packaging/app.getvela.VelaWallet.metainfo.xml.
License:        LicenseRef-proprietary
URL:            https://getvela.app
Source0:        %{name}-%{version}-stage.tar.gz

# rpm's automatic dependency generator reads the ELF's NEEDED entries, which for
# this binary covers only glibc, libstdc++, libxcb and libxkbcommon. Everything
# below is loaded with dlopen() at runtime and is therefore invisible to it —
# `strings` on the release binary is how the list was derived, and re-deriving it
# after a gpui bump is the way to keep it honest:
#
#     strings -a target/release/vela-wallet |
#       grep -oE 'lib[A-Za-z0-9_-]+\.so(\.[0-9]+)*' | sort -u
#
# Without these the package installs cleanly and then dies at startup with
# "Library libwayland-client.so could not be loaded" or a blank window.
Requires:       libwayland-client
Requires:       libwayland-egl
Requires:       libglvnd-egl
Requires:       vulkan-loader
Requires:       hicolor-icon-theme
# gpui renders through Vulkan. The loader is required above; an ICD is a
# separate package that no soname records. Recommends rather than Requires so an
# NVIDIA or AMDVLK box is not forced to pull Mesa's driver in as well.
Recommends:     mesa-vulkan-drivers
# Fontconfig is parsed in pure Rust (fontdb + fontconfig_parser), so libfontconfig
# is not a dependency — but the config and the fonts it points at still have to
# exist, or text falls back to nothing.
Requires:       fontconfig
# Security keys are opened directly (/dev/hidraw*). systemd 252 and later
# already tags FIDO devices for the logged-in user; the rule this package ships
# covers older systems and is inert alongside it.
Requires:       systemd-udev

%description
Vela Wallet is a self-custodial smart account wallet for EVM networks. Accounts
are Safe smart contracts with ERC-4337 account abstraction, and transactions are
signed with a passkey - there is no seed phrase and no private key to store.

Balances and USD prices for 24 EVM networks appear in a single view, priced from
on-chain DEX quotes with a Chainlink oracle fallback.

%prep
%setup -q -c -T
tar -xzf %{SOURCE0}

%build
# Intentionally empty: Source0 is a staged filesystem tree, not source code.

%install
mkdir -p %{buildroot}
cp -a usr %{buildroot}/
install -Dm0644 usr/lib/udev/rules.d/70-vela-fido.rules \
  %{buildroot}%{_udevrulesdir}/70-vela-fido.rules

%files
%{_bindir}/vela-wallet
%{_udevrulesdir}/70-vela-fido.rules
%{_datadir}/applications/%{appid}.desktop
%{_datadir}/icons/hicolor/*/apps/%{appid}.png
%{_datadir}/icons/hicolor/scalable/apps/%{appid}.svg
%{_datadir}/metainfo/%{appid}.metainfo.xml

%changelog
* Thu Oct 08 2026 Monday Labs <hello@getvela.app> - 0.9.7-1
- Pre-release: dApp browser tabs keep their page when you switch between them
  (on desktop, no reload), and close the way a browser's do: others, to the
  right, or all. A dApp can ask Vela to add a network, and Vela adds only one
  your account can use. If the relay cannot reach a network, the wallet stops
  before you sign, and it says when the relay is topping up its gas. The fee
  coin defaults to one that can pay, and a coin that cannot says why. Coins paid
  straight to the wallet show in Activity, the desktop balance has a refresh
  button, and a tiny incoming amount is no longer shown a trillion times too
  large. Uniswap no longer shows a swap that landed as failed. On iPhone and
  iPad: a spinner while a send is submitted, search when adding group members,
  phone-QR sign-in on iPad, contact export that works, a security-key prompt
  that asks for the key, and no crash when the share sheet is dismissed. On
  Android: the launch screen, showing a hidden group again, and the scan-a-code
  sheet are fixed. The web wallet and the browser extension now send anonymous
  usage statistics, never addresses, amounts or the sites you visit; turn them
  off in Settings → About.

* Sat Oct 03 2026 Monday Labs <hello@getvela.app> - 0.9.6-1
- Pre-release: signing a dApp request now says what the call actually does — the
  coin it sends, a token contract by name, an unlimited allowance, an order's
  unseen terms — and says plainly when something could not be read rather than
  stating it as certain. Every dApp transaction and signature lands in Activity.
  The home screen names each network it cannot reach. The fee is never paid in a
  coin the operation itself may spend.

* Thu Sep 24 2026 Monday Labs <hello@getvela.app> - 0.9.5-1
- Pre-release: a dApp transaction now lands on a receipt instead of closing on you,
  and the browser extension's side panel is the wallet itself; the Clear Signer
  shows what you are signing on this device's own browser and answers back over
  velawallet://; and the Trusted Signer lets a key you trust sign for you.

* Mon Sep 21 2026 Monday Labs <hello@getvela.app> - 0.9.4-1
- Pre-release: a passkey says where it lives (cloud-synced or on this device);
  sign-in checks what an index tells it against the chain; the fee can be
  refreshed and a speed chosen, with the amount's unit shown; and fixes to
  sending, the receive code on Android, and the web layout.

* Sat Sep 19 2026 Monday Labs <hello@getvela.app> - 0.9.3-1
- Pre-release: a wallet's founding record can be backed up to Ethereum; signing in
  with a USB security key no longer crashes; every waiting dialog can be cancelled;
  and About shows the real version and commit.

* Thu Sep 17 2026 Monday Labs <hello@getvela.app> - 0.9.2-1
- Pre-release: the total settles as it loads, contacts keep their names, and the
  founding record can be backed up to Ethereum.

* Thu Sep 17 2026 Monday Labs <hello@getvela.app> - 0.9.1-1
- Pre-release: the first cut whose Linux packages and iOS archive build from a tag.

* Thu Sep 17 2026 Monday Labs <hello@getvela.app> - 0.9.0-1
- Pre-release: the desktop, mobile and extension shells cut together.

* Fri Aug 07 2026 Monday Labs <hello@getvela.app> - 0.1.0-1
- First packaged desktop release.
