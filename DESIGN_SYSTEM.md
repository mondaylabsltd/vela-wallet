# Vela Wallet Design System

Design principles and implementation rules for maintaining visual consistency across all screens and future features.

---

## 1. Core Principles

### 1.1 Warm Precision
Vela uses a warm neutral palette (`#FAFAF8` base, `#F5F3EF` sunken) with a single bold accent (`#E8572A`). Every screen should feel warm and approachable, but every element must be precisely placed on the 4px spacing grid.

### 1.2 Depth Through Shadow, Not Border
Cards use subtle shadows (`shadow.sm` by default, `shadow.md` for hero/elevated elements) to create depth. Borders are reserved for input fields and dividers, not for creating visual separation. Use `VelaCard elevated` for elements that need to stand out.

### 1.3 Motion With Purpose
Every animation must serve a function:
- **Spring scale (0.97x)** on press: confirms the element is interactive
- **FadeInDown** on screen entry: establishes reading order
- **Pulsing opacity** on status indicators: signals ongoing activity
- Never animate for decoration. Never exceed 400ms duration for transitions.

### 1.4 Icons Over Text
Use Lucide icons to communicate actions and states whenever the meaning is universally clear. Reserve text labels for ambiguous actions or primary CTAs. Example: use `<ArrowLeft>` instead of "Back" text, use `<X>` instead of "Close" text, but keep "Confirm & Send" as text because the action has consequences.

---

## 2. Typography

### 2.1 Font Zones

| Zone | Font | Usage |
|------|------|-------|
| `font.sans` | System | All UI text: labels, buttons, body, nav |
| `font.display` | SF Rounded (iOS) / System (Android) | Hero numbers: total balance, token balance on detail |
| `font.mono` | Menlo (iOS) / monospace (Android) | Addresses, hashes, contract data |
| `font.numeric` | System | Token balances and USD values in lists (tabular alignment) |

### 2.2 Size Scale

Sizes are scaled by user preference via `createStyles()`. Base values:

| Token | Base px | Usage |
|-------|---------|-------|
| `text.xs` | 10 | Badges, timestamps |
| `text.sm` | 11 | Section labels, secondary info, chain names |
| `text.base` | 13 | Body text, form labels, list items |
| `text.lg` | 15 | Row titles, button text, token symbols |
| `text.xl` | 17 | Screen titles, nav titles |
| `text['2xl']` | 20 | Page titles (Settings, token name) |
| `text['3xl']` | 26 | Step titles (Send flow) |
| `text['4xl']` | 32 | Hero balance on token detail |
| `text['5xl']` | 40 | Reserved for splash/onboarding |

### 2.3 Weight Rules

- `weight.regular` (400): Body text, hints, secondary values
- `weight.medium` (500): Mono text (addresses), form values
- `weight.semibold` (600): Row titles, button labels, section headers
- `weight.bold` (700): Page titles, hero numbers, brand text

---

## 3. Color

### 3.1 Foreground Hierarchy

| Token | Hex | Usage |
|-------|-----|-------|
| `color.fg.base` | `#1A1A18` | Primary text, icons |
| `color.fg.muted` | `#7A776E` | Secondary text, descriptions |
| `color.fg.subtle` | `#B0ADA5` | Tertiary text, placeholders, disabled |
| `color.fg.inverse` | `#FFFFFF` | Text on dark/accent backgrounds |

### 3.2 Background Layers

| Token | Hex | Usage |
|-------|-----|-------|
| `color.bg.base` | `#FAFAF8` | Page background |
| `color.bg.raised` | `#FFFFFF` | Cards, inputs, modals |
| `color.bg.sunken` | `#F5F3EF` | Inset areas, chips, address boxes |

### 3.3 Usage Rules

- Never use raw hex values in components. Always reference `color.*` tokens.
- The accent color (`#E8572A`) is reserved for: primary CTAs, active states, destructive actions, brand highlights.
- `color.accent.soft` (`#FFF0EB`) is for accent backgrounds (avatars, badges, scan buttons).
- Success green (`#2D8E5F`) is for: deposit confirmation, wallet creation success, active connection dots.
- Info blue (`#4267F4`) is for: network-related UI, BLE/WiFi status, information badges.

---

## 4. Spacing

4px base grid. Always use `space.*` tokens.

| Token | px | Common usage |
|-------|-----|--------------|
| `space.xs` | 2 | Inline icon gaps |
| `space.sm` | 4 | Tight gaps, chip padding |
| `space.md` | 8 | Standard gap, icon-to-text |
| `space.lg` | 12 | Row padding, section gaps |
| `space.xl` | 16 | Card padding, input padding |
| `space['2xl']` | 20 | Large card padding |
| `space['3xl']` | 24 | Section margins, screen padding |
| `space['4xl']` | 32 | Major section breaks |
| `space['5xl']` | 48 | Empty state top padding |

---

