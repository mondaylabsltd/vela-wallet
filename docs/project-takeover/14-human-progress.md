# 14 — 人类接管训练进度 (Human Progress)

> 跨会话状态文件。每次训练开始先读本文件，结束必须更新。
> 等级定义:L0 不知道 / L1 能识别 / L2 能解释作用 / L3 能追踪完整流程 / L4 能独立改+测+排障 / L5 能讲取舍+设计演进+担生产责任。

## 当前状态

- **HUMAN TAKEOVER READINESS: `NOT READY`**
  - 证据:基线诊断 11 题+5 追问(2026-07-02)。不能说出测试/部署命令(D9/D11=L1);服务端密钥清单认知错误("都不需要密钥",实际 getvela.app 3 枚 secret + bundler EOA 私钥,D10 一票否决级);代码位置能力弱(说不出 service 名)。
  - 强项:设计意图/恢复模型/取舍理由(创始人本人拍板的部分)达 L3;推理能力强(2-b 自我修正、6-a 连点)。
  - 画像:**"AI 写码创始人"** — 产品与架构意图熟,实现与操作生。训练重心=操作技能+代码地图,不是概念课。
- 基线 commit: `73d7aac`(2026-07-02)
- 训练阶段: 基线诊断✅完成 → 最短路径已制定 → 待开始单元 0

## 知识域等级(D 编号见 09;证据引本次诊断题号)

| 域 | 描述 | 基线 | 目标 | 证据(诊断题) |
|----|------|-----|------|-------------|
| D1 | 项目目的/核心流程 | **L3** | L4 | Q1/Q2:定位、商业模型、bundler→EntryPoint→Safe 链路正确;MetaMask 对比未答透 |
| D2 | 代码结构/模块边界 | **L1** | L4 | Q5.1:有排查方法论但说不出 service 名;键空间不知(Q6.1) |
| D3 | passkey→Safe 身份链 | **L3** | L5 | Q3/3-a:attestation 一次性✓、失效两级分级✓、暂存窗口自己指出✓;但全是概念,零代码引用;Gnosis 依赖条件说错(实时路径是 HTTP API);索引后端陈述待对质(外部仓库) |
| D4 | 交易提交链(4337) | **L2** | L5 | Q2/2-a/2-b:UserOp/EntryPoint/initCode 经追问正确(先答错 MultiSend 部署,自我修正);字段记不得;MultiSend 三场景✓漏 Tempo |
| D5 | dApp 签名安全 | **L2** | L5 | Q4:"改成 0"错(实为 user-chosen finite,approval-guard.ts:12);模拟防线全漏;enforceNoUnlimited 兜底不知;授权检测与 clear-signing 边界混淆 |
| D6 | RPC 池/余额 | **L2** | L4 | Q5:multicall 机制✓;显示铁律答反("让用户换 RPC" vs 实际"回退缓存永不清零") |
| D7 | 状态/持久化 | **L2** | L4 | Q6/6-a:介质半知,vela.* 不知;pending-at-submit 时机未说出;"全可丢"设计观✓且 6-a 推理出唯一副本窗口✓但不知代码是否已实现 |
| D8 | Expo/RN 框架运行时 | **L2** | L4 | Q7:四框架分工✓;expo-router 漏;CF Pages✓;Hermes 坑不知(polyfills 分叉) |
| D9 | 测试体系 | **L1** | L4 | Q8:工具名✓;规模/命令全不知;parallel space 不变量不完整;改 safe-transaction 只答"跑单测" |
| D10 | 安全边界 | **L1** | L5 | Q10.2:"不需要密钥"重大错误;bundler EOA 凭据全漏;never-unlimited 机制不知(Q4.3) |
| D11 | 生产运维 | **L1** | L5 | Q9:构建命令不知;CF Pages 回滚机制✓;不知道仓库有 CI(169c02f) |
| D12 | 外部服务/故障域 | **L2** | L5 | Q10.1:bundler 写死读活分区✓;Q3 索引单点✓;bundler 报价角色/underfunded 高频故障未提 |
| D13 | 架构取舍 | **L3** | L5 | Q11:no-viem 真实理由(供应链威胁模型,xlsx 事件自证)✓;代价浅;推翻条件回避 |

