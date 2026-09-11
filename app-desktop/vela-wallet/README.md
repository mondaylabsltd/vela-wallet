# Vela Wallet — Desktop

The desktop client, built on [gpui](https://github.com/zed-industries/zed) (Zed's
UI framework). This crate is **standalone** — it is not a member of the
`rust/` workspace, so all commands run from this directory.

Linux has the largest system dependency set. macOS needs Xcode command-line
tools; Windows needs MSVC build tools and LLVM's `libclang.dll`. Each platform
has its own packaging section below: the Windows installer, the macOS `.dmg`
matrix, and the Linux packages.

---

## Table of contents

- [What the build actually needs](#what-the-build-actually-needs)
- [Install the system dependencies](#install-the-system-dependencies)
- [Windows 11 setup](#windows-11-setup)
- [Build a Windows installer](#build-a-windows-installer)
- [Build macOS packages](#build-macos-packages)
- [Build Linux packages](#build-linux-packages)
- [Build & run](#build--run)
- [Sign in during development](#sign-in-during-development)
- [What the first build does](#what-the-first-build-does)
- [Environment pins](#environment-pins)
- [Tests](#tests)
- [Known gpui quirks](#known-gpui-quirks)
- [Troubleshooting](#troubleshooting)

---

## What the build actually needs

Two direct dependencies drive the entire system-package list, plus one path dep:

| Dependency | Declared at | Pulls in |
|---|---|---|
| `gpui` + `gpui_platform` | [Cargo.toml:7-8](Cargo.toml#L7-L8) | Wayland/X11, xkbcommon, fontconfig, Vulkan, ALSA |
| `dotlottie-rs` | [Cargo.toml:28](Cargo.toml#L28) | **A C++ compiler** — ThorVG is compiled from vendored C++ |
| `vela-core` | [Cargo.toml:11](Cargo.toml#L11) | Pure Rust, no system deps. `i18n-all` compiles all 15 locale catalogs in |

Three notes that are easy to get wrong:

- **The C++ toolchain is not optional.** `dotlottie-rs`'s `build.rs` drives
  `cc::Build` with `.cpp(true)` and calls `cc_build.compile("thorvg")`. A box
  with `cc` but no `c++` fails partway into the build, not at the start.
- **No cmake or meson is required for ThorVG** — despite what the upstream
  project's own docs suggest, the Rust binding compiles the sources directly.
  cmake is still worth having for gpui's transitive crates that fall back to a
  vendored build when pkg-config misses.
- **The build is offline after the fetch.** [Cargo.toml:20-24](Cargo.toml#L20-L24)
  deliberately omits the `tvg-wg` / `tvg-gl` features, because ThorVG's
  `build.rs` *downloads prebuilt archives over HTTP* under `tvg-wg`. Adding a
  GPU backend later re-introduces a network dependency on every build.

### Rust toolchain

`rustc` **1.97.1**, matching the pin in [rust-toolchain.toml](../../rust/rust-toolchain.toml).
The crate is `edition = "2024"`, so older toolchains will not parse it.

---

## Install the system dependencies

### Windows 11 setup

The default build uses the MSVC Rust target (`x86_64-pc-windows-msvc`). Two
separate native toolchains are required:

| Toolchain | Used for |
|---|---|
| Visual Studio Build Tools, **Desktop development with C++** workload | Compiling ThorVG's vendored C++ sources |
| LLVM | Providing `libclang.dll` for `dotlottie-rs`'s `bindgen` build step |

Install the Visual Studio workload in the Visual Studio Installer, or use:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
```

Install LLVM, then point `bindgen` at its `bin` directory:

```powershell
winget install --id LLVM.LLVM --exact
setx LIBCLANG_PATH "C:\Program Files\LLVM\bin"
```

Restart the terminal (and the IDE's integrated terminal) after `setx`, then
run:

```powershell
cd app-desktop\vela-wallet
cargo run
```

For the current PowerShell session only, set the variable directly instead of
restarting it:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo run
```

#### Window controls

The app uses a transparent, custom titlebar to match the design. The empty
band above the Vela mark is a native Windows caption area. Its upper-right
corner also contains explicit **minimize**, **maximize/restore**, and **close**
buttons:

- Drag it to move the window or use Windows 11 Snap Layouts.
- Double-click it to maximize or restore the window.
- Right-click it to open the standard Windows window menu.
- Press `F11` to toggle fullscreen.

Fedora GNOME/Wayland uses the same three controls when it supplies no server
titlebar; X11 and other server-decorated sessions retain their native controls.

If these actions do not work, make sure the executable was rebuilt after a
source update (`cargo run`), rather than reusing an already-running instance.

### Build a Windows installer

Distribute the installer, not the raw `vela-wallet.exe`: it embeds the release
application and Microsoft's signed Visual C++ Redistributable. It creates a
Start-menu shortcut (with an optional desktop shortcut), and installs the
runtime before it offers to launch the app.

Install Inno Setup once:

```powershell
winget install --id JRSoftware.InnoSetup --exact
```

#### x64 installer (default)

Build this package for ordinary Intel/AMD 64-bit Windows 10/11 devices:

```powershell
.\scripts\build-windows-installer.ps1
```

Output: `dist\windows\VelaWallet-Setup-<version>-x64.exe`.

#### ARM64 installer (cross-compiled from x64 Windows)

You do **not** need an ARM64 Windows development machine to create a native
Windows on ARM package. Before the first ARM64 build, install the Rust target
and add **MSVC v143 - VS 2022 C++ ARM64 build tools** in Visual Studio
Installer. Then run:

```powershell
rustup target add aarch64-pc-windows-msvc
.\scripts\build-windows-installer.ps1 -Architecture arm64
```

Output: `dist\windows\VelaWallet-Setup-<version>-arm64.exe`.

The script automatically selects Visual Studio's x64-hosted ARM64 compiler,
builds the `aarch64-pc-windows-msvc` target, and bundles the Microsoft ARM64
VC++ Redistributable. It stops with an actionable error if the Rust target or
ARM64 C++ tools are unavailable. `-SkipBuild` may be added when the matching
release executable has already been built and only the installer needs
repackaging.

#### x64 and ARM64 installers (one command)

After completing the ARM64 prerequisites above, build both Windows packages in
one command:

```powershell
.\scripts\build-windows-installer.ps1 -Architecture all
```

The script builds x64 first and then ARM64. It stops at the first failure, so
do not publish the pair unless both installer files were produced.

| Package | Distribute to | Command |
|---|---|---|
| `VelaWallet-Setup-<version>-x64.exe` | Intel/AMD x64 Windows 10/11 | `./scripts/build-windows-installer.ps1` |
| `VelaWallet-Setup-<version>-arm64.exe` | Native Windows on ARM64 | `./scripts/build-windows-installer.ps1 -Architecture arm64` |
| Both packages | x64 and ARM64 Windows | `./scripts/build-windows-installer.ps1 -Architecture all` |

The application icon is embedded into `vela-wallet.exe` by [build.rs](build.rs),
and `SetupIconFile` gives `setup.exe` the same icon. See
[Icons](#icons) — gpui looks the icon up by resource id, so this is not
something Inno Setup can supply on the executable's behalf.

Both installers print a SHA-256 checksum for publishing next to the download.
The `dist/` directory and downloaded prerequisites are generated and ignored by
Git. An ARM64 executable cannot run on an x64 Windows machine, so complete the
final install and window-behaviour check on a physical ARM64 Windows device or
an ARM64 CI runner. The x64 package can run under Windows on ARM emulation, but
it is not a substitute for distributing the native ARM64 package.

On a release tag (`desktop-v*`),
[the Windows CI workflow](../../.github/workflows/desktop-windows-packages.yml)
builds both installers on x64 runners — ARM64 cross-compiles through the same
VsDevCmd path the script uses locally — and attaches them to the same release
as the macOS and Linux packages. One tag therefore ships every desktop
platform: this pair of installers, the three macOS `.dmg` images, and the
Linux RPM/DEB/Flatpak set, each workflow contributing its own
`SHA256SUMS-*` file.

Release builds use the Windows GUI subsystem, so users see no console window.
Debug builds keep the console available for `cargo run` diagnostics.

The generated installer is **unsigned** until a code-signing certificate is
configured. It works, but Windows SmartScreen can show an “unknown publisher”
warning on first release. Sign the final installer before a public release.

### Fedora / RHEL

```bash
sudo dnf install -y \
  gcc-c++ clang cmake make pkgconf-pkg-config \
  wayland-devel libxkbcommon-x11-devel libxcb-devel \
  fontconfig-devel freetype-devel alsa-lib-devel \
  openssl-devel libzstd-devel vulkan-loader mesa-vulkan-drivers
```

### Debian / Ubuntu

```bash
sudo apt install -y \
  build-essential clang cmake pkg-config \
  libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev \
  libfontconfig-dev libasound2-dev \
  libssl-dev libzstd-dev libvulkan1 mesa-vulkan-drivers
```

### Arch

```bash
sudo pacman -S --needed \
  base-devel clang cmake pkgconf \
  wayland libxkbcommon-x11 libxcb \
  fontconfig alsa-lib \
  openssl zstd vulkan-icd-loader
```

These lists are the intersection of gpui's needs with [Zed's own
`script/linux`](https://github.com/zed-industries/zed/blob/main/script/linux),
trimmed of the packages that serve Zed-the-editor rather than gpui-the-crate
(`sqlite`, `libgit2`, `libva`, `pipewire`, `xdg-desktop-portal`, `musl`, `jq`).
If a gpui bump ever fails on a missing library, that script is the place to look
first — add `glib2-devel` before anything else.

### Why each group

| Packages | Needed for |
|---|---|
| `gcc-c++` | ThorVG's vendored C++ sources |
| `wayland-devel`, `libxkbcommon-x11-devel`, `libxcb-devel` | gpui's Linux windowing and keyboard handling — both display servers |
| `fontconfig-devel`, `freetype-devel` | Font enumeration and rasterization (`gpui_platform`'s `font-kit` feature) |
| `vulkan-loader` + a driver (`mesa-vulkan-drivers`) | gpui renders through Vulkan via blade. **Runtime**, not just build-time |
| `alsa-lib-devel` | gpui's audio path. Carried over from Zed's own list rather than proven necessary for this crate — cheap to install, annoying to discover missing mid-build |
| `openssl-devel`, `libzstd-devel` | Transitive Rust crates that prefer the system library over a vendored build |

---

## Build macOS packages

macOS reads the Dock icon, the application name and the high-DPI opt-in from a
bundle's `Contents/`, so the bare executable runs as a generic tool called
`vela-wallet` and renders at 1x. There is no window-level API that fixes that
from inside gpui — it has to be a bundle. The bundle ships inside a `.dmg`
whose window holds the app and an `/Applications` shortcut — drag one onto the
other and the install is done. That image is macOS's equivalent of the Windows
Setup.exe and the Linux packages, and it is the artifact to distribute.

| Package | Distribute to | Command |
|---|---|---|
| `VelaWallet-<version>-macos-arm64.dmg` | Apple silicon (M1 and later) | `./scripts/build-macos-app.sh --arch arm64` |
| `VelaWallet-<version>-macos-x86_64.dmg` | Intel Macs | `./scripts/build-macos-app.sh --arch x86_64` |
| `VelaWallet-<version>-macos-universal.dmg` | Any Mac — one download, both slices | `./scripts/build-macos-app.sh --arch universal` |

With no `--arch` the script builds the host architecture. Every variant builds
on either kind of Mac — the Apple SDK carries both architectures, so unlike the
Linux packages no second machine is needed. The one per-architecture
prerequisite is the Rust target; when rustup manages the toolchain, the script
stops with the exact command before spending minutes in the build:

```bash
rustup target add x86_64-apple-darwin     # Intel or universal, on Apple silicon
rustup target add aarch64-apple-darwin    # Apple silicon or universal, on an Intel Mac
```

`universal` compiles both targets and merges them with `lipo`, so one file runs
natively everywhere at roughly the sum of the two download sizes — the usual
trade-off against publishing two smaller, architecture-specific images. The
script checks with `lipo -archs` that the packaged binary contains exactly the
slices its filename claims, so a mislabelled `.dmg` cannot reach a user.

Three flags compose with `--arch`: `--skip-build` reuses
`target/<triple>/release/vela-wallet` instead of recompiling, `--zip` also
emits a `ditto`-packed `.zip`, and `--no-dmg` stops after the bundle for quick
local iteration. Bundles land in `dist/macos/<arch>/Vela Wallet.app`, and every
`.dmg` and `.zip` gets a SHA-256 checksum printed for publishing next to the
download.

macOS only: `iconutil` turns the committed
`packaging/icons/macos/AppIcon.iconset` into `AppIcon.icns`, and `codesign`
applies an ad-hoc signature. The iconset itself is generated on any platform by
`./scripts/generate-desktop-icons.sh`, so only the final assembly needs a Mac.
On a release tag (`desktop-v*`),
[the macOS CI workflow](../../.github/workflows/desktop-macos-packages.yml)
builds all three images on one arm64 runner — `--arch universal` compiles both
Rust targets, and the two follow-up invocations repackage with `--skip-build` —
and attaches them to the same release the Linux packages land on.

The bundle is **ad-hoc signed, not notarized**. That is deliberately not
nothing: Apple silicon refuses to run a completely unsigned bundle, so ad-hoc
signing is the difference between "Gatekeeper warns on first launch" and "will
not launch at all". Signing with a Developer ID certificate and notarizing
remains open, alongside the Windows code-signing certificate and package
signing — see [Before the first public release](#before-the-first-public-release).

---

## Build Linux packages

Distribute a package, not the raw `vela-wallet` binary — the package is what
installs the icon, the menu entry and the AppStream metadata that make the app
appear in GNOME Software, KDE Discover and the shell's own search.

| Package | Distribute to | Command |
|---|---|---|
| `vela-wallet-<version>-1.fc<n>.<arch>.rpm` | Fedora, RHEL, openSUSE | `./scripts/build-linux-packages.sh --formats rpm` |
| `vela-wallet_<version>_<arch>.deb` | Debian, Ubuntu, Mint, Pop!_OS | `./scripts/build-linux-packages.sh --formats deb` |
| `app.getvela.VelaWallet-<arch>.flatpak` | Every distribution, sandboxed | `./scripts/build-flatpak.sh` |
| `vela-wallet-<version>-linux-<arch>.tar.gz` | Manual installs, other distros | `./scripts/build-linux-packages.sh --formats tar` |

### RPM and DEB

```bash
cd app-desktop/vela-wallet
./scripts/build-linux-packages.sh              # builds release, emits rpm + deb
```

Output lands in `dist/linux/` with a `SHA256SUMS` file for publishing next to
the downloads. `--skip-build` reuses an existing `target/release/vela-wallet`
instead of recompiling; `--formats` selects a subset.

Every package installs the same five things:

```
/usr/bin/vela-wallet
/usr/share/applications/app.getvela.VelaWallet.desktop
/usr/share/metainfo/app.getvela.VelaWallet.metainfo.xml
/usr/share/icons/hicolor/{16,24,32,48,64,96,128,256,512}x*/apps/app.getvela.VelaWallet.png
/usr/share/icons/hicolor/scalable/apps/app.getvela.VelaWallet.svg
```

There is nothing else to ship. The Lottie launch animations and all 15 locale
catalogs are `include_bytes!`-compiled into the binary
([launch_animation.rs:45-50](src/ui/launch_animation.rs#L45-L50)), so the
package has no runtime data directory to get wrong.

Neither package runs a maintainer script. `desktop-file-utils` and
`hicolor-icon-theme` ship file triggers on `/usr/share/applications` and
`/usr/share/icons/hicolor` in both ecosystems, so the menu database and the icon
cache refresh themselves on install and removal.

**The machine you build on sets the package's glibc floor.** A `.deb` linked on
Fedora records `GLIBC_2.43` and will refuse to install on Debian stable, even
though the packaging itself is fine. Build the `.deb` in a Debian container and
the `.rpm` in a Fedora one — which is exactly what
[the CI workflow](../../.github/workflows/desktop-linux-packages.yml) does:

```bash
podman run --rm -v "$PWD/../..":/src:Z -w /src/app-desktop/vela-wallet debian:12 \
  bash -c 'apt-get update && apt-get install -y build-essential clang libclang-dev \
    cmake pkg-config curl git file libwayland-dev libxkbcommon-x11-dev \
    libx11-xcb-dev libfontconfig-dev libasound2-dev libssl-dev libzstd-dev &&
    curl --proto "=https" -sSf https://sh.rustup.rs | sh -s -- -y &&
    . "$HOME/.cargo/env" && ./scripts/build-linux-packages.sh --formats deb'
```

#### Runtime dependencies are not all in the ELF

`rpm` and `dpkg` both derive dependencies from the binary's `NEEDED` entries,
and for this binary that covers only glibc, `libstdc++`, `libxcb` and
`libxkbcommon`. Four more libraries are opened with `dlopen()` at runtime and
are invisible to both:

```
libwayland-client.so.0   libwayland-egl.so.1   libEGL.so.1   libvulkan.so.1
```

Miss one and the package installs cleanly, then dies at startup with `Library
libwayland-client.so could not be loaded`. They are declared by hand in
[packaging/vela-wallet.spec](packaging/vela-wallet.spec) and in
`dlopen_sonames` in
[scripts/build-linux-packages.sh](scripts/build-linux-packages.sh), and the
build script warns if a declared name no longer appears in the binary. After a
gpui bump, re-derive the list rather than trusting it:

```bash
strings -a target/release/vela-wallet | grep -oE 'lib[A-Za-z0-9_-]+\.so(\.[0-9]+)*' | sort -u
```

The `.deb` maps each soname to a Debian package through a table in the same
script. An unmapped soname is a hard error, not a silent omission.

### ARM64 packages

The build script does **not** cross-compile: ThorVG's vendored C++ plus gpui's
Wayland/X11/Vulkan link set make cross-building far more trouble than building
natively. Three ways to get an `aarch64` package, all running the same script
unchanged:

1. **On an ARM64 machine** — nothing special, just run the script.
2. **In an ARM64 container on an x86_64 host**, via qemu's binfmt handler.
   Correct, and slow enough that it is a last resort — emulating the cold gpui
   and ThorVG build costs hours, not minutes:
   ```bash
   sudo dnf install -y qemu-user-static     # once; registers the binfmt handler
   podman run --rm -it --arch arm64 -v "$PWD/../..":/src:Z \
     -w /src/app-desktop/vela-wallet fedora:latest
   ```
3. **On CI**, which is the intended path — the `ubuntu-24.04-arm` runners build
   natively. `ubuntu-24.04-arm` is free for public repositories; on a private
   repository it requires a paid plan, and the arm64 jobs will queue
   indefinitely without one.

If a release binary was produced elsewhere, wrap it without rebuilding:

```bash
./scripts/build-linux-packages.sh --arch aarch64 --binary /path/to/vela-wallet
```

The script refuses to package a binary whose real architecture disagrees with
`--arch`, so a mislabelled package cannot reach a user.

### Flatpak

```bash
./scripts/build-flatpak.sh --install     # build, export a bundle, install it
flatpak run app.getvela.VelaWallet
```

Everything runs per-user; nothing needs root. The script installs
`org.freedesktop.Platform//25.08` and the `rust-stable` and `llvm20` SDK
extensions if they are missing — llvm20 because `dotlottie-rs` compiles
ThorVG with a hardcoded `clang++` and runs `bindgen` (libclang), neither of
which the base SDK provides — then builds
[packaging/flatpak/app.getvela.VelaWallet.yml](packaging/flatpak/app.getvela.VelaWallet.yml),
and exports a single-file `.flatpak` bundle into `dist/flatpak/`.

Two things to know:

- **The manifest builds from the local git repository, not the working tree.**
  A release bundle should correspond to a commit, so uncommitted changes under
  `app-desktop/vela-wallet`, `rust` or `design` are an *error* rather than a
  silently missing change. To build what is on disk right now:

  ```bash
  ./scripts/build-flatpak.sh --worktree
  ```

  That generates a throwaway manifest in `dist/flatpak/` whose source is a
  `dir` pointing at the checkout, skipping `.git`, `node_modules`, `dist` and
  the `target/` directories — without those skips flatpak-builder would copy
  the multi-gigabyte build tree. It needs PyYAML (`python3-pyyaml`). Use it for
  iteration, not for anything you hand to someone else.
- **The sandbox holds no filesystem permission.** `finish-args` grants the DRI
  device, the Wayland (or fallback X11) socket, IPC and network, and nothing
  else. A wallet should not carry host filesystem access it never uses.

The manifest prints `rustc --version` as its first build command, so a toolchain
mismatch shows up as one line in the log rather than a wall of `edition2024`
parse errors. On the `25.08` runtime the extension is **rustc 1.97.1**, exactly
the version this crate pins.

#### Name resolution fails when the builder is itself a Flatpak

`flatpak run org.flatpak.Builder` puts flatpak-builder inside a sandbox, and the
build it starts is a *second* sandbox nested in the first. That inner sandbox
inherits the runtime's `/etc/resolv.conf`, which points at systemd-resolved's
stub on `127.0.0.53`, without the NSS plumbing that makes the stub usable. The
result is a build with working IP connectivity but no DNS, and cargo fails on
its first git dependency:

```
warning: spurious network error: failed to resolve address for github.com
error: failed to get `dotlottie-rs` as a dependency of package `vela-wallet`
```

Measured inside that sandbox: reaching GitHub by raw IP returns HTTP 301 and
`127.0.0.53:53` accepts a TCP connection, but `getent hosts github.com` fails.
So it is name resolution specifically, not connectivity, and `--share=network`
is already set — it is not the missing piece.

Two ways out, in increasing order of effort:

1. **Use the distro flatpak-builder** rather than the flatpak'd one, so the
   build sandbox is nested one level less deep:
   ```bash
   sudo dnf install flatpak-builder
   ```
   `scripts/build-flatpak.sh` prefers `org.flatpak.Builder` only when no
   `flatpak-builder` binary is on `PATH`, so installing it is the whole fix.
2. **Vendor the crates and build offline**, which removes the network from the
   build entirely. This is required for Flathub regardless — see below — so it
   is the fix that pays for itself.

### Publishing to Flathub

The manifest is Flathub-shaped already — the app id matches the `getvela.app`
domain, and the AppStream metadata validates. Two changes are needed at
submission, both marked in the manifest header:

1. **A public source.** Replace the local `type: git, path:` source with a
   public `url:` and `commit:`. Flathub builds from a public repository.
2. **An offline build.** Flathub grants no network during a build, so
   `build-args: [--share=network]` has to go and the crate set must be vendored:
   ```bash
   git clone https://github.com/flatpak/flatpak-builder-tools
   python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py \
     app-desktop/vela-wallet/Cargo.lock -o cargo-sources.json
   ```
   then add `cargo-sources.json` to the module's `sources` and build with
   `--offline`. Note that `gpui` and `gpui_platform` are git dependencies on the
   whole Zed repository, so the generated source list is large.

### Before the first public release

Three things are deliberately left as placeholders, because each is a decision
rather than a detail:

| What | Where | Why it is a placeholder |
|---|---|---|
| `LicenseRef-proprietary` | [vela-wallet.spec](packaging/vela-wallet.spec), [metainfo.xml](packaging/app.getvela.VelaWallet.metainfo.xml) | The repository ships no `LICENSE` file. Flathub requires a real SPDX identifier |
| Screenshots | [metainfo.xml](packaging/app.getvela.VelaWallet.metainfo.xml) | The `<screenshots>` block is commented out: `appstreamcli compose` downloads every screenshot during the Flatpak build, and the placeholder `getvela.app/screenshots/…` URLs 404'd — killing the x86_64 CI build with `file-read-error` (the aarch64 one went green on the same dead URLs; do not trust a green compose as proof the URLs work). Restore it with real, resolving URLs before a Flathub submission, which requires at least one screenshot |
| Package signing | — | Both packages are unsigned. `rpm --addsign` and `debsigs` need a release key, the same open question as the Windows code-signing certificate |

### Icons

Every icon in the repository — this app's, both native projects', and the
marketing site's — is rendered from one vector source,
[docs/design/icon/](../../docs/design/icon/):

| Source | Used for |
|---|---|
| `app-icon.svg` | The mark inside its tile. Every platform that draws the icon as-is |
| `app-mark.svg` | The mark alone, transparent. Android's foreground layer, and anything that supplies its own background |
| `app-mark-mono.svg` | Flat white silhouette. Android themed icons, iOS tinted appearance |

Two scripts consume them, and both commit their output so that building a
package needs no image tooling:

```bash
./scripts/generate-desktop-icons.sh          # Linux hicolor + .ico + .iconset
../../scripts/gen-app-icons.sh               # app-ios, app-android, getvela.app
```

Four platform rules are encoded in those scripts, and every one of them fails
*quietly* — the icon simply looks wrong somewhere nobody tested:

- **Windows takes the icon from the executable, not from a window API.** gpui
  calls `LoadImageW(module, MAKEINTRESOURCE(1), IMAGE_ICON, …)` and the call
  site is `load_icon().unwrap_or_default()`, so a miss is silent. [build.rs](build.rs)
  embeds the `.ico` as **resource id 1** specifically.
- **macOS needs a bundle**, and the artwork must carry its own rounded corners
  and padding — Apple's grid puts the tile at 824 of 1024 units, which is the
  `macos_inset` in the icon script.
- **GNOME's app grid asks for 96px**, which is why `96x96` is in the size list.
  Without it the lookup falls through to `scalable/`.
- **gdk-pixbuf identifies an SVG by sniffing its first few hundred bytes.** The
  canonical SVG keeps `<svg>` on line 2 for that reason; a comment above the tag
  pushed it to byte 723 once, and the file stopped being recognised as an image
  at all.

---

## Build & run

```bash
cd app-desktop/vela-wallet
cargo run
```

The window opens at **1280×800** — the mocks' logical size, which is also the
minimum. Wider windows flex the card grid; the action panel keeps its width.

---

## Sign in during development

Two ways, and they are not interchangeable (spec 038):

**The parallel space — no authenticator at all.** The real app with one
substitution: where a passkey would sign, `vela-core`'s fixed keyset signs.
Creating and signing in work end to end, the address is the same golden Safe
the web's parallel space founds, and a badge marks every screen.

```sh
VELA_PARALLEL_SPACE=1 cargo dev          # = cargo run --features dev-fixtures
```

**The signed bundle — the real ceremony.** macOS hands the platform
authenticator ("This device": Touch ID / iCloud passkeys) only to a process
with an application identifier: a signed `.app` with a bundle id, the
associated-domains entitlement and, under a team signature, an embedded
provisioning profile. A bare `target/debug/vela-wallet` has none of these,
and `ASAuthorization` refuses it with *"The calling process does not have an
application identifier"* — the sheet now says so instead of blaming your
fingerprint. Build and run the bundle instead:

```sh
VELA_SIGN_IDENTITY="Apple Development: …" \
VELA_PROVISION_PROFILE=path/to/VelaWallet.provisionprofile \
./scripts/build-macos-app.sh --no-dmg
open "dist/macos/$(uname -m)/Vela Wallet.app"
```

The phone (caBLE) and USB security-key methods need neither: the phone runs
the ceremony itself, and the USB key is driven by this app's own CTAP client.

Other switches, all read at startup: `VELA_INTRO=1` shows the first-run intro
again, `VELA_TEST_PANIC=1` panics once inside the next background task (to
see the failure sheet), `VELA_STATE_DIR=<dir>` points storage somewhere
disposable, `VELA_SKIP_LAUNCH_ANIMATION=1` skips the animation.

## What the first build does

Budget real time for the cold build, and don't interrupt it:

1. **Clones `zed-industries/zed`.** `gpui` and `gpui_platform` are git
   dependencies on the full Zed repository, which is large. This is the slowest
   single step and it is pure network.
2. **Compiles ThorVG from C++.** Hundreds of translation units through `cc`.
3. **Compiles gpui**, then this crate.

`dotlottie-rs` and `thorvg` land in `~/.cargo/git` and are reused across
rebuilds; only the first fetch pays for them. Expect a multi-gigabyte
`~/.cargo` and a `target/` directory to match.

---

## Environment pins

The app follows the system appearance and the system/env locale by default.
Both can be pinned without touching system settings:

```bash
cargo run                                  # system appearance + system locale
VELA_THEME=dark cargo run                  # force dark
VELA_THEME=light cargo run                 # force light
VELA_LANG=de cargo run                     # any of the 15 supported tags
VELA_THEME=light VELA_LANG=zh cargo run    # both
```

| Variable | Read at | Effect |
|---|---|---|
| `VELA_THEME` | [theme.rs:21](src/theme.rs#L21) | `dark` / `light`. Pinning it also **stops** the live restyle on OS appearance change |
| `VELA_LANG` | [loc.rs:20](src/loc.rs#L20) | First entry in the chain `VELA_LANG` → `LC_ALL` → `LC_MESSAGES` → `LANG`. Unrecognized tags fall back to `en` |
| `VELA_SKIP_LAUNCH_ANIMATION=1` | [theme.rs:218](src/theme.rs#L218) | Skips the launch animation outright. For tests and screenshots — the deterministic alternative to sleeping ~1.9 s |
| `VELA_UPDATE_GOLDEN=1` | [launch_animation.rs:869](src/ui/launch_animation.rs#L869) | Rewrites the golden references instead of asserting against them. Review the diff — a change here means the animation changed on this platform |

---

## Tests

```bash
cargo test                       # everything: fit-rule table, texture-lifetime bound, goldens
cargo test golden -- --nocapture # just the golden frames
```

> The quickstart in `specs/012-launch-animation-lottie/` gives the filter as
> `launch_golden`. That matches no test — the real name is
> `golden_frames_match_the_committed_references`, so filter on `golden`.

The golden PNGs live in [tests/golden/](tests/golden/) — five frames per
appearance, at frames 0 / 24 / 45 / 65 / 101. Regenerate with
`VELA_UPDATE_GOLDEN=1`, never by hand.

**The texture-lifetime test is not decoration.** A leak of one GPU texture per
frame is invisible on screen and only surfaces after repeated launches; see
`specs/012-launch-animation-lottie/contracts/desktop-frame-pump.md`.

If you touch the localization corpus, the round-trip runs from the repo root:

```bash
node scripts/gen-i18n.mjs                      # regenerate catalogs + paths.rs
cargo test -p vela-core --features i18n-all    # conformance corpus stays green
```

---

## Known gpui quirks

Three upstream behaviours found while debugging second-display fullscreen and
Dock reopen (2026-08-07, zed pin `c97b7c0`). Deliberately documented here
instead of reported upstream — re-verify each after any gpui bump.

- **`PlatformDisplay::bounds()` discards the display origin on macOS.** Every
  display reports origin `(0, 0)`, so `Bounds::centered(Some(display_id), …)`
  always lands on the primary display no matter which id it is given. To place
  a window on a secondary display, set the bounds origin yourself in global
  top-left coordinates — the primary's top-left is `(0, 0)`, and a display
  arranged above it has a negative `y`.
- **A nil `NSApp.mainMenu` disables the fullscreen titlebar reveal on
  secondary displays.** The hot-edge menu-bar/titlebar reveal never engages
  for a fullscreen Space when the application has no main menu: pushing the
  cursor against the top edge shows nothing, so a fullscreened window on a
  second display has no titlebar, no traffic lights, and no pointer path back
  out. The primary display masks the bug because macOS 26 keeps its menu bar
  visible in fullscreen there. Zed always installs its own menus, which is why
  upstream never trips over this — [main.rs](src/main.rs) sets ours (and F11 /
  ⌃⌘F remain as keyboard exits either way; see
  [onboarding.rs](src/onboarding.rs)).
- **`hasVisibleWindows` stays true after the last window closes, so gpui's
  reopen callback never fires.** AppKit's TextInputUI panel (`TUINSWindow`)
  remains `isVisible` in a gpui process on macOS 26, so when the Dock icon is
  clicked with no real window left the system reports `hasVisibleWindows=YES`,
  and gpui's `should_handle_reopen` gate drops the event before any
  `on_reopen` callback runs. An app that keeps running after its window
  closes is therefore stranded: the Dock icon activates nothing and can only
  Quit. [main.rs](src/main.rs) sidesteps the state entirely with
  `QuitMode::LastWindowClosed` — the same behaviour gpui already defaults to
  on Windows and Linux.

---

## Troubleshooting

| Symptom | Cause |
|---|---|
| `Unable to find libclang` from `bindgen` | Install LLVM and set `LIBCLANG_PATH` to the directory containing `libclang.dll`; see [Windows 11 setup](#windows-11-setup) |
| Windows window cannot move or maximize | Run the current executable and use the titlebar controls or empty band above the Vela mark; see [Window controls](#window-controls) |
| Build dies inside `dotlottie-rs`/`thorvg` with C++ errors or a missing compiler | `gcc-c++` not installed. `cc` alone is not enough |
| `pkg-config` cannot find `wayland-client`, `xkbcommon`, or `fontconfig` | The `-devel` / `-dev` packages are missing; the runtime library alone does not satisfy the build |
| Build succeeds, window never appears, Vulkan error at startup | `vulkan-loader` is present but no ICD. Install `mesa-vulkan-drivers` (or your GPU vendor's driver) |
| The build tries to reach the network on every rebuild | A `tvg-wg` / `tvg-gl` feature crept into the `dotlottie-rs` dependency. See [Cargo.toml:20-24](Cargo.toml#L20-L24) |
| `edition2024` / unstable feature errors | Toolchain older than 1.97.1 |
| Installed package shows a generic icon, or "vela-wallet" instead of "Vela Wallet", in the dock | The window's `app_id` no longer matches the `.desktop` file name. All six places — the five Linux ones plus the macOS `CFBundleIdentifier` — are asserted by the two packaging workflows' `metadata` jobs; see [main.rs:36](src/main.rs#L36) |
| The package installs, then exits at startup with `Library libwayland-client.so could not be loaded` | A `dlopen`'d dependency is missing from the package. See [Runtime dependencies are not all in the ELF](#runtime-dependencies-are-not-all-in-the-elf) |
| `.deb` refuses to install: `libc6 (>= 2.4x) is not installable` | It was linked on a newer distribution than the target. Build it in a `debian:12` container |
| `error: no Debian package is mapped for: <soname>` from the build script | A new shared-library dependency appeared. Find its owner with `apt-file search <soname>` and add a case to `deb_package_for_soname()` |
| `.deb` build: `value "X.Y" for key "Version" ... is not a known version`, `the .desktop file is invalid` | The `.desktop` `Version` key declares a spec version newer than the *validator* in the debian:12 container knows — Debian 12 ships desktop-file-utils 0.26, which predates spec 1.5. Keep `Version=1.0`; the key only names the spec the file follows. Local containers that never installed `desktop-file-utils` skip the validation entirely (the script guards on `command -v`), so this only surfaces in CI |
| Flatpak compiles for minutes, then dies packaging with `Failed to execute child process "eu-strip"` | flatpak-builder splits debuginfo by exec'ing `eu-strip` on the **host**. Ubuntu only *Recommends* elfutils and the CI install uses `--no-install-recommends`, so it must be listed explicitly; Fedora's flatpak-builder pulls it, which is why local Fedora builds never see this |
| Flatpak dies at `appstreamcli compose` with `file-read-error` — possibly on one architecture only | compose rasterizes the scalable SVG icon (it wants a 128x128@2 HiDPI variant no fixed-size directory provides) through the **host's** gdk-pixbuf, and the SVG loader lives in `librsvg2-common`, which nothing declares. The x64 ubuntu-24.04 runner image ships without it while the arm64 image happens to include it — so a green compose on one architecture says nothing about the other. Diagnosed with a 1-minute probe workflow that composes the committed packaging files directly (no app build); reuse that trick for any future compose failure |
| Flatpak build fails inside `cargo build` with `edition2024` errors | The `rust-stable` SDK extension is older than 1.97.1. Check the `rustc --version` line at the top of the build log |
| Flatpak build fails with `failed to resolve address for github.com` | DNS does not work in a sandbox nested inside `org.flatpak.Builder`. See [Name resolution fails when the builder is itself a Flatpak](#name-resolution-fails-when-the-builder-is-itself-a-flatpak) |
| App name appears in the GNOME app grid with no icon, but the dash icon is fine | The grid asks for 96px. If no `96x96` PNG exists the lookup falls through to `scalable/`, and an SVG whose `<svg>` tag sits past gdk-pixbuf's sniff window is not recognised as an image at all. Both are guarded by `scripts/generate-desktop-icons.sh` |
