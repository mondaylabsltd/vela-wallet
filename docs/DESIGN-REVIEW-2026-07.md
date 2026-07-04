# 设计评审 · 2026-07-04

> 范围：底部 Dock、收款人选择器、批量导入、接收页（双 tab）、全 app 控件一致性。
> 方法：五个独立评审视角，每条结论锚定到实际渲染代码 + `DESIGN-LANGUAGE.md` 的约束条款。
> 严重度：P0 = 功能/正确性/不可见，P1 = 明显违反设计语言或层级错误，P2 = 打磨，P3 = 细节。

## 三大横切主题（最高杠杆）

1. **强调色纪律重建**：一条规则——**橙色 = 动钱或提交的动作**。今天 RPC 离线、退出登录、筛选 chip、主题选择器、保存图片、信息文本都穿着 CTA 的颜色；错误态甚至用 accent 而不是 `error.base`（theme.ts 特意把两色分开的意义被抹掉了）。纯样式值替换，无布局工作，是单笔最大的视觉信任修复。
2. **全 app 只允许一种分段控件**：现存 4 种方言（新 SegmentedToggle / Receive 自绘带阴影凸起 pill / BatchImport 旧凸起 pill / Settings 主题+头像的 accent 边框盒）。给 SegmentedToggle 加一个可选 `icon` 槽位，然后全部替换——删 ~90 行重复样式，免费修好 4 处 a11y 缺失（role/selected/haptic），动效统一。
3. **CTA 只走 VelaButton**：BatchImport 的 applyBtn、TokenSelector 的 sweepConfirm、Receive 的警告确认按钮都是手搓的，半径/字重/禁用态/loading 四不一致。

## Dock（WaveDock.tsx）

