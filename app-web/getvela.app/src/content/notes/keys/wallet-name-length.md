---
title: "Why a wallet name fits nine Chinese characters, but not ten"
nav: "How long a wallet name can be"
description: "A wallet name can be up to 27 bytes. That's 27 English letters, or nine Chinese, Japanese or Korean characters, which take three bytes each."
facts:
  - "Limit | 27 bytes"
  - "English | 27 letters"
  - "Chinese, Japanese, Korean | 9 characters"
  - "Emoji | 6 at most"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/mod.rs#L98-L103
  - rust/crates/vela-core/src/app/mod.rs#L900-L904
  - rust/crates/vela-core/src/app/create_wallet.rs#L784-L795
  - rust/crates/vela-core/src/app/mod.rs#L1084-L1090
docs: create-wallet
related:
  - security-layers/decide-your-keys-first
draft: true
---

Vela stores the wallet name in the passkey's user ID. WebAuthn limits the user ID to 64 bytes, and
Vela uses 37 of them for a separator and a unique ID. That leaves 27 bytes for the name.

The create form counts the name in bytes as you type, and tells you if it's too long before the
passkey prompt opens. Without that check, the prompt would open and then fail with a "User handle
exceeds 64 bytes" error.

Extra keys are named after the wallet and the key's label together, so your password manager shows
which wallet each key belongs to. If the combined name doesn't fit, Vela uses the label on its own.

Names in Chinese, Japanese or Korean get about a third as many characters as English names. Emoji
take four bytes or more each.
