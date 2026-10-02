# Feature Specification: 091 — Debug mode: the wallet for http dApps on the local network

**Feature Branch**: `091-debug-mode-lan-dapps` | **Created**: 2026-10-02 | **Status**: Implemented (see [results.md](results.md))
**Input**: owner ruling 2026-10-02, a follow-up to spec 088. Spec 088 stopped offering the wallet to plain-http
pages on the local network: such a page may load in the in-app browser, but it gets no provider (only `https`
and exact loopback are secure contexts). The owner: "consider supporting injection when debug mode is on", and
chose a **hidden entry**. In Settings → About, tapping the version 7 times reveals a "debug mode"
(zh 「调试模式」) switch, like Android's developer options. Ordinary users never see it.

**Second ruling, same day:** "正式版应该都是禁止的吧，只有调试开发的时候能就行". Store builds forbid it
entirely. Debug mode exists only in developer builds, the ones that carry the parallel space: the Android
`debug` variant, the iOS `Debug` configuration, and the desktop's `dev-fixtures` builds. A store build reveals
nothing on taps, draws no row, ignores any stored value, and offers the wallet exactly as spec 088 does.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A developer reveals the switch (Priority: P1)

In a developer build, a developer opens Settings → About and taps the version line 7 times in a quick run. A short notice says debug
mode is now available, and a "Debug mode" row with a switch appears in About. The row stays there across
launches, so the switch can be turned off again. Nobody who has not done this sees it.

**Independent Test**: on each shell, tap the version 7 times. The notice shows once, and the row appears.
Relaunch the app, and the row is still there with the switch where it was left.

**Acceptance Scenarios**:
1. **Given** a fresh install, **When** About opens, **Then** no debug-mode row is drawn.
2. **Given** About, **When** the version is tapped 7 times, each within 1 s of the one before, **Then** the
   notice "Debug mode is now available" shows once, and the row appears with the switch off.
3. **Given** 6 quick taps and then a pause longer than 1 s, **When** the next tap comes, **Then** the count
   starts again at 1.
4. **Given** the row is revealed, **When** the version is tapped again, **Then** nothing happens.
5. **Given** the device is erased (Settings → Storage → erase), **When** About opens, **Then** the row is
   hidden again, and debug mode is off.
6. **Given** a store (release) build, **When** the version is tapped any number of times, or `vela.debugMode`
   is stored `on` (left by a developer build), **Then** nothing is revealed, no row is drawn, and debug mode
   is off. The browser behaves exactly as in spec 088.

### User Story 2 — With debug mode on, a LAN http dApp gets the wallet (Priority: P1)

In a developer build with the switch on, the in-app dApp browser (iOS, Android, desktop) offers the wallet to `http` pages on the
device's own network: RFC 1918 IPv4 (`10/8`, `172.16/12`, `192.168/16`), link-local (`169.254/16`), `.local`
names, IPv6 unique-local (`fc00::/7`) and link-local (`fe80:`). Hosts are matched exactly, never by a name
that only starts with digits.

**Acceptance Scenarios**:
1. **Given** debug mode on, **When** `http://192.168.1.5:3000` loads, **Then** the page has the provider, and
   it can connect and sign.
2. **Given** debug mode on, **When** `http://10.0.0.1.evil.com`, `http://192.168.1.5.nip.io` or any public
   http page loads, **Then** it gets no provider, and a hand-made call to the bridge is ignored. This is
   exactly the behaviour without debug mode.
3. **Given** debug mode off (the default), **When** a LAN http page loads, **Then** spec 088 applies
   unchanged: no provider, and no connection shown for it.

### User Story 3 — The setting changes while a page is open (Priority: P2)

**Acceptance Scenarios**:
1. **Given** a LAN page using the wallet, **When** debug mode is turned **off**, **Then** the wallet is
   withdrawn at once. Every request the page had open is answered 4900, a signing sheet showing one of them
   closes, and the page's later messages are ignored. Other tabs are untouched.
2. **Given** a LAN page open without the wallet, **When** debug mode is turned **on**, **Then** nothing
   changes for that page until it loads again (reload or navigation). A WebView reads its injected script
   when a document starts.
