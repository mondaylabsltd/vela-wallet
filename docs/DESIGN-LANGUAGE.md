# Vela Design Language — "Quiet, typographic, de-containered"

The confirmed visual language for the whole app (Apple Wallet / Wise register). Every
screen — and every future change — MUST follow it. Reference screen: **HomeScreen**.

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
   Reuse `<Divider/>` (from `DetailRow`) or a `sep` style `{height:1, backgroundColor:
   color.border.base, marginLeft: <icon+gap>}`.

4. **Section labels.** Use `<SectionLabel>` (uppercase, letter-spaced, `fg.subtle`,
   small) — `src/components/ui/SectionLabel.tsx`. Not bold black headings.

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

9. **Tokens only.** All values from `theme.ts` (`color.* space.* text.* radius.*
   inter.*`). Never hardcode hex/px. Must work in **light AND dark** (uses `color.*`).

10. **Entrances play once.** `entering` (fadeIn/fadeInDown) must not replay on re-render
    (gate with a `hasEntered` ref) — else the screen "flickers/slides" on state updates.

## Accessibility is not optional (already in place — keep it)

Every pressable: `accessibilityRole="button"` + a translated `accessibilityLabel`;
selected controls expose `accessibilityState={{selected}}`; ≥44×44 targets; keyboard
focus ring (web `:focus-visible`); modals trap focus + close on Escape (`useWebDialog`).

## Shared primitives to reuse (don't reinvent)

`SectionLabel`, `Divider` (DetailRow), `AmountText` (symbolScale), `SegmentedToggle`
(light), `VelaButton` (CTA), `AppModal`/`AppAlert` (sheets), `DetailRow`.
