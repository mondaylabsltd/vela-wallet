# 字体选用原则

> 适用范围：Web（含浏览器扩展）、可信签名页（`sign.getvela.app`）、iOS / iPadOS、Android、桌面端（macOS / Windows / Linux），以及应用生成的图片（分享卡片、商店截图）。
>
> 本文是字体的唯一依据。`docs/design-system.md` 的排版章节、`docs/design-tokens.json` 的 `font.*` 与它冲突时，以本文为准。

## 1. 原则

**系统字体优先（System Fonts First）。** 产品不下载、不打包第三方字体文件，文本一律交给操作系统的原生字体显示。

目标不是让各平台的字形完全一致，而是让每个平台都用它自己原生、成熟、清晰的字体。系统字体还带来这些：

- 零字体体积，也没有字体加载造成的布局跳动（FOUT / CLS）；
- 跟随系统的字重渲染、Dynamic Type、OEM 字体（如小米 MiSans、三星 One UI Sans）以及用户的辅助功能设置；
- 字库更新由操作系统负责，不需要我们跟进上游版本。

字体按用途分为三个角色：

| 角色 | 用于 | 设计 token |
| --- | --- | --- |
| UI 无衬线 | 标题、正文、导航、按钮、表单、金额 | `font.sans` / `font.display` / `font.numeric` |
| CJK 无衬线 | 中文（简 / 繁）、日文、韩文 | 不单设 token，由 UI 字体按语言回退 |
| 等宽 | 地址、交易哈希、十六进制、代码、ID | `font.mono` |

`design-tokens.json` 里的字体名（Plus Jakarta Sans、IBM Plex Mono）只是 Penpot 设计稿作图用的字体。Penpot 必须用真实安装的字体才能绘制，这些名字只表示"这是哪个角色"，各端都按本文映射到系统字体，不读取这些名字本身。

## 2. 各平台实现

| 平台 | UI 无衬线 | CJK | 等宽 |
| --- | --- | --- | --- |
| Web | `--font-ui`，见第 3 节 | 由 `<html lang>` 交给系统回退 | `--font-mono`，见第 3 节 |
| iOS / iPadOS | `Font.system(size:weight:)`（SF Pro） | 系统自动回退（PingFang / Hiragino / Apple SD Gothic） | `Font.system(size:weight:design: .monospaced)`（SF Mono） |
| Android | `FontFamily.Default`（Roboto 或 OEM 系统字体） | 系统自动回退（Noto Sans CJK） | `FontFamily.Monospace`（Droid Sans Mono 等） |
| macOS 桌面 | gpui `.SystemUIFont`（SF Pro） | 系统回退 | Menlo |
| Windows 桌面 | gpui `.SystemUIFont`（Segoe UI） | 系统回退（Microsoft YaHei / JhengHei、Yu Gothic、Malgun Gothic） | Consolas |
| Linux 桌面 | 已安装的系统字体（见下文） | fontconfig 回退（Noto Sans CJK / 文泉驿等） | DejaVu Sans Mono |

桌面端（gpui）的注意事项：

- **等宽字体必须写真实存在的字体名。** 字体名写错时，gpui 会静默回退到 UI 字体，结果地址变成了比例字体（Windows 上就出现过）。
- **Linux 上 gpui 的 `.SystemUIFont` 不跟随桌面环境。** 它固定指向 "IBM Plex Sans"，普通发行版没有装这款字体，而走回退时会丢掉字重和 `calt` 设置。所以桌面端启动时由 `theme::settle_ui_family` 检查：如果 IBM Plex Sans 不存在，就改用第一个已安装的系统字体（Ubuntu Sans、Ubuntu、Adwaita Sans、Cantarell、Noto Sans、DejaVu Sans）。macOS 和 Windows 不受影响。
- **桌面分享卡片用 resvg 渲染。** resvg 0.45 不支持可变字体，而 SF Pro 只以可变字体形式提供，粗体会画成常规字重。所以卡片的字体栈以 Helvetica Neue、Segoe UI 等静态字体开头。

## 3. Web 字体栈

```css
--font-ui: system-ui, -apple-system, 'Segoe UI', Roboto, 'Noto Sans', 'Helvetica Neue', Arial, sans-serif;
--font-mono: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', 'DejaVu Sans Mono', monospace;
```

