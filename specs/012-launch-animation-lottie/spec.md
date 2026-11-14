# Feature Specification: Lottie Launch Animation Across Four Apps

> **Superseded on 2026-11-14 (the runtime, not the experience).** The Lottie
> runtimes are gone from all four apps (lottie-web, lottie-ios, lottie-compose,
> dotlottie-rs). The same animation is now one scene,
> `docs/design/onboarding/launch/launch-scene.json`, generated for each
> platform by `scripts/gen-launch.mjs` and drawn with that platform's own
> vector and animation APIs. The behaviour this spec defines (play once, hold,
> cross-dissolve, skip on input, reduce motion, the fit rule, the replay gate)
> still holds; what it says about Lottie files, the portable subset, asset
> loading and the desktop frame pump is history.

**Feature Branch**: `012-launch-animation-lottie`

**Created**: 2026-08-05

**Status**: Draft

**Input**: User description: "lottie 你了解吗，我已经创建了开机启动动画，有透明背景的暗色
和暗色，在这个目录下 /Volumes/data/production/vela-wallet/design/onboarding/launch。我希望
在 app-android/vela-wallet、app-ios/VelaWallet、app-desktop/vela-wallet、app-web/vela-wallet
这四个仓库里接入这个开机动画…lottie 库考虑使用 airbnb 还是 lottiefiles 的呢？…我希望性能
好，稳定，现在只是开机动画接入 lottie，后续会有其他动画接入的。创建新分支来做这个任务，
使用 speckit 工作流"

Two decisions were taken with the founder before this spec was written, and both
are binding on it:

1. **Runtime selection is per-platform, not one library everywhere.** Rationale
   and measurements live in `plan.md` / `research.md`; the spec states only the
   behaviour that must hold regardless of which runtime renders it.
2. **The animation follows the app's effective light/dark theme**, and the
   Android system splash background moves from a fixed ink colour to a
   theme-following one so the hand-off does not flash.

## Why

The four Vela clients each open on a bare Welcome screen. There is no moment that
says *whose* wallet this is before the user is asked to trust it with money —
and "new wallet, why would I trust you" is the top barrier this product has
(`docs/marketing/100-marketing-leads.md`). A 1.7-second wordmark build is the
cheapest credibility signal available, and it is the only screen every single
user sees.

The reason this is a spec and not a one-line asset drop is that it is the
**first** animation, not the only one. The founder has said more are coming.
Three things therefore have to be decided once, here, rather than four times,
badly, later:

| Decision | Cost of getting it wrong later |
| --- | --- |
| Where the animation source of truth lives | Four divergent copies of every future animation, discovered when one of them is stale in production |
| What a launch animation is allowed to delay | A wallet that takes measurably longer to open, on the one screen with 100% user exposure |
| Which Lottie features an animation may use | Four renderers that agree today and disagree the first time a designer reaches for a gradient or a matte |

The animation being integrated is deliberately simple — shape layers, solid
fills, opacity keyframes, one position tween, no masks/mattes/gradients/trim
paths/text/images/expressions — which means it renders identically everywhere
*today*. That is a property of this file, not a guarantee of the format, and
this feature must turn it into an enforced rule rather than a lucky accident.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The app opens with the Vela wordmark building itself (Priority: P1)

A user launches Vela Wallet — on iPhone, on Android, on desktop, or by opening
the web app. Instead of the Welcome screen appearing bare, the sailboat mark
slides from centre to the left while the letters of "Vela Wallet" fade in one
after another; the finished lockup holds for a beat, then dissolves into the
Welcome screen. The whole thing takes about two and a half seconds, the user can
cut it short at any point with a tap, and it plays *while* the app is becoming
ready rather than before it starts.

**Why this priority**: This is the feature. Every other story exists to keep this
one from costing something.

**Independent Test**: Ship only this story on one platform and that platform
opens with the animation, plays it once, and lands on Welcome — verifiable by
launching the app and by a golden-frame comparison at fixed timestamps.

**Acceptance Scenarios**:

1. **Given** a cold start of any of the four apps, **When** the app becomes
   ready to show its first screen, **Then** the launch animation plays exactly
   once, at its authored speed, and the Welcome screen follows it without a
   visible gap, colour jump, or repositioning of the brand mark.
2. **Given** the app is running in dark appearance, **When** the animation plays,
   **Then** the dark variant is used (ivory wordmark, dusk-ivory hull); in light
   appearance the light variant is used (ink wordmark, warm-graphite hull).
