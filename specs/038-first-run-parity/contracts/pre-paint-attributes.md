# Contract — pre-paint attributes on `<html>` (web)

Two decisions are made by an inline, render-blocking script in `app.html`
before any module loads, and each is duplicated in a module that owns the
rule. A test reads `app.html` and asserts the two copies agree.

| Attribute | Set when | Module of record | Pinned by |
| --- | --- | --- | --- |
| `data-launch="playing"` | `vela.launch.played` absent or ≥ 604800000 ms old, and no `?skipLaunch` | `$lib/launch/constants.ts` | `constants.test.ts` (exists) |
| `data-intro="pending"` | `?intro` present; else `?skipIntro` absent AND `vela.intro.seen` absent (or storage throws) | `$lib/intro/gate.ts` (`STORAGE_KEY`, `FORCE_PARAM`, `SKIP_PARAM`, `shouldShowIntro`) | `gate.test.ts` (new cases) |

CSS in `app.css`:

```css
html[data-launch='playing'] [data-launch-page] { opacity: 0; }   /* exists */
html[data-intro='pending']  [data-intro-page]  { opacity: 0; }   /* new   */
```

`[data-intro-page]` is the Welcome `<main>` only; the intro itself carries no
attribute and is visible the moment it mounts. When the intro mounts or the
gate says no, the page removes `data-intro` from `<html>` so the attribute
never outlives the decision.

Invariants:

1. The prerendered document is the landing page for every locale (SEO); the
   attribute only hides, never removes.
2. Removing `localStorage` (private mode) shows the intro — same as the module.
3. `?skipIntro` wins over a missing flag; `?intro` wins over everything.
