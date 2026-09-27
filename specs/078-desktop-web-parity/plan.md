# Plan: the desktop wallet is the web wallet (078)

## Approach

1. **Foundations first** (research X-01…X-06). Most "looks like a defective
   copy" findings share six causes — two missing colours and a wrong dark
   sunken, buttons at 37px/13px instead of 52px/17px, a third column that
   cannot scroll and has the wrong chrome, dialogs without scrim/shadow/
   Escape, search fields that are pictures, copy buttons that do nothing.
   Each is fixed once, in the shared primitive, and every surface that uses
   it moves with it.
2. **P0 behaviour next** — anything wrong or unsafe: the slide that is a
   click (G-01), a refused request with no way out (G-03), deletes without a
   question (C-01), custom-group rows opening the wrong page (E-02), the
   hero's decimal split (H-07), Max leaving Continue stuck (M-01).
3. **P1 missing features by how often they are hit**: account switcher and
   identicon viewer (H-01/02), status line (H-03), scan from send + scanner
   (F-01…03), receipts (F-04, G-04), day headers (H-04), search (X-05 users),
   copy feedback, then the rest of P1 in research order.
4. **P2 visuals** surface by surface, each verified by a screenshot.

## Where the work lives

- Theme: `D theme.rs` (colours, type scale helpers).
- Primitives: `D flows/components.rs` (buttons, search, fact rows, status
  hero), `D ui/` (fields, scrollbar, a new `dialog` and `copy_button`),
  `D wallet/components.rs` (rows, header).
- Surfaces: `D wallet/page.rs` (panel scaffold, dialogs, wiring),
  `D flows/panels.rs`, `D contacts/`, `D settings/`, `D explore/`,
  `D signing/`.
- Core, for M-01…M-03 only: `rust/crates/vela-core/src/app/send.rs`, with
  tests in `tests/app_send.rs` — every shell benefits.

## Verification

- The desktop suite and clippy (no new warnings) on every commit.
- Screenshots: a separate desktop build (`CARGO_TARGET_DIR` outside the
  tree, so it never fights the owner's running `cargo run` for the exe),
  its gallery (`VELA_GALLERY=1 VELA_GALLERY_STATE=n`) or live window
  captured with `System.Drawing`, next to the web's gallery
  (`/en/gallery/<state>`) in headless Chrome at the same size. The web dev
  server needs `pnpm@10.11.1 install` (the machine's pnpm 8 cannot read the
  lockfile; the first attempt died on a network reset — retried).
- A commit per research ID group, its message naming the web file it
  matches.

## Risks

- `page.rs` is ~14k lines and most fixes touch it: work serially, small
  commits, no parallel editors on it.
- Dark `bg_sunken` #262622 → #0F0F0D changes every surface that used it as a
  hover or chip fill in dark; checked against the web in dark, not assumed.
- Removing desktop-only extras (money-in toast, row glow, assets empty state,
  the Trusted Signer section in Settings) is a product call — kept unless
  the owner says otherwise.