3. **Given** the animation has reached its final frame, **When** it finishes,
   **Then** the completed lockup is held briefly so it registers as a moment
   rather than a flash, and the launch screen then **cross-dissolves** into
   Welcome — the lockup fading out as the Welcome content fades in over one
   continuous background, never cutting.
4. **Given** the animation has finished, **When** the user navigates anywhere and
   returns to Welcome, **Then** the animation does not play again.
5. **Given** the app is resumed from the background rather than cold-started,
   **When** it becomes visible, **Then** the animation does not play.

---

### User Story 2 - The animation can never trap a user outside their wallet (Priority: P1)

A user launches the app on a slow device, or after an update that broke the
animation asset, or with a corrupted install. They reach the Welcome screen
anyway, promptly, without knowing anything went wrong.

**Why this priority**: This is a wallet. An animation is decoration; a decoration
that can block entry to funds is a defect regardless of how well it renders. The
failure modes are all silent-by-design, which is exactly why they must be
specified rather than assumed.

**Independent Test**: Force each failure path (missing asset, malformed asset,
runtime unavailable, animation slower than its budget) and confirm the app
reaches Welcome within the stated budget in every case.

**Acceptance Scenarios**:

1. **Given** the animation asset is missing or unparseable, **When** the app
   launches, **Then** the app proceeds directly to Welcome with no error message,
   no blank screen, and no added delay beyond the readiness the app already
   required.
2. **Given** the animation has not produced its first frame within the startup
   budget, **When** the budget expires, **Then** the animation is abandoned and
   Welcome is shown.
3. **Given** the animation is playing, **When** the user taps, clicks, or presses
   any key, **Then** playback ends immediately and Welcome is shown.
4. **Given** playback has begun, **When** the total elapsed animation time exceeds
   its hard ceiling for any reason, **Then** it is cut short and Welcome is shown.
5. **Given** any unexpected runtime failure during playback, **When** it occurs,
   **Then** it is contained: the app reaches Welcome and the failure is never
   surfaced to the user as an error state.

---

### User Story 3 - Users who asked for less motion get less motion (Priority: P1)

A user who has turned on their platform's reduce-motion accessibility setting
launches the app and sees the finished lockup, not a build.

**Why this priority**: Motion sensitivity is an accessibility requirement, not a
preference, and a full-screen animation on every launch is precisely the class of
motion the setting exists to suppress. Every platform in scope exposes the
setting; none of them is exempt.

**Independent Test**: Enable reduce-motion on each platform, launch, and confirm
the final composed frame is shown with no movement and no fades.

**Acceptance Scenarios**:

1. **Given** reduce-motion is enabled, **When** the app launches, **Then** the
   final frame of the animation is presented statically — the mark in its
   left-hand resting position with the full wordmark visible — and no element
   moves or fades.
2. **Given** reduce-motion is enabled, **When** the static frame is shown,
   **Then** the app advances to Welcome no later than it would have without the
   setting.

---

### User Story 4 - The next animation costs a day, not a week (Priority: P2)

A designer exports a second animation. A developer drops the file into the design
directory, references it from one small component per platform, and it works —
without touching build scripts, without copying the file into four repositories,
and without discovering three months later that one platform renders it wrong.

**Why this priority**: This is the durable payoff and the stated reason the
founder asked for Lottie rather than a hand-coded animation. If integrating the
second animation is as expensive as the first, the format bought nothing.

**Independent Test**: Add a second animation file and confirm it reaches all four
apps through the same path with no per-file build changes, and that a file using
a disallowed feature is rejected automatically.

**Acceptance Scenarios**:

1. **Given** a new animation file placed in the design directory, **When** each
   app is built, **Then** the file is present in that app's bundle without any
   file being copied into an app directory by hand.
2. **Given** an animation file that uses a feature outside the agreed portable
   subset, **When** the repository's checks run, **Then** the check fails and
   names the file, the layer, and the offending feature.
3. **Given** a maintainer wants to know what an animation is allowed to contain,
   **When** they look, **Then** the rule is written in one place and is the same
   rule the automated check enforces.

---

### User Story 5 - The four platforms are provably showing the same thing (Priority: P2)

A maintainer changes the animation, or bumps one platform's animation runtime,
and finds out immediately if that platform started rendering differently from the
others — rather than finding out from a user's screenshot.

**Why this priority**: Three different rendering engines are in use by design.
Drift between them is the accepted risk of that choice, so it must be the
*detected* risk rather than the latent one.

**Independent Test**: Capture frames at fixed animation timestamps on each
platform, compare to committed references, and confirm a deliberate change to the
asset makes all four fail.

