<!-- vela-preview-guide -->
## 📱 测试员指南 — 怎么在手机上测这个 PR

**日常测试(纯 JS 改动的 PR,占绝大多数):**
1. 打开手机上已装的 **Vela Wallet 开发版**(dev build)
2. 扫上方机器人评论里的 **Update QR**(iPhone 扫 iOS 列、Android 扫 Android 列;iOS 可直接用系统相机,或用 App 首屏的 Scan QR code)
3. App 会直接加载本 PR 的代码,不需要登录 Expo 账号

**首次准备(每台手机只做一次):**
1. 注册 Expo 账号,并请管理员把你加进 monday-labs-ltd 组织(打开 Build 链接需要);**iPhone** 还需先注册设备——管理员执行 "eas device:create" 后把注册链接发给你
2. 用手机浏览器打开上方评论的 **Build Permalink** → Install,装上开发版 App

**什么时候需要重装 App?**
- 只有当机器人评论里的 **Runtime version 和你上次装的不一样**(说明这个 PR 改了原生代码)时,才需要重新走一遍 Build Permalink 安装;其余情况永远只扫码。

**常见报错:**
- `Expected MIME-Type … got 'text/html'` → 手机上装的是旧版 dev build(不支持热更新)。删掉重装本评论 Build Permalink 里的版本再扫。
- 扫码没反应 → 别用微信/其它扫码 App;iOS 用系统相机或 App 内扫码器。
