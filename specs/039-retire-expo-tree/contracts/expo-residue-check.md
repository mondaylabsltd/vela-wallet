# Contract: `scripts/check-expo-residue.mjs`

The committed form of the spec's clean-tree test 3 (SC-394, FR-395). Run
as `npm run check:expo-residue` at the repo root and as a step of the CI
`app` job. Node 22, no dependencies, reads `git ls-files` only (untracked
files are not its business — test 2 covers those and is a one-line
`git status --ignored` in quickstart.md).

## Hard failures (exit 1)

1. **Deleted paths reappear.** Any tracked path under `src/`, `e2e/`,
   `modules/`, `plugins/`, `targets/`, `packages/safari-extension/`, or any
   of the root files: `package.json package-lock.json` (the tooling package
   is `scripts/package.json`; the root carries no npm state — founder
   ruling, 2026-09-11), `app.json app.config.js eas.json metro.config.js
   index.js index.web.js expo-env.d.ts fingerprint.config.js
   .fingerprintignore jest.config.js jest.setup.js playwright.config.ts
   tsconfig.json eslint.config.js keystore.properties.example
   scripts/reset-project.js scripts/fix-cf-pages-assets.js
   scripts/jest-skipped-reporter.js .verify-rescan.mjs .verify-rescan-ja.mjs`
   and any tracked path under `.eas/`.
2. **Expo / RN dependencies.** In every tracked `package.json`: a key under
   `dependencies`, `devDependencies`, `peerDependencies` or
   `optionalDependencies` matching `/^(expo|@expo\/|expo-|react-native|@react-native|@react-navigation|eslint-config-expo|@bacons\/apple-targets|lucide-react-native)/`.
3. **Expo terms in executable config.** In `.github/workflows/*.yml`: any
   non-comment line (not starting with optional whitespace then `#`)
   matching the term regex. In every tracked `package.json`: any `scripts`
   value matching the term regex. The checker's own name (`expo-residue`,
   `check:expo-residue`) is the one term allowed.
4. **Dead commands anywhere.** In every tracked text file (any extension,
   including `.md`), any line matching
   `/(npx\s+expo\b|\bexpo\s+(start|export|run|prebuild|lint|install)\b|\beas\s+(build|submit|credentials|update)\b|npm\s+run\s+(web|build:web|test:live)\b|\bnpx\s+metro\b|\bmetro\s+(start|bundle)\b)/i`
   — only commands that existed at the repository ROOT for the Expo tree;
   `npm run test:e2e` and the like are alive in the shells and are not
   matched
   — unless the document is **banner'd**: its first 20 lines contain a line
   matching `/^>\s*\*\*(History\s*\(\d{4}-\d{2}-\d{2}\)|勘误\s*[（(]\d{4}-\d{2}-\d{2})/`.
   A banner'd document is exempt from rule 4 in full (the banner precedes
   everything it quotes); a living document may not quote a dead command.
   `specs/**`, this script, its fixtures and lockfiles are exempt from rules
   3–4 and from the report.

The term regex for rules 2–3: `/\bexpo\b|react-native|react native|\bmetro\b|\beas\b|\bhermes\b/i`.

## Report (exit 0, printed)

For every other tracked file with a term-regex match, one line: `hits path`
sorted by hits descending, followed by a total. This is the "words are
gone from code and config" view; it is informational so that history
banners and past-tense provenance comments do not fail CI, while the
number stays visible in every run.

## Exit codes and output shape

```
$ node scripts/check-expo-residue.mjs
expo-residue: rule 1 (deleted paths)        ok
expo-residue: rule 2 (dependencies)         ok
expo-residue: rule 3 (executable config)    ok
expo-residue: rule 4 (dead commands)        ok
expo-residue: prose mentions (report only)  37 files, 141 lines
   33 docs/project-takeover/11-interview-answer-key.md
   …
```

On failure the offending `path:line: text` lines print under the rule
that caught them, and the process exits 1.

## Self-test

`node scripts/check-expo-residue.mjs --self-test` runs the four rules
against fixtures under `scripts/__fixtures__/expo-residue/` — one file that
must trip each rule and one banner'd document whose quoted `npx expo start`
must not — and exits non-zero if any expectation is wrong. The CI step runs
the self-test first, as `lint-lottie-assets.mjs` does.
