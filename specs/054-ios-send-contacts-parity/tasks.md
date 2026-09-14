# Tasks — 054 iOS Send & Contacts Parity

**Read this first if you are picking the branch up cold.**

Branch `054-ios-send-contacts-parity`, stacked on `053-ios-dapp-browser-signing`,
in the worktree `/Volumes/data/production/vela-wallet-ios`.

Commands: [053's quickstart](../053-ios-dapp-browser-signing/quickstart.md) —
nothing about the loop changed. Phases: [plan.md](./plan.md). Decisions:
[research.md](./research.md).

---

## Done

### Phase 0 — the papers and the pure pieces ✅ `628f4987` … `a47f08e2`

`spec.md` (FR-001…011, SC-001…010, seven stories), `research.md` (D0–D10),
`plan.md`. Then: `XlsxMatrix` against the desktop's real payroll fixture,
`DocumentPorts` (three verbs), `BatchWire`, `BatchExecutor`, `SplitRows`,
`SweepPick`, and `SendWire` gaining `multi_specs`. 447 hermetic tests.

### Phase 1 — split ✅ `9ee04829`

`SendLive.flowState` can reach sd2b/sd2c/sd2d/sd3b/sd3c. The editable row, the
per-row verdict, the whole-list event, and the two dead buttons.

**Device pass owed.** Everything here is hermetic; a two-row split on the
phone (SC-001) has not been driven.

---

## Next — Phase 2: sweep

- [ ] **T201** `SendPickBody`'s tick boxes: `SweepPick.tap` → `send.sweepTap`,
      and `dimmed` rows **not tappable**.
- [ ] **T202** 发送多个代币 on SD1 enters the pick — it is the **second dead
      button** the survey found, and it needs the shell's own `sweepPicking`
      flag because the core's `multiSelectMode` only flips at confirm.
- [ ] **T203** 全选有价值代币 → `SweepPick.selectAll` with the **visible** ids.
- [ ] **T204** Continue → `confirm_multi_selection`; SD2d reads `multi_specs`,
      not the balances the picker showed.

### Then

3 batch · 4 the two exits · 5 **draw C7/C8/C9** · 6 wire the form · 7 the book
travels · 8 closeout.

---

## What the survey found that is not in any phase yet

Recorded here so it is not lost. Each is a live defect or a dropped judgement:

1. **`ContactDetailScreen`'s edit pencil is a literal no-op** (`Button {}`).
2. **Delete from the contact detail does nothing**, because `ContactsLive.detail`
   passes `sheet: nil`. Delete works only from the C1 row-swipe.
3. **The contacts search field is a `Text`**, so `contacts.query` is permanently
   `nil` and live search cannot work.
4. **`set_group_members` has zero call sites** in `ContactsStore`.
5. **`view.recipient` is decoded and never read** — no `inspect_recipient` is
   ever dispatched, so it is always `nil`.
6. **`import_failure` and `export` are not decoded at all.**
7. `sections` is not decoded **on purpose**: iOS derives its own A–Z with CJK
   transliteration, and that divergence is recorded in 050.
8. `BatchImportBody`'s four callbacks are not passed by `FlowHost`, and its
   paste box is a read-only `Text`.
9. `SendFormBody.onMax` drops the sweep row index (`{ _ in onMax?() }`).

---

## The traps

Everything in [research.md](./research.md) D4–D10, plus:

1. **`FlowHost` is one argument from a type-checker timeout.** Extract every
   new closure into a named method on `RootView` (052's lesson, hit again in
   phase 1).
2. **A hand-written `SendViewWire` fixture breaks when the wire grows** —
   which is the gate working. `MoneyPlumbingTests` has one.
3. `BatchUnit` is shared between the wire and the drawn model on purpose: two
   cases, two identical spellings, and the drift test would catch a divergence
   before a screen could.
