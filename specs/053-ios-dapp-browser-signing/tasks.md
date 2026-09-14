# Tasks — 053 iOS dApp Browser and Signing

**Read this first if you are picking the branch up cold.** It is written to be
enough on its own: the commands, what is done, what is next, and the traps.

Branch `053-ios-dapp-browser-signing`, stacked on `052-ios-money-wiring`, in
the worktree `/Volumes/data/production/vela-wallet-ios`.

The commands are in [quickstart.md](./quickstart.md). The six phases are in
[plan.md](./plan.md). The 28 operations are in
[contracts/shell-operations.md](./contracts/shell-operations.md). The channel
is [contracts/page-envelope.md](./contracts/page-envelope.md).

---

## Done

### Phase 0 — the papers

[spec.md](./spec.md) (FR-001…FR-018, SC-001…SC-014, six user stories),
[plan.md](./plan.md), [research.md](./research.md) (D0–D15 plus the ten
inherited Android device defects), the two contracts,
[quickstart.md](./quickstart.md), [checklists/requirements.md](./checklists/requirements.md).

Baselines in [results.md](./results.md).

---

## Next — Phase 0 code: the wires, the drift, the provider bundle

- [ ] **T001** `Features/Explore/Core/{ExploreWire,BhistWire,DpermWire}.swift`
      and `Features/Signing/Core/{SignWire,ClearWire,GuardWire}.swift`. Views
      `Decodable` through `CoreJSON.decoder`; **operations and results stay
      dictionaries** — the house rule in every existing Wire header.
- [ ] **T002** Read numeric types from the **Rust struct**, not a TS mirror.
      `u32 → Int`, `f64 → Double`, error codes `i32`. 051 lost a chain index to
      a `u32` overflow doing this the other way.
- [ ] **T003** Six `CoreWireDriftTests` cases, one per machine: instantiate the
      `*Core()`, round-trip `core.view()` through the wire.
- [ ] **T004** `app-ios/scripts/bundle-provider.sh` + the two `.xcfilelist`s +
      the `PBXShellScriptBuildPhase`. Model it on `bundle-animations.sh`;
      `bundle-catalogs.sh` cannot be reused (its corpus rule aborts on an empty
      match).
- [ ] **T005** `ProviderBundleTests`: the bundled bytes equal the web tree's,
      read through `#filePath`.
- [ ] **T006** `Features/Explore/Core/DappRpc.swift` + `DappRpcParityTest`
      reading `extension/lib/protocol.js`. **Ship 4900 for unsupported**, not
      the 4200 the contract docs say — the code is the authority and three
      clients agree.

### Then

Phase 1 the engine · 2 memory · 3 connect · **4 sign (the gate)** · 5 guard and
message · 6 closeout.

---

## The traps, inherited and new

Everything in [research.md](./research.md)'s last section is a device-found
Android defect that will recur here. The iOS-specific ones:

1. **`WKUserScript` defaults to an isolated content world.** A
   `window.ethereum` defined in `.defaultClient` is invisible to the page.
   Both scripts, the message handler and every `evaluateJavaScript` must name
   `.page`.
2. **`WKScriptMessage.webView?.url` is the origin source**, read on the main
   thread at message time. Not a navigation callback (it has not fired yet),
   not `frameInfo.securityOrigin` (correct, but spelled differently from
   `dappOriginOf`, and a grant keyed differently is a grant that does not
   apply).
3. **`app.webViews` in XCUITest sees only painted text.** The test page prints
   `#verdict` lines for this reason.
4. **`file://` has no origin** under `dappOriginOf`, so the harness is served
   over loopback.
5. **A `UIViewRepresentable` that outlives its container leaks the web view
   onto another screen.** FR-013.
6. Everything 052 learned still applies: `CoreStore.onFault` is silent without
   a logger; an event before `boot()` is dropped; a `var onRefresh: (() async
   -> Void)?` stored in a `View` segfaults AttributeGraph; one automation
   session at a time.

---

## Deliberately not in this cut

The camera scanner and the simulation block (055). `eth_sign` depth and the
SIWE phishing panel's copy (055) — here `eth_sign` is refused. Split, sweep,
the payroll importer, contacts I/O (054). `dapp_session` and `ext_cache` (both
ruled out). `dapp_permissions::PopupRequest` — an extension-window shape iOS
does not have; the arm stays undispatched and the reason is recorded.
