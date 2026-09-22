# Contract — erase this device (FR-017)

Erase stays shell-owned (each platform's stores differ), but every shell implements the same four steps:

1. **Confirm** — an explicit confirmation that names what is removed and states that passkeys held by the user's passkey provider are not.
2. **Sweep** — every store listed for that shell in [data-model.md](../data-model.md) §5, minus the keep-list (`vela.pendingUploads` everywhere; Android also keeps accounts and active index until sign-out completes).
3. **Verify** — re-read; anything left outside the keep-list is an error the user sees (the web module's `EraseIncompleteError` is the reference).
4. **End the session** — sign-out **and** its confirmation, so the app restarts as new rather than in a half-signed-in state.

Controls that exist but do nothing are part of this contract: desktop's danger card and the web wide-layout card must both be wired.
