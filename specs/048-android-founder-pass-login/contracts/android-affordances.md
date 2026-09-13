# Contract — every dead control and what it does after 048

Row numbers follow the audit table (results.md will carry the after-state).

| Row | Control | Action |
|---|---|---|
| 1 | copy (R1 rows, R2 address/contract, A2 address/hash, T2 facts, SD4 hash, contact 复制地址) | `Clipboard.copy` → tick only on success; `haptic select` |
| 2–3 | A1 / T1 row tap | `onNavigate(TxDetail/TokenDetail, id)` → sheet; no phantom step |
| 4 | 保存图片 | real code, web composition, `Gallery.save` → snackbar; share stays |
| 5 | contact 转账 / 收款 / 二维码 | send.open(prefilled_recipient) / receive / contact QR sheet |
| 6 | 群发转账 | send.open + seedSplit(members); 1 member → prefilled form |
| 7 | contacts 新建分组; group ⋯ rename/delete | GroupEditSheet → saveGroup; menu → saveGroup(id,name) / deleteGroup(confirm) |
| 8 | SD1 class chips | `sendClassFilter` → visibleSendTokens |
| 9 | 全部网络 pill (SD1/A1/T1) | chain sheet → `ChainFilter` → lists narrow; feed `chain_filter_changed` |
| 10 | identicon (12 sites) | NavHost-hosted `IdenticonViewerSheet(address)` |
| 11 | hero amount | `togglePrivacy` |
| 12 | hero status line | rescue sheet (RpcFix / BalanceDetail bodies hosted from the home) |
| 13 | 字号 | `VelaSlider` drag+tap, detent haptic |
| 14 | scanner 翻转 | lens flip when a front camera exists; else disabled |
| 15–17 | 在区块浏览器中查看 (R2/A2/T2) | `openUrl(explorer_url + path)` |
| 18 | T2 转账 | send.open(preselected_symbol, preselected_network) → form |
| 19 | group ⋯ | menu sheet |
| 20 | network detail overrides | `VelaUrlField` editable → `override_field_edited/blurred` |
| 21 | "+ 添加网络" foot | add-network flow |
| 22 | picker 扫码 row | scanner → recipient |
| 23 | picker group rows | seedSplit(members) |
| 24 | contact 复制地址 | row 1 |
| 25 | home rows | recorded (P3, animation) — no change |
| 26 | T2 收款 | receive with the token's own code |
| 27 | T2 activity rows | tx detail |
| 28 | A2 删除记录 | feed delete with confirm |
| 29 | T3 原生币 tab + chain pick | network_admin wizard events |
| 30 | split card 通讯录 pick | `openRowPicker(id)` |
| 31 | scanner look | brackets at the web weight; one status line |
| 32 | share card look | row 4 |
| 33 | 字号 haptic | row 13 |
| 34 | contact 最近往来·全部 | history filtered to the contact |
| 35 | contacts swipe 转账/删除 | send.open / delete confirm |
| 36 | export | CSV / JSON choice sheet |
| 37 | add-network custom RPC + 重新检查 | editable field; recheck event |
| 38 | provider 检查密钥/获取密钥, drpc link | `provider_test_requested`; `openUrl` |
| 39 | language sheet contribute link | `openUrl` |
| 40 | settings home rows; feedback box | rows absent on the live route; editable box |
