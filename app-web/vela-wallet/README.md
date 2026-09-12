# Vela Wallet — Web

SvelteKit rewrite of the Vela Wallet UI. First page: Onboarding Welcome
(spec `specs/006-web-onboarding/` at the repo root — spec, plan, contracts,
tasks, and delivery report live there).

## Architecture (what makes this app different)

- **Design tokens are generated, never hand-written.** `pnpm gen:tokens` reads
  the Penpot DTCG export at `../../docs/design-tokens.json` and writes
  `src/lib/tokens/tokens.css` (dark = base, light via
  `prefers-color-scheme` + dormant `[data-theme]` hooks) and
  `src/lib/tokens/tokens.ts`. Both are committed and drift-gated
  (`src/lib/tokens/tokens.test.ts` + `--check` in `build`/`check`). Product UI
  must use `var(--…)` only — a literal audit test enforces it. Token intent:
  `docs/design-system.md`.
- **i18n runs the real vela-core Rust engine at build time.** Every
  `/{locale}` page (15 locales) is prerendered; `src/lib/i18n/engine.server.ts`
  `initSync`s the wasm from `../../rust/pkg-web` and resolves strings from the
  generated catalogs in `../../assets/i18n/`. No translation runtime ships to
  the client, and no wasm reaches the deployed Worker (it cannot compile wasm
  from bytes — e2e asserts `_worker.js` stays clean). The only runtime route
  is `/`, a wasm-free Accept-Language 307.
- **Translations live in the corpus, not here.** Add/edit strings in
  `../../rust/crates/vela-core/i18n/locales/<locale>/<namespace>.json` (all 15
  locales), then run `npm run gen:i18n` at the repo root (update its path-count
  pin when adding keys) and rebuild. Never fork translations into this app.

## Commands

```sh
pnpm dev          # dev server
pnpm gen:tokens   # regenerate the token layer after a Penpot export update
pnpm check        # tokens drift + wrangler types + svelte-check
pnpm lint         # prettier + eslint
pnpm test:unit    # vitest: drift/audit/contrast/negotiation/engine gates
pnpm build        # prerenders all locale pages (runs the wasm engine in Node)
pnpm preview      # wrangler dev of the built worker on :4173
pnpm test:e2e     # playwright: SSR html, layout/overflow, dark-light visuals
```

## Adding a page (the intended pattern)

1. Put reusable UI in `src/lib/ui/` — visual values only via tokens.
2. Route under `src/routes/[locale]/…`; get strings via a `.server.ts` load
   that calls the engine (extend `messages.ts`/`engine.server.ts`), returning
   plain strings to the page.
3. Keep it prerenderable if it can be; runtime SSR on Cloudflare cannot use
   the wasm engine (see `specs/006-web-onboarding/research.md` D1/D2 before
   changing that).
