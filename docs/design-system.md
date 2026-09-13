# Vela Wallet — AI Design System Brief

Use this brief whenever you design, review, or generate UI for Vela Wallet.

## Source of truth

- The Penpot file **Vela Wallet** is the visual source of truth.
- Use its local token sets, never literal replacement values:
  - `core` — foundational tokens; always active.
  - `mode-light` — light semantic colors; activate for light UI.
  - `mode-dark` — dark semantic colors; activate for dark UI.
- Activate `core` plus **exactly one** mode set. Never enable both modes.
- This document explains token intent; Penpot remains the authority for current values.

## Non-negotiable rules

1. Do not hard-code color, spacing, radius, font size, weight, shadow, opacity, or motion values in product UI.
2. Prefer semantic tokens (`color.fg.base`) over visual descriptions or hex values.
3. Use a complete `typography.*` token for text before setting individual type properties.
4. Keep the orange accent for primary value-moving actions, not routine navigation or decoration.
5. Preserve accessible contrast in both `mode-light` and `mode-dark`.
6. Before handing off a design, list the tokens used for its key surfaces, text, and actions.

## Work in this order

1. Select the surface mode: `mode-light` or `mode-dark`.
2. Choose semantic colors by role.
3. Build geometry from the spacing, sizing, dimension, and radius scales.
4. Apply typography roles and allow the user text-scale setting to resolve size.
5. Add only the named opacity, shadow, and motion tokens needed by the interaction.

## Color roles

| Need | Use |
| --- | --- |
| Page background | `color.bg.base` |
| Raised card, sheet, or field surface | `color.bg.raised` |
| Recessed area | `color.bg.sunken` |
| Main heading, value, or body copy | `color.fg.base` |
| Supporting label or helper copy | `color.fg.muted` |
| Quiet copy | `color.fg.subtle` |
| Divider or default input border | `color.border.base` |
| Stronger border / selected boundary | `color.border.strong` |
| Primary transfer, confirm, send, connect action | `color.accent.base` |
| Quiet brand surface or secondary action treatment | `color.accent.soft` |
| Confirmed / successful outcome | `color.success.base` and `color.success.soft` |
| Pending / caution state | `color.warning.base` and `color.warning.soft` |
| Destructive or failed state | `color.error.base` and `color.error.soft` |
| Neutral information | `color.info.base` and `color.info.soft` |

### Brand color guardrail

`color.accent.base` is Vela’s Sunset Orange (`#E8572A` in the current token source). Use it sparingly: the user’s most important financial action, a focused state, or a small brand accent. It should not become the default color for links, every icon, or every button.

## Layout tokens

Use the named rhythm; do not create intermediate gaps.

| Token | Value | Typical use |
| --- | ---: | --- |
| `space.0` | 0px | Reset / no gap |
| `space.xs` | 2px | Hairline offset |
| `space.sm` | 4px | Icon-to-label |
| `space.md` | 8px | Tight stack |
| `space.lg` | 12px | Field internals |
| `space.xl` | 16px | Card padding |
| `space.2xl` | 20px | Section stack |
| `space.3xl` | 24px | Section gap |
| `space.4xl` | 32px | Screen section |
| `space.5xl` | 48px | Major break |

- Radius: `radius.none`, `radius.xs`, `radius.sm`, `radius.md`, `radius.lg`, `radius.xl`, `radius.full`.
- Controls: use `sizing.control.sm` (36px), `sizing.control.md` (44px), or `sizing.control.lg` (52px). Prefer 44px or larger for touch targets.
- Platform dimensions: use `dimension.tabBar.ios` (50px), `dimension.tabBar.android` (80px), and `dimension.max.content` (800px) where applicable.

## Typography and localization

- UI stack: `font.ui` — **Plus Jakarta Sans** for Latin plus **Noto Sans SC** for CJK fallback.
- Technical stack: `font.mono` — **JetBrains Mono** for addresses, code, IDs, and numeric diagnostics.
- Use `typography.label`, `typography.body`, `typography.button`, `typography.title`, `typography.display`, `typography.numeric`, and `typography.mono` by intent.
- Keep the same type role in English and Chinese. Do not introduce another CJK display font.
- Text scale is user-controlled and persistent. Support all six levels:
  `compact` 0.82×, `small` 0.91×, `standard` 1.00×, `comfortable` 1.10×, `large` 1.22×, `xlarge` 1.35×.

## Effects and engineering tokens

- Opacity: use `opacity.disabled`, `opacity.low`, `opacity.medium`, or `opacity.high`.
- Elevation: use `shadow.sm`, `shadow.md`, or `shadow.lg` only on raised surfaces; borders still define structure.
- Motion: use `motion.fast` (150ms), `motion.base` (250ms), or `motion.slow` (400ms).
- Use `border.width.base`, `tracking.none`, `text.case.*`, `text.decoration.*`, and `rotation.*` instead of manual values.

## Brand mark and app icon

There are two distinct assets. Do not mix their rules.

1. **In-app mark** — transparent sailboat mark used in headers and browser UI.
   - Light UI hull: Warm Graphite `#554B46`.
   - Dark UI hull: Dusk Ivory `#DED5CE`.
   - Preserve the orange main sail and peach secondary sail.
   - Do not redraw, recolor, or add an app-icon background in a UI header.

