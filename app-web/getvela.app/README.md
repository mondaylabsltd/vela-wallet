# sv

Everything you need to build a Svelte project, powered by [`sv`](https://github.com/sveltejs/cli).

## Creating a project

If you're seeing this, you've probably already done this step. Congrats!

```sh
# create a new project
npx sv create my-app
```

To recreate this project with the same configuration:

```sh
# recreate this project
bun x sv@0.13.0 create --template minimal --types ts --add prettier eslint vitest="usages:unit,component" playwright sveltekit-adapter="adapter:auto" mcp="ide:claude-code+setup:remote" --install bun getvela.app
```

## Developing

Once you've created a project and installed dependencies with `npm install` (or `pnpm install` or `yarn`), start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

## Building

To create a production version of your app:

```sh
npm run build
```

You can preview the production build with `npm run preview`.

> To deploy your app, you may need to install an [adapter](https://svelte.dev/docs/kit/adapters) for your target environment.

## Localization

The site is served in fifteen locales. English lives at the unprefixed paths
(`/`, `/docs/faq`); every other locale is a prefix (`/ja/`, `/pt-BR/docs/faq`).
The full design is in `specs/059-getvela-site-i18n/`.

### The three rules worth knowing before you touch it

1. **Fallback is per page, not per key.** A page namespace is used only when it
   is complete; otherwise the whole namespace renders English under a notice in
   the reader's own language. A key-by-key merge would ship a page with a
   Japanese heading and an English third paragraph, and nothing would catch it.
2. **`chrome` and `notice` must be complete in every locale.** They are the
   frame an untranslated page is drawn in — including the sentence that says the
   page is untranslated, which cannot itself fall back.
3. **`hreflang` advertises only what exists.** A falling-back page is an English
   page at a localized URL; listing it as a translation is what earns a
   duplicate-content penalty. `src/lib/i18n/urls.ts` is the single source the
   router, the head and the sitemap all read.

### Adding a string

1. Add it to `src/lib/i18n/messages/en.ts`. That file is the shape: `bun run
   check` will fail on any component reading a key that does not exist.
2. Ship it. A missing translation never blocks an English change (FR-027) — the
   locales that lack the new key fall back for that page and say so.
3. When a locale is translated, run `bun run i18n:stamp` so the English it was
   translated from is recorded.

### Adding a translation

1. Edit `src/lib/i18n/messages/<tag>.json`. Translate link TEXT, never the
   `href`; keep `<a>`, `<strong>` and `<em>` exactly as the English has them —
   `messages.test.ts` fails if the href or tag set differs.
2. A page namespace is all-or-nothing: a partial one is a test failure, not a
   partial translation. Leave it out entirely instead.
3. `bun run i18n:stamp`, then `bun run i18n:status` to see where things stand.

### Adding a locale

The locale list must equal `app-web/vela-wallet`'s `SUPPORTED_LOCALES` — a test
reads that file off disk and fails on any divergence. Changing the set means
changing the app's too, deliberately, in both places.

### Commands

```bash
bun run i18n:status   # per-locale coverage, and what is STALE
bun run i18n:stamp    # record the English a translation was made from
bun run check         # catches a key that does not exist
bun run test:unit     # shape, chrome completeness, href/tag integrity, hreflang
```

**STALE** means the English changed after a translation was written: the page
still renders, but it now says something the English no longer says. It is the
one state in the report that means something is wrong rather than unfinished.
