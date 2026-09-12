# 035 · Desktop Message Signing — results

分支 `035-desktop-message-signing`,叠在 034 上。

## Phase 1 — 一个注释自己说了、代码却没写的分支

`signing_host` 里那句注释写着:"交易从它的调用解码;**类型化数据和纯文本消息是同一架梯子上
各自的一级**"。而它下面那个 `match` **没有消息那一档**。

后果:核心把一条登录请求的**解码文本、SIWE 字段、域名绑定、危险等级**全都算好了,
桌面把它们全扔了 —— 而 `ClearSurface::MessageSign` 这一屏从 spec 022 起就画着、就接着视图。
**签名面板上最该说话的那一屏,从来没被启动过。**

现在:`personal_sign` → `MessagePresented{PersonalSign}`;
`eth_sign` → `MessagePresented{EthSign}`,**不是**那个平静的消息视图 ——
它签的是一个**不透明的哈希**,核心给它硬警告(它自己的注释,引自 `SigningSheet.tsx:465-470`)。

两处细节:

- **origin 是浏览器的事实,不是页面的自称**;空字符串按 `None` 传 —— SIWE 的绑定检查
  把"没有 origin"当作**无法绑定**,而不是当作匹配。
- **params 原样、按序、只留字符串**。机器是**按位置**读 `params[0]`/`params[1]` 的,
  把一个对象塞进那张表会把消息和地址整体错位一格。

抽成 `clear_kickoff` 函数(而不是内联 match),这样这张映射表不需要窗口、传输层和一个 dApp
就能测。四条测试:消息那一档、`eth_sign` 不是平静视图、另外两档没被改动、
非字符串参数被丢弃而不是被字符串化。

**没在实机验**:要一个真 dApp 发一次 `personal_sign`。今天这台机器的合成点击不可靠
(032 交接已记),而内置浏览器要点好几层。映射有四条测试守着,渲染那一半从 spec 022 起
就在跑。

desktop **350 / 346**(+4 测试),fmt clean,画廊全渲染,Windows 通过。