2. **Fixed home-screen app icon** — the same asset on iOS and Android, regardless of system appearance.
   - Background: Sunset Orange `#E8572A`.
   - Main sail: Porcelain `#FFF3EC`.
   - Secondary sail: Peach `#FFC6B0`.
   - Hull: Harbor Umber `#5A4037`.
   - Never generate separate light and dark home-screen icon variants.

Browser favicons are transparent and use the in-app mark rules for the active browser appearance; they do not use the home-screen icon background.

## Required AI handoff format

When proposing a screen or component, include a short token map after the design:

```text
Mode: core + mode-light
Surface: color.bg.base / color.bg.raised
Text: typography.title / typography.body / color.fg.base / color.fg.muted
Layout: space.xl padding / space.3xl section gap / radius.lg
Action: color.accent.base / typography.button / sizing.control.lg
Effects: shadow.sm / motion.base
```

If a needed token does not exist, say so explicitly and propose a semantic token name and its intended role. Do not create a hidden one-off value.

## Inherited rules (merged from the retired `DESIGN_SYSTEM.md`, 2026-09-11)

The Expo-era design system document was folded into this brief when that app
was retired (spec 039). Everything React Native-specific in it (`createStyles`,
`withSpring`, `Pressable`, `lucide-react-native`) is gone; the rules below are
the platform-neutral ones every shell still follows, kept here so the code
comments that cite them keep pointing at a living document.

### Principles

- **Warm precision.** A warm neutral palette with one bold accent; every
  element sits on the 4px spacing grid (`space.*`), never in between.
- **Depth through shadow, not border.** Raised surfaces use `shadow.sm`
  (`shadow.md` for hero/elevated cards); borders are for inputs and dividers,
  not for separating areas.
- **Motion with purpose.** Every animation does a job: a press confirms the
  element is interactive, an entrance establishes reading order, a pulse
  signals ongoing activity. Never decorate. Nothing longer than
  `motion.slow` (400ms) except the launch animation.
- **Icons over text** where the meaning is universal (back, close); text
  where the action has consequences ("Confirm & Send").

### Motion

| Context | Animation | Config |
| --- | --- | --- |
| Button press | scale to 0.97 | spring (each shell's native spring; the damping is mirrored across shells) |
| List-row press | scale to 0.98 | spring |
| Screen content entry | fade + rise | 300–400ms |
| Staggered sections | fade + rise with delay | delay = N × 50ms |
| Status indicator | opacity pulse | 0.3 ↔ 1.0, 800ms each way |

Rules: springs for interactive feedback, never a plain timing curve; at most
two properties animate on one element at once; an entrance completes within
500ms including its delay; no bounce or elastic easing on content.

### Button states

- **Pressed**: the scale above plus the platform's haptic where it has one;
  a press must always be felt.
- **Loading state**: a busy button turns a spinner where its label was and
  keeps its size and colour — busy is not disabled. Dimming means "cannot be
  used", and the two must never look alike.
- **Navigation buttons never show a loading state**; only value-moving
  actions do.

### Screen patterns

- **Navigation bar**: `[ back/close ]  [ title ]  [ spacer as wide as the left ]`
  — the title is `typography.title`; small nav targets get a generous hit
  area (44px control or an 8px slop).
- **Section header**: uppercase `typography.label` in `color.fg.subtle`,
  letter-spaced, optional action on the right.
- **Form field**: uppercase label above; input on `color.bg.sunken`,
  `radius.lg`, 1px `color.border.base`, `space.xl` padding; helper actions
  (Scan, MAX) beside the input at `space.md`.
- **Empty state**: an icon in a 56px sunken circle, a title, one or two
  sentences in `color.fg.muted`, then the call to action.
- **Confirmation card**: a raised card of label/value rows separated by 1px
  dividers — label muted, value semibold.

## Haptics (spec 048, the founder's rule of 2026-09-12: 很多地方应该加，但不能泛滥)

Vibration marks what the eye cannot confirm and what really changed. Nothing else.

| Class | When | Android constant |
|---|---|---|
| Press | a button under the finger (press = deformation + haptic, the standing rule) | VIRTUAL_KEY |
| Detent | a slider step crossed, a picker snapping, the signing slider's threshold, the index rail's letters | SEGMENT_TICK (34+) / CLOCK_TICK |
| Select | a selection that takes effect: a switch, a class or network filter, a network / fee-token / account pick, a favourite, a copy | CONFIRM (30+) / KEYBOARD_TAP |
| Success | an outcome the core decided: money sent, money arrived | the two-beat pattern (`Haptics.success`) |
| Reject | an outcome the core decided: a refusal | the heavy click (`Haptics.error`) |

Never: scrolling, a row tap that navigates, a tab switch, Back, a sheet opening or closing, typing, an animation, or a value the app changed by itself.

Three hard rules: one per gesture; none on a control that also navigates; no repeat inside the same step while dragging.

One helper — `VelaHaptic` + `rememberVelaHaptic()` — through `View.performHapticFeedback`, so the phone's own haptics switch is honoured; every call leaves a `haptic <class>` log line so the scripted device pass can count them. `Haptics.success/error/moneyIn` remain the core-decided outcomes.