- **等宽字体要列出具名字体。** `ui-monospace` 只有 Safari 支持，Chrome 和 Firefox 会跳过它，回落到浏览器默认的 `monospace`，而这个默认值取决于浏览器设置，可能是 Courier。所以 Menlo、Consolas 等要写进字体栈里。
- **UI 字体同理。** 具名回退覆盖不认识 `system-ui` 的环境。
- **通用字体栈里不写具体的 CJK 字体名。** 汉字在中、日、韩之间统一编码，但字形写法各不相同。如果把 `PingFang SC` 或 `Microsoft YaHei` 放在前面，日文页面的汉字会显示成中文写法。页面的 `<html lang>` 已按语言设置（`zh` / `zh-TW` / `zh-HK` / `ja` / `ko`），浏览器会据此选用对应的系统字体。如果确实需要点名，只能写在 `:lang()` 规则里，并按语言分别点名（可信签名页的 `sheet.css` 就是这样做的）。
- 用一个字体栈解决问题，不按操作系统名称写分支判断。
- 不写 `@font-face`，不引用任何 `.woff2`。

## 4. 地址、哈希与数字

- **地址和哈希一律用等宽字体。** 每个字符宽度固定，逐字比对时位置能对齐。上表所列的系统等宽字体都能区分 0 / O 和 1 / l / I。
- **关闭上下文替换（`calt`）。** 有的字体会在数字后把 `x` 替换成 `×`、把 `-` 替换成 `−`，地址就会显示成 "0×14fB…"。凡是可能显示十六进制的文本（含 UI 字体），都要关闭 `calt`。这条规则成本为零，换了字体也继续保留。
- **金额、余额和数字列使用等宽数字**，不必换成等宽字体：Web 用 `font-variant-numeric: tabular-nums`，iOS 用 `.monospacedDigit()`，Android 用 `fontFeatureSettings = "tnum"`。
- **字体不承担安全保证。** 地址必须能完整查看、完整复制、独立校验，不能只靠"看起来像"。安全敏感界面中，文本的完整性和可核对性优先于视觉一致性。

## 5. 生成的图片

- **分享卡片**用当前系统的字体渲染。卡片文档没有 `lang`，所以字体栈在拉丁字体之后点名各语言的 CJK 字体。不同平台保存出的图片字形会略有不同，这是允许的；但版式不能依赖某款字体的精确宽度，长文本必须能换行或截断。
- **商店截图和宣传图**同样用系统字体（生成机器是 macOS，因此是 SF Pro 和 PingFang）。

## 6. 例外

只有在下列情况下可以引入第三方字体：

- 系统字体缺少某种语言或符号的覆盖；
- 测试确认系统等宽字体存在不可接受的辨识问题；
- 有明确的品牌视觉要求，例如官网 Hero 标题。

每个例外都必须登记在下表中，写明理由、范围（哪个平台、哪些页面）和体积。没有登记的字体文件视为违规。

| 字体 | 理由 | 范围 | 体积 | 日期 |
| --- | --- | --- | --- | --- |
| （无） | | | | |

例外字体只用 WOFF2，并且只包含必要的字重和字符子集。

## 7. 测试与验收

- **测试不能写死文本的绝对宽度或高度。** 系统字体在各操作系统上的度量不同，一台机器上量出的数值，换一台就会失败。测试应断言相对关系：不换行、不溢出、不重叠、与另一段同字体文本等宽。
- **人工验收覆盖：** macOS、iOS、Windows、Android 以及一个主流 Linux 发行版；英文、中文（简 / 繁）、日文、韩文，以及中英混排；标题换行、按钮宽度、表单布局、文字截断；地址、哈希、数字列对齐；缺少 CJK 字体的环境（如精简 Linux）下的回退表现。
- **依赖清单：** Web 的 `package.json` 中不应出现 `@fontsource/*`；各原生端的工程里不应打包字体文件，例外表中登记的除外。
- **运行时强制：** Web 的 CSP 设为 `font-src 'none'`（`src/lib/security-headers.ts`，有测试锁定），任何混进来的 `@font-face` 都会被浏览器拒绝加载。可信签名页的 CSP 不写 `font-src`，由 `default-src 'none'` 拒绝所有字体，`single-file-test` 会检查页面里一个字体都没有。登记例外时，要同时放开对应的规则。

## 8. 变更记录

| 日期 | 变更 |
| --- | --- |
| 2026-11-13 | 采用本原则。移除 Plus Jakarta Sans（Web、可信签名页、iOS、Android、桌面端，以及仓库里的 `assets/fonts/`）、IBM Plex Mono（Web）和 Noto Sans SC（Web）。iOS、Android、桌面端此前已经在用系统等宽字体和系统 CJK 字体。可信签名页因此出了新构建 `e8ce5134`，已加入 `BUILD_ALLOWED` 和 `FROM_BYTES`，但还不是 `LAUNCH`：要等 `dist/` 部署之后才能切换。 |