3. **Given** the desktop, which has one webview built with its scripts, **When** debug mode changes, **Then**
   the view is rebuilt at the page it showed, the next time the browser is drawn.

### Edge Cases

- Tricky hosts: `10.0.0.1.evil.com`, `192.168.1.5:3000`, `[fd00::1]`, `foo.local`, `foo.local.evil.com`,
  `local`, `172.32.0.1`, `[2001:db8::1]`, `[::ffff:192.168.1.5]`, `999.1.1.1`, and shorthand such as
  `http://192.168.1`, which the URL parser normalises before either side sees it.
- A stored value this build does not write (`ON`, `true`) reads as hidden, and hidden is off.
- A clock that went backwards between two taps starts the count again.
- A LAN site connected in debug mode shows no connection while debug mode is off. Its grant stays listed in
  Settings, so it can still be removed.

## Requirements *(mandatory)*

- **FR-001** The core decides once whether an origin is offered the wallet:
  `dapp_permissions::offers_wallet(origin, debug_mode)`. Debug mode off is exactly `is_secure_context` (spec
  088). Debug mode on adds `http` whose host passes the existing exact private-host rule.
- **FR-002** The in-app browsers' page gate (`dapp_browser`) asks `offers_wallet` with the debug mode the shell
  stated in `Event::DebugModeChanged { on }`. Turning it off retires every open document that is no longer
  offered.
- **FR-003** The injected script (`dapp_rpc::provider_script(host, debug_mode)`) follows the same rule. With
  debug mode off it is byte-identical to spec 088. With debug mode on, it gates on `window.isSecureContext`, or
  on `http:` plus a host test that the core writes out of the same tables (`private_host_js`). No shell has a
  copy of that test. A web test runs the generated JS against the Rust rule on a table of hosts.
- **FR-004** The setting is stored with the other preferences: `vela.debugMode`, `off` / `on`, absent = hidden
  (`prefs::DebugMode`). The 7-tap rule is the core's (`prefs::version_tapped`: 7 taps, each ≤ 1,000 ms after
  the one before).
- **FR-004a** Developer builds only (second ruling). The core reads the setting and counts taps with the
  shell's build fact: `prefs::debug_mode(entries, developer_build)` and `version_tapped(…, developer_build)`.
  Outside a developer build, the mode is always hidden (off) and no tap reveals. Each shell passes only that
  fact: Android `BuildConfig.DEBUG`, iOS `#if DEBUG`, desktop `cfg!(feature = "dev-fixtures")`. The last is
  the same compile-time gate as `parallel_space::active`.
- **FR-004b** Android cleartext: the release build stays as it is, with no network security config, so all
  cleartext is refused. Only the debug source set's config allows cleartext (`base-config`), so a LAN http
  page can load in a debug build. A source-set test pins both.
- **FR-005** Every in-app browser follows FR-001 to FR-003. When the setting changes, each open tab installs the
  new script for its next document (iOS `WKUserScript`, Android `addDocumentStartJavaScript`). The desktop
  rebuilds its one webview.
- **FR-006** The hidden entry and the switch are on iOS, Android and desktop (developer builds). The switch's description is one
  short line: what it does, and that it is for development only. Copy is in the corpus, in all 15 locales.
- **FR-007** Web and the Chrome extension: the web wallet has no in-app browser, and the extension never
  stopped offering the wallet to LAN http pages (088 did not touch it). So neither gets the switch. See
  [plan.md](plan.md) D7.

## Success Criteria

- **SC-001** Core: an origins table (incl. the tricky hosts) for `offers_wallet` both ways. Machine tests for
  the gate, for turning off with an open LAN page, and for turning on. Tests for the 7-tap rule and the stored
  value.
- **SC-002** The generated JS host test agrees with the Rust rule on every host in the table (web suite).
- **SC-003** Each shell: tests for its wiring (script per mode, swap on change, stored setting, tap → reveal),
  and a screenshot of About with the switch revealed.
- **SC-004** Both build kinds are tested in the core and in each shell: a store build never reveals, and
  reads a stored `on` as off.
