# Feature Specification: First Run, and a stable app in an unstable environment

**Feature Branch**: `038-first-run-parity` (off `main`, after 037 merged)

**Created**: 2026-09-11

**Status**: Draft

**Input**: Founder, with three screenshots of the running apps:

> 桌面版这个页面展示的不太好，因为它完全就是移动网页设计啊 … 而且
> app-desktop 似乎缺少了这个页面。以及 … 有一个从 "The unstoppable Ethereum
> wallet" 到 "no seed phrase to copy" 的闪烁，每次刷新页面都会闪烁，理论上不该
> 这样吧。我们现在只关心 app-web 和 app-desktop 的表现。

> 这个 app-desktop 中的 settings 按钮不需要，因为如果遇到要设置的话，会自动弹出
> 需要设置的东西对不？所以不需要提供一个首页的设置入口。

> app-desktop 版本老是弹出来，但是 app-web 版本没有，我感觉这个体验很差，好像在
> 说我们的公钥索引服务器一直在出错。而且点 close 关闭不了。

> 而且你有没有发现 app-desktop 相比 app-web 很多地方字很粗重了，我希望保持一致。

> 我们要在一个不稳定环境中搭建一个稳定的用户使用体验。你深刻理解这句话，并检查
> app-web app-desktop 还有哪些不稳定的地方，我们要把它修复优化好。

> #188 #189 #190 #191 这几个 issue 是别人发现的问题，你要整合进来理解，并一起放到
> 038 中解决。

> [caBLE 日志：tunnel 握手完成 → getAssertion → 手机端 CLOSE → 又发了一帧] 发生
> 报错然后页面卡死了。

> #190 图标要用更加合适的，比如 `~/Documents/ChatGPT/zzzz/assets/passkey-icons`
> 这几种。如果是平台，那就确定知道当前是哪个平台就用哪个。

> 现在网页版本 app-web 仅仅支持平台，这不对，它也要支持多种选择，比如
> clearsigning 那种方式。

> app-web 的 Meta-tag inspector：og:title / og:image / twitter:image /
> twitter:card / og:site_name 全缺，og:description 154 字符太长；以及 logo 用错了
> favicon。

> 资产发送失败了，存储的 activity 中却是成功了；发送确认界面没有展示全总览，
> 接收人数很多时希望能看到有多少个接收地址、能细看每一个及其头像；等待上链过程
> 中最好有一个预估倒计时；以及视觉上要优化 UI/UX。

> 还发现几个问题：1 添加 celo 网络时显示不兼容，实际上 celo 可以兼容，expo/web
> 都可以添加；2 某个代币的交易记录列表能看到，但没法看某条记录的详情，只能从活动
> 里看；3 设置的日期格式 13.06.2026 没生效；4 服务节点要新增对 AAGUID 的支持
> (aaguid-explorer.awesometools.dev)；5 语言设置显示"简体中文"，但网址是 /en、
> 页面是英文——应该显示当前是英文；6 导入收款人按 CNY 计价 / 按 USDT 数量的 tab 无法
> 切换，汇率无法修改；7 发送 ETH 时输入金额的输入框很丑，地址展示不全；8 "部分代币
> 无法获取价格"的弹层只展示每个网络的余额，要展示哪些代币没有价格。

> cargo run app-desktop 版本时，在这个开发环境下，无法选择使用 this device 登录，
> 这体验很差，很影响开发和测试，能不能自动完成相关配置，因为我记得之前不知道用了
> 什么 env 前缀运行 cargo run 就可以的。

## Why

Every defect the founder found is in the same thirty seconds: the launch
animation, the intro, and the front door — the only part of this product a
stranger sees before deciding whether to give it a key. Nothing else in the app
gets judged that fast, and nothing else is judged with so little to go on.

They are not unrelated bugs either. Three of them are the same mistake made
in three places: **the first-run screens were built as a phone and the desktop
was left to inherit them.** The web intro is a phone column centred in a 3840px
window. The desktop app never got the intro at all. The desktop app plays its
launch animation every single time, because the replay rule was only ever
written down in the web's HTML. The rest are one cluster: the endpoint sheet on the
desktop front door — a permanent entry to it, a modal that will not close, and a
sentence that blames our own index service for a failure that happened inside
the machine.

## Part A — the first thirty seconds

### What is wrong, precisely

1. **The web intro is the one first-run screen that never got a desktop
   composition.** `IntroCarousel.svelte`'s only `min-width: 1280` rule caps the
   column at `--layout-flowColumn` and centres it; everything else about the
   screen is the phone layout — `space-between` over `100dvh`, dots and a
   full-width button riding the bottom of the viewport. Its two neighbours were
   fixed for exactly this in spec 019: Welcome (`[locale]/+page.svelte`) and the
   create journey (`FlowShell.svelte`) both grow `OnboardingRail` at 1280 and
   size their buttons to their labels, with the rail's own file recording the
   reason — *"on a desktop window it opened a hole that read as a phone page
   pulled tall."* So a desktop visitor meets the hole on screen one, and then
   watches the app change its mind about what it is on screen two.