## 最短掌握路径(按依赖+收益排序)

| 单元 | 内容 | 目标 | 预计 | 验收任务 |
|------|------|------|------|---------|
| **U0** | 操作生存:package.json scripts 全读→亲手跑 tsc/lint/jest/e2e/build:web;读 05/06 runbook | D9→L3, D11→L2 | 1h | 不看资料背出提交前门禁清单+全绿截图 |
| **U1** | 密钥与故障面:全系统凭据盘点(getvela.app secrets/bundler EOA/keystore 计划);bundler 故障诊断路径+underfunded 弹窗机制 | D10→L3, D12→L3 | 1.5h | 手写密钥清单卡片(存放/后果/轮换);口述 bundler 半夜故障 runbook |
| **U2** | safe-transaction.ts 精读:sendUserOp 十步、UserOp 字段、gas 定价权威原则、Tempo 报销 | D4→L4 | 3h | 白板画链路;预测 3 个故障场景;首个无 AI 演练 |
| **U3** | 签名安全三件套:detect/rewrite/enforce + simulation + clear-signing 边界 + SigningSheet 单渲染路径 | D5→L4, D10→L4 | 2.5h | 反向讲解;指出一个新签名路径必须过的关卡 |
| **U4** | RPC 池(7 级优先/评分/两级封禁)+余额铁律+持久化(pending-at-submit/tx-reconciler/vela.*) | D6→L4, D7→L4 | 2h | 用故障注入(vela.*)复现并解释行为 |
| **U5** | 框架坑:expo-router/polyfills-Hermes/__DEV__ vs dev_unlocked/parallel space 不变量 | D8→L3, D9→L4 | 1.5h | 解释 fixture 风险修复(_layout.tsx) |
| **U6** | 身份链代码化:attestation-parser/validateCreateClientData/upload 重试队列;对质 p256-index 后端陈述 | D3→L5 | 2h | 从代码证实/证伪自己的后端陈述 |
| **U7** | 取舍深化:no-viem 迁移风险分析、counterfactual 取舍、演进条件 | D13→L5 | 1.5h | 写 ADR 式分析进 13 号文档 |

穿插:U2 起每单元配无 AI 演练(见 12 号文档);1/3/7/14 天复习节奏见下方队列。

## 训练日志

### 2026-07-02 会话 1(commit 73d7aac)
- 内容:首次训练。验证审计文档抽查全吻合;创建 09/14;基线诊断 11 题+5 追问,全部 13 域覆盖。
- 关键正确:bundler EOA→EntryPoint→Safe;initCode(修正后);attestation 一次性;失效分级;multicall;CF Pages 回滚;no-viem 供应链理由。
- 关键错误:MultiSend 部署(已自我修正);approval 改写值"0";余额铁律反向;"服务端不需要密钥"(最危险);不知 CI 存在;测试/部署命令为零。
- 提示等级:—(诊断阶段无提示)。手工任务:—。
- 诊断中的验证动作:public-key-index.ts 端点覆盖(:23-24)/查询路径(:80);approval-guard.ts:12 改写语义;SigningRequestModal.tsx 调用点。
- 下次动作:**U0(操作生存)**——最高杠杆,不会跑测试则一切修改无法验证。
- 待办:题库(10/11)/演练(12)/ADR(13)由后台工作流生成,生成后需人工抽查再启用。

## 复习队列(1/3/7/14 天)

| 知识点 | 首学 | 复习到期 | 方式 |
|--------|------|---------|------|
| initCode vs MultiSend 部署裁决 | 07-02(诊断中自我修正) | 07-03 / 07-05 / 07-09 / 07-16 | 变换题型:给一个未部署账户的 UserOp 十六进制片段,指出部署信息 |
| 索引服务实时路径=HTTP API 非 Gnosis RPC | 07-02(诊断中纠偏) | 同上节奏 | 画恢复时序图 |

## 薄弱项清单(按危险度)

1. **密钥清单认知错误**(D10)——错误认知比无知危险,U1 强制修正
2. 测试/部署零操作能力(D9/D11)——U0
3. 余额铁律方向反(D6)——U4
4. approval 改写机制误解(D5)——U3
5. 代码地图缺失(D2)——贯穿各单元用"先定位再讲解"训练法
