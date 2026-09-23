# Vela Design Language — "Quiet, typographic, de-containered"

The confirmed visual language for the whole app (Apple Wallet / Wise register). Every
screen — and every future change — MUST follow it, on all four shells. Reference screen:
the **wallet home**.

The principles are the contract; the component and API names below are named where they live in
the web shell (`app-web/vela-wallet/src/lib/`), and each of the other shells has its own
equivalent. Where a name was left over from the retired Expo tree it has been corrected.

## Principles

1. **De-container.** Do NOT box each element in its own card. Content sits directly on
   the page (`color.bg.base`), grouped by **space + a `SectionLabel` + hairline
   `Divider`** — never by wrapping every row/section in a bordered/shadowed card.
   - A "card pile" (stacked white rounded panels) is the anti-pattern we removed.
   - Cards are reserved for genuinely distinct surfaces: `AppModal` sheets, a
     deliberate warning/confirm gate, a *selected* option. When you keep one, keep it
     light (hairline, no heavy shadow).

2. **Open heroes/headers.** Balance, screen headers, and section intros are open on the
   page (no card). Group with a `SectionLabel` above and whitespace below.

3. **Hairline dividers.** Between de-boxed rows use a 1px `color.border.base` line,
   **inset past the leading icon** so it aligns under the text (Apple-Wallet style).
   Reuse the shell's `Divider` (the one `DetailRow` draws), or a 1px rule inset by the
   icon width plus its gap.

4. **Section labels.** Use `SectionLabel` (uppercase, letter-spaced, `fg.subtle`, small) —
   `app-web/vela-wallet/src/lib/settings/ui/SectionLabel.svelte` and its per-shell twins.
   Not bold black headings.

5. **Subordinated symbols.** In big amounts, the currency symbol is smaller than the
   number (the number is the hero). Use `AmountText` with `symbolScale` (~0.58).

6. **Light controls, not heavy boxes.** Tabs = transparent track + a single floating
   active chip (`SegmentedToggle`). Filters/pills = soft `bg.sunken` chips (`radius.full`),
   no borders. No chunky filled control boxes.

7. **Plain icon buttons.** Header/settings/close icons have **no** card bg/border/shadow —
   just the icon, with a ≥44×44 hit target (size or `hitSlop`).

8. **Restraint.** No decorative blobs/glows/gradients unless whisper-subtle and on-brand.
   Single accent (`#E8572A`) reserved for CTAs and truly-primary actions. Warm/light,
   low contrast with the page.

9. **Tokens only.** All values from the generated token layer — `tokens.css` / `tokens.ts` on
   web, `Tokens.swift` on iOS, and so on — whose one source is
   [`docs/design-tokens.json`](design-tokens.json) (`color.* space.* text.* radius.* weight.*
   leading.*`). Never hardcode hex/px; literals are test-banned in product UI. Must work in
   **light AND dark** (uses `color.*`).

10. **Entrances play once.** `entering` (fadeIn/fadeInDown) must not replay on re-render
    (gate with a `hasEntered` ref) — else the screen "flickers/slides" on state updates.

## Accessibility is not optional (already in place — keep it)

Every pressable carries its role and a **translated** accessible name, and a selected control
says it is selected — in each platform's own idiom (`aria-*` and real `<button>` elements on the
web, `accessibilityLabel`/`accessibilityAddTraits` on iOS, `contentDescription`/`semantics` on
Android). ≥44×44 targets; a visible keyboard focus ring (web `:focus-visible`); sheets and
dialogs trap focus and close on Escape.

## Shared primitives to reuse (don't reinvent)

`SectionLabel`, `Divider` (DetailRow), `AmountText` (`--amount-symbolScale`, 0.58),
`SegmentedToggle` (the **only** segmented control — no second dialect), `VelaButton` (CTA),
`DetailRow`, and the sheet/dialog family (`Sheet.svelte`, `ConfirmSheet.svelte`,
`Dialog.svelte` — the old `AppModal`/`AppAlert` names are gone). On the desktop a sheet becomes a
third column or a `Dialog`, never a bottom sheet.
