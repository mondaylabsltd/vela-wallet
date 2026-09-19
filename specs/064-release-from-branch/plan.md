# 064 plan

**Shape**: one new workflow, six converted ones, a version/commit contract in five shells,
one script argument, docs. One Cloudflare setting that only the founder can change.

| # | Decision | Why |
|---|---|---|
| D1 | `release.yml` **calls** the packaging workflows (`workflow_call`) rather than re-triggering them with a tag | A tag pushed with `GITHUB_TOKEN` triggers nothing (GitHub's recursion guard), and one run with one set of artifacts lets `publish` decide by NAME what is attached — the phone artifacts are simply never downloaded |
| D2 | One tag `vX.Y.Z`, one Release | They were separate because they were separately tagged. One commit, one version, one thing to point at — and "push tags one at a time" stops being a rule anyone can forget |
| D3 | The tag is created by `gh release create --target $GITHUB_SHA` | It exists at exactly the released commit, atomically with the Release; nobody pushes a tag by hand |
| D4 | The gate refuses a version already released from a different commit; the same commit re-runs | Published packages are not swapped under people. A failed run must still be re-runnable |
| D5 | `publish` needs EVERY shell's build, phones included | A release whose Android build is broken is not a release, even though no Android file is attached |
| D6 | Each build job asserts its package carries `${GITHUB_SHA:0:7}` | The lesson of 0.9.0: a property of the release path is asserted ON the release path, not in a test beside it |
| D7 | `VELA_GIT_COMMIT` from the workflow's env, git as the fallback, `unknown` as the last word | Container builds often cannot ask git (`dubious ownership`); a developer's machine has no such variable; and a constant that looks like a commit is how `6ab8f` shipped for months |
| D8 | The web/extension version is `extension/manifest.json`'s | It is the one declared version that app has and the gate already checks it. `package.json` says 0.0.1 and nothing reads it |
| D9 | `released` moves fast-forward only, no `--force` | A release that does not contain the previous one is a mistake; git's refusal is the right message |
| D10 | 0.x releases are marked pre-release by the workflow | The founder's 2026-09-17 ruling, done by hand three times since |

**Verification that needs no release**: dispatch each packaging workflow on the branch (real
builds, now with D6's assertions); push throwaway `release/v*` branches whose versions cannot
pass the gate, to see each refusal — they build nothing and publish nothing.
**Verification that IS the release**: spec §7.