**Acceptance Scenarios**:

1. **Given** committed reference frames for each platform, **When** the visual
   checks run against an unchanged asset, **Then** all comparisons pass within the
   agreed per-platform tolerance.
2. **Given** the animation asset is modified, **When** the visual checks run,
   **Then** every platform reports a mismatch — proving each check is actually
   looking at the asset.
3. **Given** a platform's animation runtime is upgraded, **When** the visual
   checks run, **Then** any change in that platform's output is surfaced before
   release.

---

### Edge Cases

- **The system splash and the app's theme disagree.** On Android the operating
  system paints its splash background before the app can read the user's stored
  theme preference — it can only follow the OS-level dark-mode setting. A user
  who has overridden the theme *inside* Vela to the opposite of their OS setting
  will therefore see the system splash in one appearance and the animation in the
  other. The transition between them must be covered so this reads as a
  deliberate transition, never as a flash.
- **A very wide or very short window.** The two authored compositions place the
  lockup at very different proportions of the screen (about 81% of width on the
  phone frame, about 30% on the large-screen frame). Choosing the wrong one for a
  viewport is the visible failure — a lockup that fills a desktop window edge to
  edge, or one that reads as a speck on a phone. Tablets and resizable browser
  windows sit between the two authored anchors and must resolve deterministically.
- **The app becomes ready before the animation ends, or after.** Both orders
  happen. Neither may produce a stall, a double transition, or a Welcome screen
  that appears and is then covered again.
- **The user backgrounds the app mid-animation.** Returning must not restart the
  animation nor leave the app stuck behind a paused overlay.
- **Reduce-motion is toggled while the app is running.** The next cold start must
  honour the new setting; no requirement is placed on changing mid-playback.
- **The animation asset changes but a stale copy is bundled.** This must be
  impossible by construction, not caught by review.
- **Web: the page is server-rendered and may be seen before any script runs.**
  The animation must never be the element that determines when the page's main
  content appears, and a visitor with scripting unavailable must see the normal
  page.
- **Desktop: the window is resized during playback.** Playback must continue and
  reframe, not restart or tear.
- **The same user opens the web app repeatedly.** A full launch animation on
  every page view of a site whose job is conversion is a cost, not a feature; the
  frequency rule for web is stated in Assumptions and is deliberately different
  from the native apps.

## Requirements *(mandatory)*

### Functional Requirements

#### Asset ownership and distribution

- **FR-001**: `design/onboarding/launch/` MUST be the single source of truth for
  every animation file. No app may contain a committed copy. The directory holds
  two form factors (phone, large screen) × two appearances × two framings
  (cropped-to-motion, full-bleed); only the cropped framings are shipped, and
  the full-bleed pair for each form factor is retained as the reference that
  fixes how much of the screen the lockup occupies.
- **FR-002**: Each app MUST obtain its animation assets from that directory at
  build time, by the same mechanism the repository already uses for locale
  catalogues and design tokens.
- **FR-003**: A build that cannot obtain the assets MUST fail loudly at build
  time rather than producing an app that silently has no animation.
- **FR-004**: Adding a further animation file MUST NOT require editing any app's
  build configuration.

#### Portable-subset rule

- **FR-005**: The repository MUST define, in one place, the set of Lottie
  features an animation source file is permitted to use.
- **FR-006**: An automated check MUST reject any animation file in the design
  directory that uses a feature outside that set, naming the file, the layer and
  the feature.
- **FR-007**: All eight delivered launch files MUST pass that check unmodified,
  including the cross-file assertions that keep the form factors and framings
  consistent with one another.

#### Playback behaviour (all four apps)

- **FR-008**: The animation MUST play exactly once per cold start, never on
  resume, never on re-entering the Welcome screen.
- **FR-009**: The variant shown MUST match the app's effective appearance — the
  same resolved light/dark value the rest of the app's colours already use, not
  the raw operating-system setting.
- **FR-010**: The animation MUST be presented over a background matching the
  app's effective appearance, so no transparent-background artefact is visible.
- **FR-011**: The form factor MUST be chosen from the viewport by one predicate
  shared by all four apps, and within a form factor the brand lockup MUST occupy
  the authored proportion of the viewport width at every window and screen size.
  The proportion MUST be derived from the delivered assets, not chosen
  independently of them.
