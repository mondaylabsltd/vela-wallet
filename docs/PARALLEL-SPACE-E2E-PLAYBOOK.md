# Parallel-Space E2E Playbook

> **How to add automated e2e for any Vela Wallet screen, using the parallel space.**
> Next time: *"参考 docs/PARALLEL-SPACE-E2E-PLAYBOOK.md，给 <页面/流程> 加平行宇宙自动化测试"* → follow this.
>
> Prereq context: the parallel space itself is documented in [PARALLEL-SPACE.md](./PARALLEL-SPACE.md). This doc is the **testing recipe** on top of it.

---

## 0. The one idea

The parallel space is the **real app with only the passkey faked** (a fixed keyset → deterministic fixture wallet). So *every* screen can be driven end-to-end, deterministically, on real backends — without a biometric prompt. Automating a screen = drive the **real production screen** and assert its real behavior.

## 1. Golden rules (do not violate)

1. **Test the REAL screens, pixel-for-pixel.** Never build a mock UI. Reach screens through their real routes; the only entry that's parallel-specific is `/parallel` (which arms the mode and redirects into the real app).
2. **Select by what's on screen** — exact **English text**, **placeholder**, or an **existing** `testID`. Do **not** invent production testIDs (keeps prod pixel-identical). Grep the anchor in `src/i18n/locales/en/*.json` to get the exact string.
3. **Never spend funds.** Drive up to the confirm/sign step and **Reject**, or stay read-only. Completing a send/approve hits the real bundler. (For a genuine on-chain test, see §6.)
4. **Assert from the controllable side.** For dApp-connection flows, fire from the test dApp and assert the round-trip response there; for in-app flows, assert the rendered screen state.
5. **Must run on a DEV build** (`__DEV__` true). In production the passkey override + badge are compile-time no-ops. Point Playwright at a dev server with `E2E_BASE_URL`.

## 2. The harness (`e2e/support/`)

- **`parallel.ts`** — helpers:
  - `openWalletConnect(page)` — `/parallel` → real Home → **Connections** tab → paste field ready. (Waits for the `parallel-space-badge`.)
  - `connectWallet(page, url)` — paste a relay connect URL; waits for the connected card ("Vela Test dApp" / "Active").
  - `stubWalletNetwork(page)` — stub external RPC/bundler with fast benign replies (localhost passes through). Use for hermetic UI runs.
  - `openTestDapp(ctx, relay, session)`, `request(dapp, method, params, interact)`, `requestInstant(dapp, method, params)` — dApp-side driving.
  - `clickSheetButton(page, label)`, `confirmSheet(page)` — signing-sheet buttons.
  - `gnosisBalanceWei(addr)`, `bundlerGasAccount(chainId, safe)` — on-chain pre-checks.
  - Constants: `RELAY_PORT`, `FIXTURE.{one,two,three}` (the fixture Safe addresses).
- **`relay.js`** — a local RemoteInject relay + a self-contained test dApp page. **Only needed for dApp-connection flows.** In-app flows (Send/Receive/Settings/…) don't need it.

## 3. Recipe — add an e2e for a screen

1. **Find the real route + entry.** How does a user reach it from Home? (e.g. Send button, Settings gear, a tab, a token row.) The parallel space lands you on the real Home via `/parallel`.
2. **Collect stable anchors.** Read the screen; for each button/label/placeholder you'll target, grep the exact English string:
   ```
   python3 -c "import json;d=json.load(open('src/i18n/locales/en/<ns>.json'));print(...)"
   ```
   Prefer text/placeholder. Existing `testID`s: only `parallel-space-badge`, `receipt-card`, and the test-dApp `dapp-*` ids exist repo-wide.
