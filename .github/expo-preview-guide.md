<!-- vela-preview-guide -->
## 📱 Tester guide — how to try this PR on your phone

**Everyday flow (JS-only PRs — the vast majority):**
1. Open the **Vela Wallet dev build** already installed on your phone
2. Scan the **Update QR** in the bot comment above (iPhone → iOS column, Android → Android column; on iOS the system camera works, or use the app's Scan QR code button)
3. The app loads this PR's code directly — no Expo login needed

**One-time setup (once per phone):**
1. Create an Expo account and ask the admin to add you to the **monday-labs-ltd** org (needed to open the Build links). **iPhone only:** your device must be registered first — the admin runs "eas device:create" and sends you the registration link
2. Open the **Build Permalink** from the bot comment in your phone's browser → Install the dev app

**When do I need to reinstall the app?**
- Only when the **Runtime version** in the bot comment differs from the one you last installed (meaning this PR changed native code). Otherwise, just scan — never reinstall.

**Common errors:**
- `Expected MIME-Type … got 'text/html'` → the scanned link resolved to a web page instead of a JS bundle. Make sure you scanned the **Update QR** from the newest bot comment (not an old one, and not a Build link). If it still happens, your installed dev build's **Runtime version** differs from this update — reinstall from the Build Permalink, then scan again.
- Nothing happens when scanning → don't use WeChat or other scanner apps; use the iOS system camera or the in-app scanner.
