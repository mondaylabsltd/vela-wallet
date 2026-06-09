# Web QR Scanner — 技术总结

## 问题背景

Vela Wallet 的 Web 版（Expo Web）需要扫描 WalletPair URI 二维码来建立 dApp 连接。WalletPair URI 约 270 字符，生成 QR Version 10（57×57 模块），比普通以太坊地址的 QR（Version 3, 29×29）密集得多。

WalletPair dApp 使用深色主题，QR 码显示在深色背景上。

## 核心发现

### 1. iOS Safari 不支持 BarcodeDetector

BarcodeDetector API 在 iOS Safari 上 **disabled by default**（包括最新的 iOS 18.7）。只有 Chrome/Edge（桌面和 Android）支持。这意味着 iPhone Web 无法使用浏览器原生的 ML 扫码能力。

### 2. jsQR 无法处理真实相机/照片输入

jsQR 是纯 JavaScript QR 解码器（2020 年停更），对干净的数字截图有效，但无法处理：
- 相机视频帧的噪声、模糊、摩尔纹
- iPhone 拍照的 JPEG 压缩伪影
- 深色背景导致的安静区（quiet zone）丢失

### 3. zbar WASM 是唯一可行的解码器

zbar 是 C 语言条码库，编译为 WASM 后在浏览器中运行。它能处理 jsQR 无法解码的图片，但需要特定的图像预处理：

| 输入类型 | 有效策略 | 原因 |
|---------|---------|------|
| 截图 PNG (~1000px) | jsQR `invert@800` 或 `bin160@600` | 干净数字图像，二值化/反转恢复安静区 |
| 拍照 JPEG (3024×4032) | zbar **缩小到 1200px** | 缩小=低通滤波，平滑 JPEG 噪声和摩尔纹 |
| 相机视频帧 (1080×1920) | zbar **缩小到 1000px** | 同上，1000px 是速度和精度的甜蜜点 |

**为什么缩小有效**：浏览器 canvas drawImage 用双线性插值缩小图片，等于做了一次低通滤波，把高频噪声和摩尔纹平滑掉，QR 的边界变得更清晰。1200px 是最佳平衡点——噪声被去除但 QR 细节足够（~5px/模块）。

### 4. iPhone Safari 的文件输入需要挂载到 DOM

`document.createElement('input')` 创建的元素如果不 `appendChild` 到 DOM，Safari 不会触发 `onchange` 事件。

### 5. video 元素会拦截触摸事件

全屏 `<video>` 会拦截上方浮动按钮的点击，需要 `pointerEvents: 'none'`。

### 6. getUserMedia 需要 HTTPS

iPhone Safari 在 HTTP 下禁止 `navigator.mediaDevices`。开发环境需要用 Cloudflare Tunnel 等工具提供 HTTPS。

## 最终架构

```
Web 扫码
├── 相机扫码
│   ├── getUserMedia (1920×1080)
│   ├── 每帧 → canvas → 缩小到 1000px → zbar WASM
│   └── 复用单一 canvas（避免 GC 压力）
│
├── 图片上传
│   ├── input[type=file] 挂到 DOM → 选图 → 加载到 canvas
│   ├── zbar WASM @[1200, 1000, 800, 600, 400]（逐个尝试）
│   └── fallback: jsQR + binInvert(160) / invert / binarize(160)
│
└── zbar WASM 加载
    ├── 从 jsDelivr CDN 动态加载（绕过 Metro 的 import.meta 限制）
    └── 懒加载 + 缓存（只加载一次）

Native 扫码（iOS/Android）
├── 相机：expo-camera CameraView（MLKit / Vision 框架）
└── 图片：expo-image-picker → scanFromURLAsync → jsQR fallback
```

## 依赖

| 包 | 用途 | 加载方式 |
|----|------|---------|
| `@undecaf/zbar-wasm` | C zbar 编译的 WASM，主力解码器 | CDN 动态 import（Metro 不兼容直接 import） |
| `jsqr` | 纯 JS 解码器，处理干净截图 | npm，Metro 正常打包 |
| `qr-scanner` | ~~已移除~~，被 zbar 替代 | — |

`public/zbar.wasm` 是 zbar 的 WASM 二进制（备用，当前从 CDN 加载）。

## 优化空间

### 性能 / 发热

1. **降低相机帧率**：当前每 ~500ms 抓一帧。可以在检测到 QR finder pattern 后才加快，平时降到 1s。
2. **Web Worker 解码**：把 zbar 调用放到 Web Worker 中，避免阻塞主线程 UI。需要处理 Worker 和 WASM 的加载。
3. **降低相机分辨率**：当前请求 1920×1080，实际只用 1000px。可以直接请求 1280×720，减少 canvas drawImage 的缩放量。
4. **requestIdleCallback**：在浏览器空闲时才处理帧，避免和 UI 渲染竞争。

### 健壮性

5. **zbar CDN 故障兜底**：如果 CDN 不可达，回退到 jsQR 多策略。当前 CDN 失败 = 解码不可用。可以在 `public/` 目录也放一份 zbar JS 作为本地 fallback。
6. **预加载 zbar**：在 app 启动时（非打开扫码时）预加载 zbar WASM，减少首次扫码的延迟。
7. **离线缓存**：用 Service Worker 缓存 zbar WASM 文件，离线也能用。
8. **相机自动对焦检测**：在 zbar 解码前检查图像清晰度（拉普拉斯方差），模糊帧直接跳过，节省 CPU。

### 用户体验

9. **扫码反馈动画**：扫码成功时绿色闪烁 + 震动，失败时不要打断用户。
10. **iPhone 引导**：检测到 iOS + 相机扫码失败多次 → 提示"截图上传更可靠"。
11. **粘贴 URI 快捷入口**：Connect 页面已有粘贴输入框，可以在扫码界面也加一个"粘贴"按钮。
12. **Native app 推荐**：在 iPhone Web 上提示下载 native app 以获得最佳扫码体验。

## 调试工具

`public/qr-test.html` 是独立的诊断页面，可在任何浏览器打开：
- 测试 zbar WASM / jsQR / BarcodeDetector
- 显示 UA、HTTPS 状态、解码器可用性
- 逐策略显示绿色/红色结果
- 相机帧保存为 PNG 下载

访问：`https://<host>/qr-test.html`