2. **Welcome flashes into the intro on every load.** `intro` starts `false` so
   the prerendered document is the landing page, and `onMount` raises it
   (`+page.svelte`, `if (shouldShowIntro()) intro = true`) — i.e. after the
   landing page has painted. Spec 012 already solved this exact problem for the
   launch animation and wrote down why: *"Deciding in `onMount` means the
   prerendered page paints first and is only then covered, so the user sees
   Welcome → animation → Welcome. This must be an inline, render-blocking
   script."* The intro gate never got that treatment. It repeats on every
   refresh because `vela.intro.seen` is written on LEAVING the intro, so
   reloading while still inside it is still a first run.

3. **`app-desktop` has no intro.** There is no module for it; the desktop goes
   launch animation → Welcome. The illustration contract has expected it since
   spec 020 — `intro-illustrations.json`'s `consumers` line reads *"web inline
   `<svg>` · android ImageVector · ios vela-core rasterizer · **desktop
   resvg**"* — and the corpus keys (`onboarding.intro.*`) already ship in all
   15 locales.

4. **The desktop front door carries a settings entry no other shell has.**
   `ui/rail.rs` gives the rail a permanent settings affordance; it opens a sheet
   holding exactly one field, the passkey-index URL. That sheet already has two
   doors of its own: it **opens itself** when the health probe reports the index
   unreachable (`onboarding.rs`, *"Opened by the probe, once. A person who
   dismissed it has answered."*), and the warning line on Welcome is itself a
   button that opens it. The web's Welcome has no entry at all — only the
   warning sentence. The founder's reading is correct: the permanent entry is a
   third door to a room the app already takes you to when you need it, and it
   spends the calmest corner of the front door on a field almost nobody will
   ever type in.

5. **(Found while checking 4) The desktop plays its launch animation on every
   start.** The 7-day replay rule is real on the web (`vela.launch.played`,
   604800000 ms, decided before paint in `app.html` and asserted by
   `constants.test.ts`). On the desktop the animation is constructed
   unconditionally unless `VELA_SKIP_LAUNCH_ANIMATION=1` — an env switch for
   tests, not a gate. A person who opens their wallet four times a day sits
   through it four times.

6. **The endpoint sheet cannot be closed.** `render()` re-opens it on the very
   next frame:

   ```rust
   // Opened by the probe, once. A person who dismissed it has answered.
   if self.login_view.endpoint_unreachable && self.endpoint.is_none() && !self.creating {
       self.open_endpoint(true, cx);
   }
   ```

   The dismissal is recorded as `endpoint = None`, which is the same state that
   triggers the automatic open — so Close hands it straight back. The field that
   was meant to prevent this exists and says so in its own doc comment
   (`EndpointSurface::automatic` — *"Dismissing it must not re-open it on the
   next frame, so the automatic open happens once"*), but it is only ever read
   to decide whether the card shows its warning line. The comment describes a
   behaviour the code does not have. While the probe keeps failing, the front
   door is a modal with no way out.

7. **A failure inside the machine is reported as our service being down.** The
   sheet says *"The Passkey Index service is unreachable"* and offers to change
   its URL. On 2026-09-11 the deployed registry answered its health probe in
   about a second — `{"service":"webauthn-p256-publickey-registry","status":
   "ok","version":"2.1.1"}` — while the desktop app kept showing that sentence.
   The reason is local: a shell exporting `all_proxy=socks5://127.0.0.1:1080`
   where nothing listens on 1080, and `ureq` reads `ALL_PROXY` before
   `HTTPS_PROXY` (`executor/proxy.rs` says so in its own notes). Every registry
   call goes to a dead port and comes back classified `network: true`, which is
   the one bucket that means "the service was not there". The browser ignores
   environment proxies entirely, which is the whole reason the web never shows
   this and the desktop always does. `classify()` is right that nobody but the
   shell can tell a transport failure from a 4xx — but a transport failure has
   at least two causes, and the app currently names the wrong one out loud.

8. **The desktop is set in a different typeface, one notch heavier.** The web
   declares `--font-ui: 'Plus Jakarta Sans', 'Noto Sans SC', system-ui` and
   ships the faces with the app (`@fontsource/plus-jakarta-sans` 400/500/600/700,
   Noto Sans SC, IBM Plex Mono, imported in `routes/+layout.svelte`). The
   desktop names a family for MONO only — `theme::font_mono()` → Menlo /
   DejaVu Sans Mono — and never names one for anything else, so every other
   string in the app renders in gpui's default: the host's system font. It also
   asks for more weight than the web does at the same place:

   | element | web | desktop |
   | ------- | --- | ------- |
   | button label | `--weight-semibold` (600) | `FontWeight::BOLD` (700) |
   | wordmark | `--weight-bold` (700) | `FontWeight::EXTRA_BOLD` (800) |
   | addresses | IBM Plex Mono | Menlo / DejaVu Sans Mono |

   So the founder is seeing at least three things at once — a different
   typeface, a heavier weight asked for on shared controls, and whatever the two
   rasterisers do differently — and the first two are ours to fix. 144
   `FontWeight::` call sites across the desktop crate is the size of the audit.

9. **`cargo run` cannot use "this device", and the app blames the wrong
   thing.** macOS hands the platform authenticator only to a process with an
   application identifier — a signed `.app` with a bundle id, the
   associated-domains entitlement and (for a team signature) an embedded
   provisioning profile. `target/debug/vela-wallet` has none, so
   `ASAuthorization` refuses with *"The calling process does not have an
   application identifier"*, and the sheet answers *"Make sure Face ID, Touch
   ID or a fingerprint is set up on this device"* — advice for a problem the
   person does not have. The recipe exists
   (`scripts/build-macos-app.sh`, `VELA_SIGN_IDENTITY` + `VELA_PROVISION_PROFILE`,
   deliberately not `--deep`) and the dev path exists too — the parallel space,
   `cargo run --features dev-fixtures` with `VELA_PARALLEL_SPACE=1`, where
   `vela-core`'s fixed keyset signs in a passkey's place. Neither is one
   command, and neither is what the failure sheet says.

### Scope (Part A)

1. A desktop composition for the web intro, built from the pieces its
   neighbours already use — the rail, the flow column, buttons at their labels'
   width — so that Welcome, the intro, and the create journey read as three
   screens of one app at desktop widths. The phone layout is unchanged.
2. The intro decided **before first paint**, the way the launch animation is:
   one render-blocking decision, the rule duplicated deliberately and pinned by
   a test that reads both copies (the precedent is `constants.test.ts`). The
   prerendered landing page stays the landing page for crawlers and for
   everybody whose first run is over.
3. The intro on `app-desktop`: the same three slides, the same corpus keys, the
   same three illustrations from the contract, the same once-only rule, sitting
   in the same place in the boot order (launch animation → intro → Welcome).
4. The rail's settings entry removed on the desktop, leaving the two doors that
   fire when the setting actually matters.
5. The endpoint sheet made dismissible — the dismissal recorded somewhere the
   automatic open reads — and demoted from a modal across the front door to
   something that does not stand between a person and the two ways in.
6. An honest sentence for a transport failure: "this machine could not reach it"
   is a different fact from "the service is down", and the app knows which one
   it has whenever the failure is a refused connection to a proxy it took from
   the environment. **Recommended, for the founder to rule on**: when that
   proxy refuses the connection, try once directly before declaring anything
   unreachable — a dead `ALL_PROXY` in a developer's shell must not look like
   our outage.
7. The launch animation's replay rule on the desktop, from the same numbers the
   web uses.
8. One typeface across both shells: the desktop loads the faces the web already
   ships and names them for its UI text, its mono, and its CJK fallback (the
   corpus runs to 15 locales, so the fallback is not a detail), and every
   `FontWeight::` in the crate is re-checked against the web's token at the same
   place. Where they disagree the web's value wins — it is the one that has been
   through a design pass.
9. One command each for the two desktop dev paths: the parallel space (no
   authenticator at all) and a signed bundle (the real ceremony), so "run the
   desktop app and sign in" stops being a recipe to remember. Plus the failure
   sheet telling the truth: an unbundled binary is not a missing fingerprint,
   and the sheet should say which one it met.
10. A failed hybrid ceremony takes its own card down: the touch state is
    cleared on every exit of `assert_hybrid` / `register_hybrid` (success,
    failure, cancel), the goodbye is skipped once the tunnel has closed, and
    the failure sheet names what the phone did (no credential vs cancelled).
11. Passkey method icons from the founder's set, as a `currentColor` contract
    ported to both shells; "This device" resolves to the running platform's
    icon; the sign-in picker on the web shows the same three rows as create
    and as the desktop, built so a fourth row is one entry.
12. A sweep of both shells' first-run and front-door surfaces at the sizes they
   actually ship at (a desktop window, a maximised desktop window, and the
   narrow web widths), so the founder is not the regression test.

### Out of Scope (Part A)

- **`app-ios` and `app-android`** — handed to a colleague on 2026-09-05. The
  illustration contract and the corpus keys stay shared, so a later port
  inherits this work rather than re-deciding it.
- The intro's argument, copy, and illustrations. They are ported and
  re-composed, never redesigned.
- Welcome's own behaviour: create still navigates, sign-in still runs in place.
- The signed-in shells — wallet, settings, contacts, explore. Their desktop
  layouts are 033–037 territory and are not reopened here.
- Any new corpus key. Everything this cut draws already ships in 15 locales; a
  new key would put 038 behind a translation pass for a layout fix.

### Success Criteria (Part A)

- **SC-411**: at a desktop width the intro is composed like the two screens
  either side of it — the same rail, the same measure, buttons at their labels'
  width — and no first-run screen has a growing hole in its middle as the window
  gets taller.
- **SC-412**: below the desktop breakpoint the intro is pixel-unchanged, drag
  and all.
- **SC-413**: loading the web app on a first run never shows the landing page
  first. Nothing but the app's own ground appears before the intro does.
- **SC-414**: the prerendered HTML of every locale still contains the landing
  page, and the SSR e2e that guards it stays green.
- **SC-415**: the pre-paint rule and the module's rule are asserted to agree by
  a test that reads both, so a change to one fails the build rather than a
  screen.
- **SC-416**: `app-desktop` shows the three intro slides once, on a first run,
  after the launch animation and before Welcome — paged by drag, by keyboard,
  and by the button, with the same skip.
- **SC-417**: the desktop's three illustrations are the contract's paths, proven
  against the contract by a test, not eyeballed.
- **SC-418**: the desktop front door has no settings entry, and pointing the app
  at an unreachable index still puts the endpoint field in front of the person
  without them looking for it.
- **SC-419**: Close closes. Dismissing the endpoint sheet leaves it dismissed
  for the rest of that run, with the probe still free to complain the next time
  it is asked — proven by a test, because this is exactly the class of bug a
  manual pass declares fixed and the next frame undoes.
- **SC-420**: whatever the endpoint's state, the two ways in on the front door
  are reachable and pressable without dismissing anything.
- **SC-421**: with the index healthy and the environment naming a proxy that
  refuses connections, the desktop app reaches the registry and says nothing —
  or, if the founder rules against the direct retry, says that THIS MACHINE
  could not get out, and never that the service is down.
- **SC-422**: a second desktop launch inside the replay window opens straight
  into the app; one after the window plays the animation again — the web's
  numbers, proven by a test that reads both.
- **SC-423**: the same string on the same control is the same typeface and the
  same weight in both shells, checked side by side at the same size — starting
  with the two the audit already found (button label, wordmark).
- **SC-424**: the desktop's faces travel with the app. A machine with none of
  them installed renders the same text as one that has them all.
- **SC-425**: one documented command puts a developer in front of a working
  "this device" sign-in on macOS, and one puts them in the parallel space; both
  are in the desktop README and neither needs a remembered env prefix.
- **SC-426**: an unbundled binary meeting the platform authenticator says so —
  the sheet names the missing application identifier, not a missing fingerprint.
- **SC-427**: a phone that closes the tunnel mid-assertion leaves the front
  door usable: the touch card is gone, the failure sheet says what happened,
  Back works — driven by a test that closes the port under the client.
- **SC-430**: every method row on both shells shows its icon; "This device"
  on a Mac shows the Apple mark, on Windows the Windows mark, in Chrome on a
  Mac the Chrome-on-Mac mark; the web sign-in shows the three rows.
- **SC-431**: verified on the running apps, not in a unit test — a first run and
  a second run, on the desktop build and in a desktop browser, screenshot to
  screenshot.

18. **A phone that hangs up leaves the front door frozen.** The founder's log:
    tunnel up, Noise handshake complete, `→ CTAP getAssertion`, then
    `← tunnel CLOSE … "Peer sent a close frame"` — the phone ended the session
    (no discoverable credential for `getvela.app`, or a cancel on the phone).
    In `executor/passkey.rs::assert_hybrid`, `(ceremony.touch)(None)` — the
    line that takes the "Check your phone" card down — runs only AFTER a
    successful `.assert(...)`; the `?` on failure returns before it. Nothing
    else clears it: `channel.close()` is called in exactly one place,
    `leave_create`, which sign-in never reaches. So `touch_waiting()` keeps
    answering, `touch_prompt()` keeps drawing the card over the failure sheet
    (the screenshot: "Check your phone" on top of "Sign In Failed", Back
    dimmed), and the only way out is to quit. Same class as finding 6 — state
    that records "a sheet is up" with nobody responsible for taking it down on
    the failure path. The `→ tunnel frame (48 bytes)` after CLOSE is the
    goodbye written into a closed socket; harmless, but it should not be
    attempted.

19. **The passkey method rows have no icons, and the web sign-in has no
    rows.** (#190, ruled — and revised the same evening: "this device" shows
    THE DEVICE, a laptop or a phone by form factor, never Apple's, Google's or
    Microsoft's mark, because the platform authenticator on a given machine is
    not reliably any of them; "phone or tablet" shows the camera that scans;
    the USB key keeps its mark; and the web's "I already have a wallet" opens
    a sheet — a dialog on desktop — on every width.) The rows get real icons — the founder's set:
    Apple passkey, Windows passkey, Google Password Manager, Chrome-on-Mac,
    FIDO2 security key, USB security key — and **"This device" shows the icon
    of the platform the app is actually running on**, decided at runtime: the
    desktop by `cfg!(target_os)`, the web by the UA's platform (macOS Safari →
    Apple; macOS Chrome → Chrome on Mac; Windows → Windows; Android/ChromeOS →
    Google Password Manager). The six SVGs are Figma exports wrapping a
    base64 inner SVG with `#5F6368` baked in; they are re-expressed as
    `currentColor` paths in a contract both shells port (the intro-art
    pattern), so dark mode and the accent work without a second set.

20. **The web offers ONE way in where the desktop offers three.**
    `+page.svelte:126` hardcodes `login.dispatch({ type: 'sign_in', method:
    'platform' })`; the desktop's `signin_method_card` lists platform / phone /
    security key. The browser's own `navigator.credentials.get()` picker does
    cover all three transports, which is why nobody noticed — but a wallet
    living on a phone or a key deserves a row that says so, the same three
    rows creating a wallet already shows (`AddMethodPicker`), and the picker
    is where the clear-signing method's fourth row will land (its own spec).

## Part B — a stable experience in an unstable environment

The founder's sentence, taken literally: **the person's network, proxy, shell,
browser cache and clock are not ours to trust, and none of them may turn into a
screen that says Vela is broken.** Every finding below is a place where one of
those unstable inputs reaches the screen unfiltered.

### What is wrong, precisely

10. **On macOS the desktop has no system-proxy story at all.** `proxy.rs`
    reads WinINET on Windows and `gsettings` on GNOME, and on macOS returns
    `None` on the stated premise that *"a system proxy there is installed into
    the network stack, so a direct connect already goes through it."* That is
    true of a TUN-mode tool and false of the ordinary HTTP/SOCKS proxy in
    System Settings → Network, which only CFNetwork clients honour — a Rust
    socket bypasses it. The people who need a proxy most are exactly the ones
    this leaves out.

11. **The environment outranks the system, and the decision is made once per
    process.** `system_proxy()` takes `ALL_PROXY`/`HTTPS_PROXY` from the
    environment first (*"the more specific statement"*) and caches the answer in
    a `OnceLock` for the life of the process. A GUI app's environment is
    whatever launched it — Finder gives one, a terminal another, an IDE a third
    — so the most unstable source is trusted most, and a proxy that dies or is
    changed while the wallet sits in the tray stays dead until restart. This is
    the mechanism behind finding 7: a stale `all_proxy` from a shell turned
    into "the Passkey Index service is unreachable".

12. **There is no fallback chain.** One route is chosen; if it refuses the
    connection the request is classified `network: true` and that is the
    answer. Nothing tries the next candidate (system proxy → environment
    proxy → direct) before telling a person a service is down.

13. **A worker thread that panics takes the wallet with it.** Ten
    `thread::spawn` sites, no `catch_unwind`, no panic hook; 50 `unwrap`/
    `expect` in non-test code (18 in `launch_animation.rs`, 6 in `proxy.rs`, 5
    in `passkey.rs`). A USB key yanked mid-ceremony or a malformed RPC body
    hitting the wrong `unwrap` on a background thread is the whole app gone,
    with no sheet and no log a person could send us.

14. **Errors are told to stderr, not to the person.** 34 `eprintln!` sites
    are the desktop's only channel for things like *"fee: relay estimation
    unavailable"*, *"in-band: estimation failed, using defaults"*, *"tempo:
    estimation failed, using defaults"*, *"send: unhandled relay error"*. The
    screen then shows a fee that is a default dressed as an estimate. A wallet
    may degrade; it may not degrade silently on the money line.

15. **The web shows "$0" when it means "I could not look".** In
    `balance_dashboard.rs`, `FetchErrored` sets `bootstrapped = true` and keeps
    last-known tokens — correct with a cache. On a **first launch with no
    network** there is nothing known: `unknown` flips false, `display_total_usd`
    becomes `Some(0)`, no chain is recorded as failed, so `balance_partial` is
    false and `live.ts`'s `zeroLive` reads it as a settled empty wallet. The
    person sees $0.00, not "unreachable". (To be confirmed under the fault
    harness before it is fixed — the reasoning is from the code, not a run.)

16. **The web app has no offline state and no version-skew handling.** No
    `navigator.onLine` listener anywhere in `app-web`; no `hooks.client.ts`,
    so no `handleError`; SvelteKit's version polling is not configured. A
    deploy while somebody has the wallet open means the next lazy chunk is a
    404 and the app is broken until they think to reload — and the intro,
    balance and signing surfaces are all lazy. An offline minute means the RPC
    pool bans endpoints on their cooldown schedule (`COOLDOWN_BASE_MS` 30 s,
    doubling to 5 min) with nothing on screen saying why, and a reconnect does
    not lift the bans.

17. **One web fetch with no timeout.** `passkey-directory.svelte.ts` fetches
    the AAGUID directory with a bare `fetch()`; every other call goes through
    `net.ts`'s per-class timeout table. It is decorative and its failure is
    caught, so a hang costs a row its icon rather than the flow — but it is
    also the only one, so it should not stay the only one. (The RPC pool,
    registry and selector lookups are all correctly on `AbortController`.)

Things checked and found **sound**, so nobody re-audits them: the RPC pool's
six-tier scoring, four-way error classification and all-banned self-rescue are
in the core and shared by both shells; the relay's `probe_treasury` returns
`Unknown`, never `Uncovered`, on a timeout; `chainlink.rs` keeps a last-good
price map and answers `None` rather than `1` for a coin it cannot price; the
web's FX rule is `null ≠ 1`; the desktop's storage writes beside the file and
renames over it; both shells keep an unconfirmed registry publish for the next
launch; every `localStorage` read in `app-web` is behind a `try`.

### Scope

11. A macOS system-proxy reader in `proxy.rs`, the same shape as the GNOME
    branch: `scutil --proxy` (or `SCDynamicStoreCopyProxies`) → HTTPS, HTTP,
    SOCKS, in that order, half-configured entries skipped.
12. The proxy decision re-ordered — **system setting first, environment
    second, direct last** — and re-evaluated on failure rather than cached for
    the process: a candidate that refuses the connection is skipped for the
    next, and the answer is re-derived after any transport failure, so a proxy
    that comes back or a setting that changes is honoured without a restart.
13. A transport failure that exhausted every candidate is reported as **this
    machine could not get out** — a different sentence, a different sheet,
    and never a modal on the front door; a service that answered anything at
    all is reported as what it said.
14. A panic boundary around every worker thread and a process panic hook that
    turns a panic into the failure sheet with the report button — the same
    sheet a ceremony failure already gets — so the worst case is a screen, not
    a vanished window.
15. The `eprintln!` money paths promoted to view state: an estimate that fell
    back to a default says so on the fee line, in the corpus's existing words
    for an estimate that could not be made.
16. "Unreachable" as a first-class home state on both shells, distinct from
    "empty": a first launch with no network shows the skeleton and a reason,
    never $0.00. Confirmed against the fault harness before the change and
    after it.
17. On the web: an online/offline listener that lifts the pool's bans on
    reconnect and names the state on screen; SvelteKit version polling with a
    reload on a failed lazy import, so a deploy never strands an open wallet.
18. The one bare `fetch()` moved onto `net.ts`'s timeout table.

## Part C — what other people found

Four issues from an outside tester, all on the web at
`wallet-sveltekit.getvela.app`, each pinned to its cause:

- **#188 — the total balance streams in for 10+ seconds and keeps changing.**
  Cause, in the core: `display_total` is the LIVE sum of whatever chains have
  arrived (`ChainAssetsArrived` merges per chain), so the number climbs
  $62.56 → $98.84 as the seven networks answer one by one. And the transient
  *"Some tokens couldn't be priced"* is `BalanceNotice::Unpriced`, which
  fires whenever `has_unpriced` is true and no chain has failed — which is the
  normal state mid-stream, before prices have arrived. The `refreshing` pill
  exists; the figure under it still moves. Fix: while a refresh is in flight
  the figure is the last settled total (cached, or the previous live sum) and
  the pill says updating; the live sum replaces it once, at settle; `Unpriced`
  is only ever raised at settle. Both shells read the same view, so this is a
  core change with two screenshots.
- **#189 — the tab shows the Svelte logo.** `$lib/assets/favicon.svg` IS the
  SvelteKit template's logo, and `routes/+layout.svelte:91` links it in
  `<svelte:head>` after the real sailboat icons from `app.html`, so it wins.
  Fix: delete the file and the line.
- **#190 — the three "Add a passkey" icons do not read as one set.** They are
  not icons: `AddMethodPicker.svelte` draws three empty bordered boxes of
  different proportions; the desktop's picker draws none. **Ruled** (finding
  19): real icons from the founder's set, "This device" resolved to the
  running platform's mark, the same rows on both shells.
