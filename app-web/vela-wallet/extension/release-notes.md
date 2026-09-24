## Installing

This is the browser extension as a `.zip`, for Chrome, Edge, Brave and other Chromium browsers. Until it is listed on the Chrome Web Store it is installed by hand:

1. Unzip `vela-wallet-extension-<version>.zip` into a folder you will keep — the browser loads the extension *from* that folder, so deleting it removes the extension.
2. Open `chrome://extensions` (`edge://extensions`, `brave://extensions`).
3. Turn on **Developer mode** (top right).
4. Click **Load unpacked** and choose the unzipped folder.

The browser may remind you at startup that a developer-mode extension is running; that is expected for an extension installed this way. To update, unzip the new version over the same folder and press the reload arrow on the extension's card.

Your wallet is not in the extension: it is the passkey you sign in with. The extension signs in to the same wallet as the web, desktop and phone apps.

The `SHA256SUMS` file on this release lists the checksum of the zip along with every other package here. The zip also carries build provenance — the run that built it signed for it, and you can check where it came from:

```
gh attestation verify vela-wallet-extension-<version>.zip --repo mondaylabsltd/vela-wallet
```

---