## 5. Components

### 5.1 VelaButton
- Always uses spring scale animation on press (0.97x)
- Three variants: `primary` (dark bg), `secondary` (border), `accent` (orange bg)
- `compact` prop for inline/smaller contexts
- Disabled state: 0.45 opacity
- Loading state: ActivityIndicator replacing text

### 5.2 VelaCard
- Default: `shadow.sm` + 1px border + `radius.xl` (16px)
- `elevated` prop: removes border, uses `shadow.md` for hero cards
- Never apply padding to VelaCard itself — let children handle internal padding

### 5.3 TokenRow
- Fixed 40px logo size
- Spring scale (0.98x) on press
- FadeIn entrance with per-row delay (40ms × index)
- Balance column uses `font.numeric` for tabular alignment
- Always show chain label below symbol

### 5.4 ChainLogo
- Prefers remote logo image from `network.logoURL`
- Falls back to colored circle with text label if image fails
- Always circular (`borderRadius: size / 2`)

### 5.5 ScreenContainer
- SafeAreaView with `paddingHorizontal: space['3xl']` (24px)
- Background: `color.bg.base`
- Default safe area edge: top only

---

## 6. Screen Patterns

### 6.1 Navigation Bar
Every modal/pushed screen follows this pattern:
```
[ Back/Close icon ]     [ Title ]     [ spacer ]
```
- Left: `Pressable` with Lucide icon or accent-colored text
- Center: `text.xl` + `weight.bold`
- Right: empty spacer matching left width (for centering)
- Use `hitSlop={8}` on all nav pressables

### 6.2 Section Headers
```
SECTION TITLE                      [+ Action]
```
- Title: `text.sm` + `weight.semibold` + uppercase + `letterSpacing: 0.8-1.2`
- Color: `color.fg.subtle`
- Optional action button on the right

### 6.3 Form Fields
```
LABEL (uppercase, sm, subtle)
[ Input field ]
```
- Label: `text.sm`, uppercase, `letterSpacing: 0.8`
- Input: `bg.sunken` background, `radius.lg`, 1px border, `space.xl` padding
- Action buttons (Scan, MAX) sit beside inputs with `gap: space.md`

### 6.4 Empty States
```
     [ Icon in circle ]
     Title (xl, bold)
     Description (base, muted, centered)
     [ CTA Button ]
```
- Icon: 28-32px Lucide icon in 56px sunken circle
- Keep copy concise (1-2 sentences max)

### 6.5 Confirmation Cards
Use `VelaCard elevated` with rows separated by 1px dividers. Each row:
```
Label (base, muted)              Value (base, semibold)
```

---

## 7. Animation Rules

| Context | Animation | Config |
|---------|-----------|--------|
| Button press | Scale to 0.97 | `withSpring(motion.spring)` |
| List item press | Scale to 0.98 | `withSpring(motion.spring)` |
| Screen content entry | FadeInDown | `duration: 300-400ms` |
| Staggered sections | FadeInDown with delay | `delay: N * 50ms` |
| Status indicators | Opacity pulse | `0.3 ↔ 1.0, 800ms each` |
| Step indicator | Layout animation | `Layout.springify()` |

### 7.1 Rules
- Never use `withTiming` for interactive feedback — always `withSpring`
- Never animate more than 2 properties simultaneously on a single element
- Entrance animations should complete within 500ms total (including delay)
- Never use bounce or elastic easing on content transitions
- The splash screen is the only place where longer animations (>500ms) are acceptable

---

## 8. Platform Considerations

- Use `Pressable` everywhere (never `TouchableOpacity`)
- `hitSlop={8}` on all small touch targets
- Tab bar height: iOS 60px, Android 56px + safe area inset
- Modal presentation: `pageSheet` on native, portal slide-up on web
- Text scaling: all text sizes multiply by user preference (0.85x–1.28x)
- Test all layouts at both `compact` (0.85x) and `xlarge` (1.28x) scale

---

## 9. Adding New Features — Checklist

When adding a new screen or component:

1. **Import from theme** — use `color`, `text`, `weight`, `space`, `radius`, `font`, `shadow`, `motion` tokens only
2. **Use createStyles()** — never `StyleSheet.create()` (breaks text scaling)
3. **Use VelaButton/VelaCard** — don't create one-off button or card styles
4. **Add spring press** — every interactive element needs `withSpring` scale feedback
5. **Add entrance animation** — use `FadeInDown` with delay for staggered content
6. **Use Lucide icons** — import from `lucide-react-native`, never use emoji or text as icons
7. **Use Pressable** — never `TouchableOpacity`
8. **Apply font zones** — `font.display` for hero numbers, `font.mono` for addresses, `font.numeric` for balance columns
9. **Check ChainLogo** — pass `logoURL` from network data when displaying chain icons
10. **Test at min/max text scale** — verify layout doesn't break at 0.85x and 1.28x