- **FR-012**: The hand-off to the Welcome screen MUST be a **cross-dissolve**:
  the launch lockup fades out while the Welcome content fades in, over a
  background that stays continuous throughout, with no flash, no blank frame and
  no cut. Fading only the launch screen's backdrop is explicitly not enough — the
  lockup itself must fade, or the transition reads as an abrupt disappearance.
  The Welcome screen MUST be fully composed beneath the animation before the
  hand-off begins, so the user sees one finished surface dissolve into another
  rather than a screen being assembled. The brand lockup dissolves in place; it is
  **not** required to travel to the position the Welcome screen's own brand row
  occupies (see Assumptions).
- **FR-012a**: The completed lockup MUST be held briefly after the animation ends
  and before the dissolve begins, so the finished mark registers rather than
  flashing past. The hold MUST be skippable by input and MUST be bypassed under
  reduce-motion.

#### Never blocking

- **FR-013**: The launch animation is a **self-contained screen**, not a
  decoration layered over a visible Welcome: it fills the viewport over an opaque
  background, and the user MUST NOT see any part of the Welcome screen while it
  plays. The Welcome screen follows it.
- **FR-013a**: The animation MUST NOT delay any work the app must do before it is
  usable. Preparing the Welcome screen MUST happen behind the animation — hidden
  by FR-013's opaque background — rather than after it, so the hand-off has
  nothing left to build. That preparation MUST NOT begin before the animation's
  first frame is presented, so it cannot compete with the animation's own start.
- **FR-014**: If the animation has not begun presenting within a defined startup
  budget, it MUST be abandoned and the app MUST proceed.
- **FR-015**: Total time attributable to the animation MUST be bounded by a hard
  ceiling; exceeding it MUST cut playback short rather than extend the launch.
- **FR-016**: Any user input during playback MUST end it immediately and proceed.
- **FR-017**: A missing, unreadable, malformed or unsupported animation asset
  MUST result in the app proceeding normally, with nothing shown to the user
  about the failure.
- **FR-018**: No animation failure may leave the app in a state where the Welcome
  screen is unreachable, obscured, or non-interactive.

#### Accessibility

- **FR-019**: When the platform's reduce-motion setting is enabled, the final
  frame MUST be presented statically with no movement or fading.
- **FR-020**: Reduce-motion MUST NOT make the app slower to reach Welcome than it
  would otherwise be.
- **FR-021**: The animation MUST NOT be announced to assistive technologies as
  content, and MUST NOT steal or trap focus.

#### Android splash continuity

- **FR-022**: The system splash background MUST follow appearance rather than
  being a fixed colour in both modes.
- **FR-023**: The transition from the system splash to the animation MUST be
  continuous in the common case (in-app theme agrees with the OS setting), and
  MUST degrade to a deliberate transition — never an abrupt flash — when the user
  has overridden the theme in-app.

#### Replaceability

- **FR-024**: Each app MUST access its animation runtime through a single, thin
  component owned by this repository; no screen, view model or route may call an
  animation library directly.
- **FR-025**: That component's interface MUST be the same conceptual shape on all
  four platforms — an animation to play, an appearance, and a completion signal —
  so a future runtime swap on one platform touches one file.

#### Verification

- **FR-026**: Each platform MUST have a visual regression check comparing rendered
  frames at fixed animation timestamps against committed reference images.
- **FR-027**: Each platform MUST have a test proving the app reaches Welcome when
  the animation asset is absent or malformed.
- **FR-028**: Each platform MUST have a test proving reduce-motion yields the
  static final frame.
- **FR-029**: The existing onboarding tests on every platform MUST continue to
  pass; where an existing test would now encounter the animation, it MUST be made
  to skip the animation deterministically rather than to wait it out.

### Key Entities

- **Launch Animation Asset**: An animation source file identified by form factor,
  appearance and framing, with a fixed canvas size, frame rate and duration;
  owned by design, consumed by all four apps, never edited by app code.
- **Form Factor**: Which of the two authored compositions applies — phone or
  large screen. Selected from the viewport by one predicate shared by all four
  apps, so tablets and resizable windows need no per-platform special-casing.
- **Portable Subset**: The enumerated set of animation features permitted in any
  asset in this repository — the contract that lets three rendering engines agree.
- **Launch Presentation**: One occurrence of the animation for one cold start:
  which variant, whether motion is reduced, when it started, and how it ended
  (completed / skipped by the user / abandoned on budget / failed).
- **Reference Frames**: Committed per-platform images at fixed animation
  timestamps, the evidence that the platforms still agree.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All four apps open with the launch animation, playing once per cold
  start, in both light and dark appearance — eight combinations, all verified on
  a real device or window, plus both form factors on the one app that can reach
  both (a resizable browser window).