3. **Write `e2e/parallel-<flow>.spec.ts`** (skeleton in §5).
4. **Run it** (§4) and iterate on selectors from the failure snapshot (`test-results/**/error-context.md` has the page's accessibility tree — read it to see the real rendered text).

## 4. Running

```bash
# One dev server (DEV build → __DEV__ true; REQUIRED). Leave it running.
npx expo start --web --port 8092

# Point the suite at it (webServer is skipped when E2E_BASE_URL is set):
E2E_BASE_URL=http://localhost:8092 npx playwright test parallel-<flow>.spec.ts --reporter=list --workers=1

# Debug one test + see the page tree on failure:
E2E_BASE_URL=http://localhost:8092 npx playwright test parallel-<flow>.spec.ts --grep "<title>" --timeout=45000
cat test-results/*/error-context.md   # rendered accessibility tree at failure
```

## 5. Spec skeleton (copy this)

```ts
import { test, expect, type Page } from '@playwright/test';
import { stubWalletNetwork } from './support/parallel';

test.describe('parallel-space · <flow>', () => {
  let page: Page;

  test.beforeEach(async ({ context }) => {
    page = await context.newPage();
    await stubWalletNetwork(page);              // omit if you want real reads
    await page.goto('/parallel');               // arm mode → redirect into real Home
    await expect(page.getByTestId('parallel-space-badge')).toBeVisible({ timeout: 25_000 });
    // Navigate to the real screen (e.g. tap a real button/label):
    // await page.getByText('Send', { exact: true }).click();
  });

  test.afterEach(async () => { await page?.close().catch(() => {}); });

  test('<high-value assertion>', async () => {
    // drive with text/placeholder selectors; assert real state; REJECT before any submit.
  });
});
```

For a **dApp-connection** flow, follow `e2e/parallel-dapp.spec.ts` / `parallel-clear-signing.spec.ts` (they add the relay + test dApp). If a spec keeps its pages open across tests (`beforeAll`), **close the context before `relay.stop()`** (SSE keep-alive hangs teardown otherwise).

## 6. Real on-chain (opt-in, real xDAI)

Only when you want a genuine settlement. See `e2e/parallel-onchain.spec.ts` (`RUN_ONCHAIN=1`, `@onchain`).
- The Safe's own xDAI is the **transfer value**.
- The bundler pays **gas from a separate per-Safe deposit address** — `GET /v1/account/{chainId}/{safe}` → `activeDepositAddress`. Fund THAT, or use the in-app "request sponsorship".
- Parallel One (Gnosis) gas deposit: `0xb32a3965c4823ea426de52c7e869dd0cfe154d03`.

## 7. Gotchas (learned the hard way — check these first when a spec misbehaves)

| Symptom | Cause / fix |
|---|---|
| Badge / passkey mock not active | Not a dev build. `__DEV__` must be true → use `E2E_BASE_URL` to a `expo start --web` server. |
| Badge not found though mode is active | The flag lives on `globalThis.__VELA_PARALLEL__` (Metro double-bundles the service). Wait for `getByTestId('parallel-space-badge')`. |
| "Connected" not found | The Connections card shows **"Active"**, not "Connected". The connected dApp name comes from relay metadata ("Vela Test dApp"). |
| Slide-to-confirm text not found | "Slide to confirm" is an **accessibilityHint only**, not visible text. Assert a nearby visible label instead (e.g. "Spending cap", "Unlimited"). |
| personal_sign message assertion fails | A message with **non-ASCII bytes renders as hex**. Use ASCII, or assert the hex. |
| `afterAll` hangs (dApp specs) | `relay.stop()` waits on keep-alive SSE. The relay's `stop()` calls `server.closeAllConnections()`; also **close the browser context before** `relay.stop()`. |
| jest (not e2e) `__DEV__ is not defined` | `jest.setup.js` defines `global.__DEV__`. Mock `react-native` if a test imports the real passkey module. |
| Sheet shows real fee/simulation even when "stubbed" | The parallel space uses **real Gnosis reads**; the stub is best-effort. Assert what actually renders (fee/simulation), reject before submit. |

## 8. What's covered / what's next (importance-ranked backlog)

**Done:**
- dApp connect + sign (all methods, approve/reject/switch/disconnect) — `parallel-dapp.spec.ts`
- clear-signing (all 25 scenarios + intent spot-checks) — `parallel-clear-signing.spec.ts`
- RPC-failure banner — `parallel-rate-limit.spec.ts`
- opt-in on-chain — `parallel-onchain.spec.ts`
- **Receive** (anti-poisoning mask + QR + networks + copy + EIP-681 builder) — `parallel-receive.spec.ts`
- **Home** (account switcher across the 3 fixtures + Activity⇄Connections tabs) — `parallel-home.spec.ts`
- **Send** entry smoke (picker + search) — `parallel-send.spec.ts`

**Backlog (build in this order):**
1. **P0 — Send deep flow**: needs the real Gnosis xDAI balance loaded (warm the Home balances first, then navigate to Send via the dock — a fresh `goto('/send')` has a cold cache and shows "No matching tokens"). Then: select funded token → recipient/amount → **first-time-recipient risk badge** → slide-to-confirm → **reject** (never Continue). Plus split (1→N) / sweep (N→1) via `preselectedMulti`.
2. **P2 — Settings**: language switch (a label re-renders), currency/number-date format, custom network + endpoint health, RPC providers, the quiet "Feedback" row.
3. **P1 — Onboarding** (parallel, mock passkey): clear wallet → welcome → name → consent boxes → register → Safe derived → sync (3 retries) → Home; plus the sync-failure state.
4. **P1 — token detail / activity detail / assets sheet / history** (needs warm real balances like Send).

_(This backlog is refined by `docs/TEST-OUTLINE.md`'s Epics; keep them in sync.)_
