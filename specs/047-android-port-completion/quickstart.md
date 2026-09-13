# Quickstart — verifying 047 on the device
1. 设置 → 语言 → English → the tabs read Wallet/Contacts; → 跟随系统 → back to 中文.
2. 设置 → 数字格式 → dot/comma → the home total reads `1.234,56` style; → auto.
3. 设置 → 存储 → the device's counts; 清除 on 余额缓存 → the item reads 0; 清除所有缓存.
4. 设置 → 账户 → the list; tap another → the home shows it.
5. 收款 → 二维码 → 保存图片 → the share sheet with a PNG.
6. `adb shell am start -a android.intent.action.VIEW -d 'velawallet://pay?to=0x7687…&chain=100&amount=0.001'` → the locked Send.
7. Airplane mode → the offline line; off → gone.
8. `adb shell am start … --ez vela.testPanic true` → relaunch → the failure sheet → Report.
9. 设置 → 擦除设备 → confirm → first run; the store is empty (`run-as … strings`).
