# Quickstart: validating the retirement

Every command below is run from the repo root unless a `cd` says
otherwise. "Green" means exit 0 with no diff where a diff is checked.

## 0. Baseline (before the first deletion commit)

```sh
git checkout 039-retire-expo-tree && git status --short   # clean
cd app-web/vela-wallet && pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build; cd -
cd app-desktop/vela-wallet && cargo fmt --all --check && cargo clippy --all-targets && cargo test; cd -
cd rust && cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures; cd -
node rust/scripts/build-web.mjs --check && node rust/scripts/verify-web.mjs
npm run dump:vectors && git diff --exit-code -- rust/crates/vela-core/tests/vectors/
node scripts/gen-identicon-features.mjs && git diff --exit-code -- rust/crates/vela-core/src/identicon_features.rs
node scripts/gen-i18n.mjs && git diff --exit-code -- rust/crates/vela-core/src/i18n public/i18n
node scripts/lint-i18n-corpus.mjs && node scripts/lint-lottie-assets.mjs --self-test && node scripts/lint-lottie-assets.mjs
node scripts/check-native-reachability.mjs && node scripts/verify-identicon-parity.mjs && node scripts/verify-i18n-parity.mjs
```

Record each result in `results.md` §Baseline. Android/iOS: run locally only
if `rust/bindings/kotlin/` and `app-ios/VelaCoreKit/Artifacts/` exist on
disk (see plan D12); otherwise CI proves them.

## 1. After the deletion commit (the founder's constraint)

Re-run every line of §0 that does not name a deleted path (all of them
still apply — none reads `src/`). Add:

```sh
npm ci                                            # the tooling package installs clean
npm run build --prefix packages/vela-sdk          # tsc resolves from the root
git ls-files src e2e modules plugins targets packages/safari-extension | wc -l   # 0
```

Expected: identical results to §0. Any difference is a shared-floor
consumer the inventory missed — stop and record it.

## 2. After the generator commit

```sh
node rust/scripts/gen-core-types.mjs --check      # zero diff under app-web/vela-wallet
node rust/scripts/gen-core-types.mjs && git status --short   # nothing new
node scripts/gen-i18n.mjs && git diff --exit-code -- rust/crates/vela-core/src/i18n rust/crates/vela-core/src/i18n_catalogs rust/crates/vela-core/src/l10n public/i18n
./scripts/gen-app-icons.sh && git diff --stat     # zero
cd app-web/vela-wallet && pnpm check; cd -        # runs gen-core-types … --check itself
```

## 3. After the CI commit

Push; open the Actions run. Green: `app`, `web`, `site`, `rust`,
`rust-macos`, `desktop`, `android`, `ios`. Paste the run URL into
`results.md`.

## 4. The drift proofs (SC-393) — do once, revert, record

```sh
# i18n: one character in one locale value
sed -i '' 's/"Send"/"Send!"/' rust/crates/vela-core/i18n/locales/en/send.json
node scripts/gen-i18n.mjs && git diff --exit-code -- public/i18n ; echo "exit=$?"   # expected: exit=1
git checkout -- rust/crates/vela-core/i18n/locales/en/send.json public/i18n rust/crates/vela-core/src

# identicon: one byte in one vector
python3 - <<'EOF'
import json,pathlib
p=pathlib.Path('rust/crates/vela-core/tests/vectors/identicon.json'); d=json.loads(p.read_text())
# flip the first output byte of the first case (shape is asserted by the dump script)
EOF
npm run dump:vectors && git diff --exit-code -- rust/crates/vela-core/tests/vectors/identicon.json ; echo "exit=$?"   # expected: the dump REWRITES the vector → diff → exit=1
git checkout -- rust/crates/vela-core/tests/vectors/
```

Record both `exit=1` lines and the clean `git status` after.

## 5. The residue check

```sh
node scripts/check-expo-residue.mjs --self-test
node scripts/check-expo-residue.mjs                # exit 0 after the doc pass; before it, rule 4 lists the work
```

## 6. Clean-tree tests 1 and 2

```sh
ls -A                                              # test 1: compare against plan.md §Source Code
git status --ignored --short | grep -E '^!! (dist|test-results|\.expo|web-build|e2e)/' ; echo "exit=$?"   # expected: exit=1 (no match)
grep -nE '^(\.expo/|dist/|web-build/|expo-env\.d\.ts|/ios|/android|\.metro|e2e/screenshots/|test-results/)' .gitignore ; echo "exit=$?"   # expected: exit=1
```

## 7. The fresh-clone walk (SC-395)

In a temporary directory:

```sh
git clone --branch 039-retire-expo-tree <origin> vela-fresh && cd vela-fresh
# follow docs/project-takeover/02-local-development.md top to bottom, running every command it names
```

Expected: a running web wallet (`pnpm dev` in `app-web/vela-wallet`), a
running desktop app (`cargo run` in `app-desktop/vela-wallet`), and the root
gate set green, with no command in the document failing "not found". Note
the time taken and any sentence that misled, in `results.md`.

## 8. Production statement (SC-395, D14)

```sh
grep -n "Pages" README.md docs/project-takeover/05-deployment-runbook.md
curl -sL https://wallet.getvela.app/en/wallet | grep -oE '/_expo/[^"]+' | head -1   # still the frozen build — expected until the move
```

The README and runbook lines must name the frozen commit and the date.