- [P0] **层级三重矛盾**：几何捧扫码（居中悬浮+唯一阴影）、颜色捧接收（唯一 accent）、商业模式捧发送（收入=中继费）。接收还是 dock 上"能动性最低"的动作。→ 收/发做成对等的中性对（hairline 边框对），accent 留给真 CTA；若坚持 dock 有 accent，只能给发送。可辩护的例外：余额为零时接收临时点亮。
- [P0] **暗色模式发送键隐形**：`bg.sunken`(#0F0F0D) 在 `bg.raised`(#1E1E1B) 上 ≈1.15:1（WCAG 1.4.11 要 3:1）。暗色里 sunken 比 raised 更暗，浅色关系被反转。→ 两个 pill 用 `bg.base` + 1px `border.strong`。
- [P1] **波浪装饰实际不可见**：注释声称"柔和阴影定义边缘"，但阴影被注释掉了——整个波浪剪影 ≈1.04:1，内容会从 FAB 两侧的透明月牙里穿出来。SVG + 半像素缝 hack + 拉伸 bug，全是成本零回报。→ 二选一：删掉 SVG 用平底 + 1px 顶部 hairline（扫码变普通 44pt 图标钮）；或真的给波浪描边。
- [P1] **图标语义冲突**：`Download` 同时=「收款」(dock) 和「存图到磁盘」(接收页)，一步之遥两个意思；activity 行已确立 `ArrowDownLeft/ArrowUpRight` 对。→ dock 换箭头对；存图换 `ImageDown`。
- [P1] 发送 pill 内图标 `fg.muted` + 文字 `fg.base`——半暗图标读作半禁用。
- [P1] `DOCK_CLEARANCE=112` 硬编码 < 实际 86+insets（120），列表最后一行永久被压 8pt。→ 从 WaveDock 导出 `DOCK_BAR_HEIGHT`，Home 用 insets 计算。
- [P2] 浅色模式接收键白字于 #E8572A ≈3.6:1（15pt 需 4.5:1）。
- [P2] `bg.sunken` chip 同屏两义：SegmentedToggle 里=选中态，dock 里=可按动作。
- [P2] 中文文案：接收/发送是文件传输语域，主流钱包用 **收款/转账**；zh-TW 已经漂移到 收款，三个中文 locale 互相不一致。→ 全 app 统一（dock+页标题一起改）。
- [P3] `preserveAspectRatio="none"` 让凹槽随屏宽拉伸（SE 上 71pt 贴脸，Pro Max 96pt 空旷）；`shadowColor:'#000000'` 硬编码违反 token 规则。

## 收款人选择器（ContactPicker.tsx）

- [P1] **扫码行是 accent.soft 大色块**（空态时是全 sheet 最响的元素，却是最次要的动作）——同时违反 de-container/轻控件/accent 纪律三条。→ 改普通行：`ScanLine` 放 40px `bg.sunken` 圆（和头像押韵），无背景无箭头。
- [P1] **文案撒谎**：「扫码添加地址」实际只填充收款人，什么都不保存。→ 「扫码填写地址」。
- [P2] **空态是死胡同**：一行干巴巴的话，没有下一步；姐妹组件 ContactsManager 的空态有标题+提示+添加按钮。→ 补提示行「粘贴地址或扫码即可直接转账」+ 安静的「添加联系人」文字钮。
- [P2] accent 堆积：输入新地址时 useRow 是第二块 accent.soft 板 + accent 书签图标 + 分组行 accent 图标盒——一屏 4+ 个 accent 元素零个真 CTA。→ 全部降为普通行/sunken 圆/文字「保存」。
- [P2] 暗色泥泞：`bg.base` 内圆在 `accent.soft`(#2C1A12) 上像被打穿的洞。（随色块删除自愈）
- [P2] IA 错标：零转账记录的已存联系人落在「最近」下；无收藏时 section 无标题渲染成无名块。→ 兜底段落标「联系人」，永远渲染标签。
- [P2] **全文件零 a11y**：6 个 Pressable 没有任何 role/label，书签图标钮对读屏完全沉默；X 触target ~38px < 44 底线。
- [P3] 三条左边线互相不齐（行 4px / 扫码行 12px / 段标 4px）；行间无 hairline 分割线；自绘 sectionTitle 硬编码 fontSize:10 与 SectionLabel 原语竞争。
- [P3] 扫码路径判定：picker 内入口有存在的理由（流内唯一扫码），但不配当横幅。→ SendScreen 地址框图标区加一个安静的 `ScanLine` 图标（一步直达），picker 行降级为普通行。

## 批量导入（BatchImportSheet.tsx）

- [P0] **「导入 0 位收款人」**：空态同屏三个 0，禁用 CTA 像机器自言自语。→ `capped.length===0` 时隐藏合计行、CTA 标签用无计数的「导入收款人」；>0 才切 `batchApply`。
- [P0] **显示汇率可能与实际计算不符**：`toFixed(2)` 把 <$0.005 的价格显示成 "1 TOKEN = 0 USD"，但换算仍按真值跑；用户一碰输入框 `parseFloat("0")` 又把每行清零。金额 UI 里显示值≠计算值不可接受。→ 有效数字格式化（类 `toPrecision(4)`），toFixed(2) 只留给法币合计。
- [P1] 旧分段控件复活（刚在首页杀掉的凸起 pill 原样出现，且零 a11y 零触感）。→ 换 `<SegmentedToggle>`。
- [P1] 汇率行排版破碎：小标签 + 巨大加粗可编辑数字 + 单位流放到最右，且没有任何"可编辑"信号。→ 一行连续句「1 USDC = 7.16 CNY」，可编辑段加 hairline 下划线，「自动」推到行尾。
- [P1] 中文量词：「0 位收款人」（位是敬语量词，配 0 像渲染 bug）；英文复数 bug：`n` 插值渲染 "1 recipients"（此 sheet + MultiRecipientEditor + SendScreen 三处）。→ i18next `count` + `_one` 键。
- [P1] 「导入文件」accent 泄漏（孪生「模板」却是 muted）；CTA 手搓不走 VelaButton，禁用态是大片洗白橙色板。
- [P2] 卡中卡中卡：sunken rateCard ⊃ raised currencyPill ⊃ sunken 圆——三层交替背景，暗色下几乎不可读。→ de-container：SectionLabel「汇率」+ 两行平铺 + hairline。
- [P2] 信息顺序：单位切换在粘贴框之下，却定义粘贴框里 "5000" 的含义；「按法币计价」不如「按 CNY 计价」（死键 batchUnitFiat 证明本来就想这么做）。
- [P2] 汇率加载/失败完全沉默（占位 "0" + 死 CTA 无解释）；超上限与无效行两条通知互斥只显示一条；无名行地址上下重复渲染两遍。
- [P3] 「模板」无动词无反馈（存了 CSV 悄无声息）；`overBalance` 借用 alert 标题当行内警告；X/「自动」触target 不足 + 无 a11y。

## 接收页（ReceiveScreen.tsx）

- [P1] **accent 层级双倒挂**：「保存图片」穿橙色而真主操作（复制地址/复制收款链接）是灰的；「任意数量的 ETH · Ethereum」信息文本也是橙色半粗——且与 40px 下方的代币行逐字重复。→ 每 tab 恰好一个 accent 动作（地址 tab=复制地址，请求 tab=复制收款链接）；信息行整行删除（导出图片模型里保留）。
- [P1] **12 网络宫格 = card pile + 假可点**：12 个 sunken pill（该形状在设计语言里=筛选控件）实为静态 View，吃半屏。→ 最优解：SectionLabel 下一条 22px 链 logo 折行带 + 一句「同一地址，通用于全部 12 个网络」，名字进 a11y 标签——省 ~40% 屏幕。保守解：去掉填充改开放行。
- [P1] 第三种 tab 方言（凸起 pill **+阴影**，正是新组件注释里点名移除的反模式；中文 2 字 vs 4 字被 flex:1 硬挤）。→ 换 `<SegmentedToggle>`（顺手扫 PayScreen 的第四处）。
- [P2] **警告门每次进入都重弹**：`warningDismissed` 是组件态不持久——高频页面永久性重复弹「我已了解」，训练用户不读就点。→ 持久化 per-account 标记，后续访问降级为 QR 下一行灰字提示。
- [P2] tab 切换时脊柱跳动：请求 tab 往 QR 卡里塞 summary 行、复制行在内容/按钮间原地变身、下半屏每次切换重播入场动画（违反规则 10）。
- [P2] 存图无进行中状态（disabled 但视觉不变）；入账 feed 全绿+0.7 透明度小字（暗色 ~3.5:1 不达标）；4 个 Pressable 无 a11y。
- [P3] 请求 tab 看不到收款地址（无法自查）；「已复制！」感叹号 off-voice + 行宽跳动；标题偏 5px；3 个死键（zh 组合还是坏中文）；4 处硬编码值。

## 建议实施顺序

1. **P0 批**：暗色发送键隐形、导入 0 位收款人、汇率显示≠计算、（+接收警告门持久化）。
2. **P1 批 A（一次全局扫）**：accent 大扫除（错误态→error.base、选中态→中性、次级动作→muted）+ 中文 收款/转账 统一。
3. **P1 批 B（控件统一）**：SegmentedToggle 加 icon 槽 → 替换 Receive/BatchImport/Settings 两选择器；三个手搓 CTA → VelaButton。
4. **P1 批 C（结构）**：12 宫格 → logo 带；ContactPicker/BatchImport de-container；dock 波浪去留决策 + 图标对 + DOCK_CLEARANCE。
5. **P2/P3**：空态、通知并存、a11y 补课、copy 打磨。