- **#191 — a contact suggested from history cannot be named.** The core
  distinguishes `ContactSource::Manual` from `Auto` and its `Save` event names
  any address, so the rule permits it; tapping a row opens the same detail and
  `edit` calls `openEdit(selectedAddress)`. The gap is in the shell's detail
  for an `auto` row — reproduce first, then make "name this contact" the
  primary action on a suggestion, which promotes it to `Manual`.

- **#meta — a shared link to the wallet renders as an anonymous text card.**
  `routes/[locale]/+page.svelte`'s `<svelte:head>` sets `<title>`,
  `description`, canonical and hreflang and nothing else: no `og:title`,
  `og:description`, `og:site_name`, `og:image`, `og:url`, `og:locale`, no
  `twitter:card` / `twitter:image`. The en `metaDescription` is 154 characters
  (previews truncate around 125). And until #189 lands the favicon a card
  falls back to is the Svelte logo. Fix: one `SocialMeta` block on the landing
  page per locale — `og:site_name` "Vela Wallet", `og:title` = `metaTitle`,
  `og:description` = a NEW ≤ 110-character `metaSocialDescription` key in all
  15 locales, `og:url` = canonical, `og:locale` + `og:locale:alternate`, a
  static 1200×630 `og-image.png` (the sailboat mark on the brand ground,
  rendered at build from an SVG in `static/`, one image for every locale —
  text-free so it needs no translation), `twitter:card` =
  `summary_large_image`, `twitter:image` = the same PNG.

