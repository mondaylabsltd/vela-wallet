# 063 plan

**Shape**: workflows, one packaging script, two plist keys, the download page, docs. No
product code, no core change, no new dependency.

## Decisions

| # | Decision | Why |
|---|---|---|
| D1 | Phone workflows keep their tag trigger and build; only the `release` job goes | The tag is still how the founder gets the artifact to sign, and the build is the only thing that compiles the release configuration. 0.9.0 showed what an unwalked packaging path costs |
| D2 | One `publishable` output, computed first, read by build, verify and release | "May this run publish?" must have one answer, not three guesses |
| D3 | Credentials in an **environment** restricted to `desktop-v*` tags; a second empty environment for every other run | Public repository. `workflow_dispatch` from a fix branch must keep working (it is how packaging fixes are proven) and must find nothing |
| D4 | The script owns signing and notarization; the workflow only supplies a keychain | The founder can run exactly what CI runs (`quickstart.md` §4), which is also the only way to test the camera and passkey before a tag |
| D5 | Notarize and staple the **app**, then the **image** | A ticket on the image stays on the image; what a person launches is the copy they dragged out |
| D6 | Validate the provisioning profile in the workflow, in words | Every way the profile can be wrong otherwise surfaces on a user's Mac, after publication, as a bundle killed at launch |
| D7 | Install notes are a file per release line, prepended with `--notes-file` | Three desktop workflows race to create the release; whoever wins must write the same words |
| D8 | The download page gains a per-card `onGithub`, not a per-store link model | No store is live (R7). A link model for a state that does not exist is guesswork; the comment says where it goes when one is |
| D9 | Identity by SHA-1 everywhere | Two same-named certificates in the founder's keychain (R4) |

## Out of scope, with where it is recorded

MSIX / Microsoft Store, the Mac App Store spike (`research.md` R6); Android signing in CI
(no GitHub package left to sign); automatic pre-release marking; the flaky Android unit
suite (ledger: 等待条件窄于断言).