- **SC-002**: Time from launch to a usable Welcome screen increases by no more
  than the animation's duration plus its hold and dissolve — a single budget
  fixed in the plan, currently 2.5 s — and by **zero** when the user skips. Under
  reduce-motion the hold is bypassed entirely, leaving only the dissolve.

  *Revised 2026-08-05 after the first desktop review.* The original wording
  promised "no more than the animation's own duration" (1.7 s). Holding the
  finished lockup and lengthening the dissolve are deliberate spends of the
  user's time, requested on seeing the transition; pretending the budget did not
  move would make this criterion unfalsifiable.
- **SC-003**: With the animation asset deleted from a build, every app still
  reaches Welcome, with no error shown and no measurable added delay.
- **SC-004**: Zero committed copies of any animation file exist outside the design
  directory — verified by searching the repository, not by convention.
- **SC-005**: A file using a disallowed animation feature is rejected
  automatically, demonstrated by a deliberately invalid fixture.
- **SC-006**: A change to the animation asset causes the visual check to fail on
  all four platforms, proving none of the four checks is inert.
- **SC-007**: Adding a second animation file requires touching no build
  configuration on any platform — demonstrated once during this feature.
- **SC-008**: On Android, the system-splash-to-animation hand-off shows no
  background flash when the in-app theme agrees with the OS setting.
- **SC-009**: Each app's animation runtime is reachable from exactly one file in
  that app, verified by inspection.

## Assumptions

- **Runtime selection is per-platform and already decided** (iOS, Android and web
  on the Airbnb runtimes; desktop on the LottieFiles Rust core, since no Airbnb
  runtime exists for Rust). The measurements behind that choice belong in
  `research.md`; the spec above holds regardless of it.
- **The startup budget and hard ceiling are derived from the asset**: the
  animation is 1.7 s, so the ceiling is set just above it and the
  first-frame budget is a small fraction of it. Exact values are set in the plan
  and are tunable without changing this spec.
- **The web app's frequency rule differs from the native apps**: the animation
  plays on the first view of a browsing session and not on subsequent
  navigations, because a full launch animation on every page view of a
  conversion-critical site is a cost rather than a feature. *This is the one
  behavioural default in this spec that is a judgement call rather than a
  derivation; it is called out here so it can be overridden cheaply.*
- **The operating systems' own launch screens stay static.** Neither iOS launch
  screens nor Android's system splash can run a Lottie animation; the animation
  necessarily plays as the app's first rendered view, immediately after the
  system splash hands off. Making that seam invisible is FR-012 and FR-023.
- **The existing brand geometry is unchanged.** The animation's mark and colours
  already match the in-app brand mark and design tokens; this feature does not
  redraw either.
- **The launch lockup dissolves rather than flying into the Welcome brand row**
  (founder decision, 2026-08-05). All four Welcome screens already render the
  same lockup — mark plus "Vela Wallet" on one row — but at a smaller size and,
  on three of the four, a different position. A travelling hand-off was
  considered and declined: the animation's wordmark is traced vector outlines
  while the Welcome wordmark is live type, so a travelling version would still
  need a crossfade on arrival. It is therefore an *addition* to the chosen
  hand-off, not an alternative to it, and can be added later without changing
  anything specified here.
- **The two form factors are the same artwork at a fixed ratio.** They differ
  only in how much of the screen the lockup occupies, so no separate visual
  review is needed for the second one — but both need their own golden frames,
  because that is what would catch them drifting apart.
- **The delivered set is verified and complete.** All eight files pass the
  portable-subset scan and every cross-file invariant (identical timing, no
  clipped keyframes, exact vertical centring, matching colours and layer names
  per form factor). No asset work is outstanding.
- **No new user-facing copy or translations are introduced.** The animation
  contains the wordmark as vector shapes, not as text requiring a font or
  localisation.
- **Golden-frame tolerance is per-platform.** Different engines will differ by a
  small number of pixels on anti-aliased edges; the checks are for drift, not for
  bit-identity.

## Out of Scope

- Any animation other than the launch animation. This feature establishes the
  path; it does not populate it.
- Animating the Welcome screen's own contents, transitions between onboarding
  screens, or any in-app micro-interaction.
- The React Native / Expo app at the repository root. It has its own launch
  behaviour and is not one of the four targets named in the request.
- Replacing the operating systems' launch screens or app icons.
- Interactive, state-machine-driven or themeable animations, and the `.lottie`
  container format — the assets in scope are plain animation JSON.
- Re-authoring or redesigning the animation itself.