## Part D — the send flow, after the button

Four founder findings on the same journey. Pinned as far as the code allows;
the first needs a reproduction before it is fixed.

- **#D1 — a send that failed is recorded as a success.** Both shells write the
  record `pending` at submit and flip it from the receipt: the web's
  `waitForReceipt` throws on `resolution.failed` and the executor marks the
  record `failed`; the desktop's tracker turns `resolution.confirmed = success
  != false` into `Receipt` vs `ReceiptFailed` and the core patches the record.
  So a receipt whose `success` is `false` IS handled. What neither shell
  handles: **a UserOp whose `success` is `true` while the Safe call inside it
  failed.** Safe's `execTransaction` returns `false` and emits
  `ExecutionFailure(bytes32, uint256)` instead of reverting, and a 4337 module
  that does not bubble that up reports the op as successful — the wallet then
  writes `confirmed` for a transfer that never happened. `grep ExecutionFailure`
  across the core and both shells finds nothing. To reproduce: a send whose
  inner call reverts (a token transfer over balance, on the parallel space)
  and the record's status afterwards. Fix, in the core tracker: a confirmed
  receipt whose authentic logs carry `ExecutionFailure` from the sender is
  `ReceiptFailed`, and the receipt screen says the payment did not go through.
- **#D2 — the confirm screen hides the recipients.** `live-send.ts`'s SD3
  builder gives a split send one amount line and a `SummaryLine` "Total · N
  recipients"; the N addresses, names and identicons are on the FORM screen
  only. The desktop's confirm mirrors it. Fix: a recipients block on the
  confirm — the count as its title, the first rows inline, "all N" opening the
  full list in the third column (desktop / wide web) or a sheet (phone), each
  row with identicon, name-or-short-address, and amount — and the same block
  on the receipt and in the activity detail.
- **#D3 — waiting for the chain has no clock.** The receipt stage shows
  "submitted" and a spinner with no sense of how long is normal. Fix: a
  per-chain typical inclusion time in the core's chain table (`network_admin`)
  — from block time × the relay's usual inclusion depth — and a receipt stage
  that shows elapsed against typical ("usually about 15 s on Gnosis"), moving
  to "taking longer than usual — still submitted, we keep checking" past 2×,
  never to failure (invariant ⑤ stands: a slow poll is not a failed payment).
- **#D4 — the send journey's visual pass**: confirm, waiting and receipt on
  both shells re-checked against the design language (hairlines, open heroes,
  subordinated fiat, accent only on the value-moving action) with the split
  case at 1, 3, 12 and 60 recipients.

## Part E — eight more from the founder's pass (2026-09-11, evening)

- **#E1 — Celo reads as incompatible.** The compatibility check probes eleven
  `REQUIRED_CONTRACTS` by `eth_getCode` on the best RPC and calls the P-256
  verifier; Celo (42220) has at least the CREATE2 deployer and the Safe
  singleton factory at their canonical addresses (probed live from
  `forno.celo.org`: 70 bytes of code each), and the web and Expo clients add
  it. The module doc admits the trap: *"rpcFailed and truly-incompatible both
  flatten to `not-compatible`"* — so a probe that could not reach the RPC
  (the desktop's proxy path, Part B) reads as an incompatible chain. Fix:
  keep the two verdicts apart on every shell (a failed probe says "could not
  check", with retry), and run the eleven probes + P-256 against Celo on the
  desktop to see which one actually fails.
- **#E2 — a token's transaction rows do not open the detail.** `History`
  passes `onclick` to `ActivityRow` and the page routes `go('tx-detail', …)`;
  `TokenDetail.svelte` renders the same `ActivityRow` with no handler. The
  detail already resolves a feed item by id (`findFeedItem`), so the token
  screen's rows only need to name theirs.
- **#E3 — the date format "13.06.2026" did not take.** Reproduced on both
  shells (2026-09-11, a record seeded at 2026-06-13 12:00). **Web**: the
  detail column read `13.06.2026 12:00 PM` — the preference works — but the
  desktop-width feed printed no day heading at all (the desktop layout
  iterated the groups without the day line the phone layout prints), so
  outside the detail there was nowhere for a date to show. Fixed: the
  desktop layout prints the day heading. **Desktop**: the number, date and
  time rows in Settings → Localization were the mock's literals with one
  drawn menu — a pick changed nothing — and every date was ISO, every clock
  24-hour, every figure comma-dot. Fixed: a stored choice (the web's own
  words) with "Automatic · System" resolved from the machine's locale the way
  the web resolves it from the browser's; the three menus pick and persist;
  every day heading, detail stamp, deposit clock, token amount and fiat
  figure reads the presets in force (SC-450).
- **#E4 — AAGUID lookups go to a third party.** The catalog is vendored in
  `vela-core` (never queried for known models), but an unknown AAGUID is
  looked up at runtime from `aaguid-explorer.awesometools.dev` by every shell.
  The founder wants our own service node to serve it: a proxy/cached copy on
  our worker (cross-repo, `biubiu-projects`), so a person's authenticator
  model is not sent to a stranger's server and the lookup works behind the
  GFW. This cut records the client change (endpoint from the same settings
  object as the index); the server is a separate task.
- **#E5 — the language row ticks the stored choice, not the active one.**
  `liveLanguageRows` ticks `preferences.language`; the route was `/en` and
  the page English. The row must tick the locale the page is IN, and choosing
  a language must navigate to that locale's route (the setter today only
  stores).
- **#E6 — batch import: the unit toggle does not switch and the rate cannot
  be edited.** Reproduced on the web at desktop width (2026-09-11): the
  "Import list" door opened the importer as the third column with the
  FIXTURE drawn over a live session — "In CNY / In USDT", "1 USDT = 7.25
  CNY", three sample rows, "Import 2 recipients" — while every tab press and
  keystroke went to the real machine, which nobody could see. The importer
  is a sheet on the phone and a column body on the desktop; the live overlay
  handled only the sheet. Fixed: the desktop body is overlaid too, and the
  overlay now words the tabs, the rate label and the hint from the currency
  in force and the token being split, and the button offers what parsed
  (none / one / many). The rate is editable in place with "back to auto"
  (SC-447).
- **#E7 — the split recipient row.** The amount `<input>` is a fixed
  `calc(--space-2xl × 6)` box and the address input shares one line with the
  pick button, so at four recipients the address is cut to `0x3187ł` and the
  amount reads as a stray box. Fix: two lines per card (ordinal, then the
  full-width mono address with ellipsis), the amount sized to its content and
  right-aligned with the symbol, a hairline under each editable field so the
  affordance is visible without a browser border.
- **#E8 — "some tokens couldn't be priced" opens a dialog that does not name
  them.** `liveBalanceDetail` lists pending networks and settled networks;
  the core already hands over `unpriced_tokens`. Fix: a third section listing
  each unpriced token (symbol, network, balance) — the sentence on the hero
  is a link, and the dialog must answer it.

### Success Criteria (Parts B and C)

- **SC-430**: on a Mac whose only proxy is the one in System Settings, the
  desktop reaches the registry, the relay and the RPCs with no environment
  variable set.
- **SC-431**: with a dead proxy in the environment and a live one in the
  system (and vice versa), the desktop reaches the network without a restart;
  with neither, it says this machine could not get out.
- **SC-432**: a deliberate panic on a worker thread (test seam) produces the
  failure sheet with the report button; the window stays.
- **SC-433**: a fee that fell back to a default is labelled as such on screen,
  on both shells.
- **SC-434**: a first launch with the network cut shows "unreachable", never
  $0.00, on both shells — driven by the fault harness.
- **SC-435**: going offline and back online in the web app names the state
  and recovers without a reload; a deploy mid-session reloads on the next
  navigation instead of breaking a lazy import.
- **SC-436**: (#188) the home's figure changes at most once per refresh, at
  settle; the unpriced notice never appears while chains are still arriving.
- **SC-437**: (#189) the tab shows the sailboat in a fresh incognito window.
- **SC-438**: (#190) the three method rows read as one set, and the desktop's
  rows match the web's.
- **SC-440**: (#meta) the landing page of every locale carries the full
  og/twitter set; the inspector reports no missing tag; `og:description` ≤ 110
  characters in every locale; the image is 1200×630 and served from the site.
- **SC-441**: (#D1) a send whose inner Safe call fails is recorded `failed`
  on both shells and the receipt screen says so; reproduced on the parallel
  space before and after.
- **SC-442**: (#D2) at 12 and 60 recipients the confirm shows the count and
  opens the full list with identicons; the receipt and the activity detail
  show the same list.
- **SC-443**: (#D3) the waiting stage shows a typical-time estimate per chain
  and moves to "longer than usual" past 2× without ever calling the payment
  failed.
- **SC-444**: (#E1) Celo can be added from the desktop; a probe that fails
  says "could not check" with retry, never "incompatible".
- **SC-445**: (#E2) a row on a token's detail opens that transaction's detail.
- **SC-446**: (#E5) the language row ticks the page's locale; choosing another
  navigates to it.
- **SC-447**: (#E6) the unit toggle switches; the rate is editable and can be
  returned to auto; the recipients recompute.
- **SC-448**: (#E7) at four recipients every address reads in full or with an
  ellipsis, and the amount field is sized to what it holds.
- **SC-449**: (#E8) the balance dialog lists every unpriced token by name and
  network.
- **SC-450**: choosing "13.06.2026" in Settings → Localization changes the day
  heading of an older record on the wallet feed to that form on both shells,
  and the choice is still in force after a relaunch.
- **SC-439**: (#191) a history-suggested contact can be given a name from its
  detail, and the name survives a reload.

## Assumptions

- **"所有 app-web / app-desktop 的 bug 和优化" is read as: 038 is the
  container.** Part A owns the first run and the front door, Part B the
  stability of both shells in an unstable environment, Part C the issues other
  people report; findings the founder adds in these two shells are appended
  here until it merges. Two things stay out on purpose: the signed-in screens'
  layouts (033–037's), and the **clear-signing key method** (a new `KeyMethod`
  across the core, the signing page, and both shells — its own spec, with the
  founder's ruling already taken: **the key's rpId is `getvela.app`**, folded
  like every other Vela key, so the page is a policy the wallet enforces, not a
  credential the wallet cannot reach).
- The web's Part B fixes are made in the shared core wherever the rule lives
  there (`balance_dashboard.rs`, `rpc_pool.rs`), so the desktop inherits them
  by construction rather than by a second pass.
- The intro stays once-only (founder, 2026-09-01 — *"不然就很烦"*). Nothing here
  reopens that.
- Removing the rail entry costs exactly one thing: changing the index URL
  *before* the probe has complained. That is acceptable because after this cut
  the stored value can only be the default or something typed into the sheet
  that the probe itself opened.
- `results.md` is written as the work lands, per 033–037.
