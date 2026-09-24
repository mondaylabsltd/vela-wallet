# Contract — the Clear Signer in every shell

Everything a shell may decide is listed here; everything else is the core's.

## 1. Core surface

UniFFI (Kotlin `uniffi.vela_core_uniffi`, Swift `VelaCore`):

| Name | What |
|---|---|
| `clearSignerDefaultUrl()` | `https://sign.getvela.app/` |
| `clearSignerRequest(input: ClearSignerInput, draft: UserOpDraft?) -> String` | the page's `{intent, context}` JSON |
| `ClearSignerConnection(signerUrl, token, id, requestJson, digest, keys)` | one accepted TCP connection; `feed(bytes) -> ClearSignerStep{write, close, outcome?}`, `closed() -> ClearSignerRefusal?` |
| `clearSignerWsLaunch(base, port: u16, token) -> String` | `…/sign.html?ch=ws#p=<port>&t=<token>` |
| `clearSignerVerify(resultJson, digest, keys) -> ClearSignerOutcome` | for channels that carry the answer as JSON (desktop, web) |
| `SignPrefCore` (bridge object) | the preferences machine, `sign_pref` |

`ClearSignerInput { method, paramsJson, origin, chainId, chainName?, nativeSymbol?, account, accountName?, credentialIdsHex, calls }`:

- a **dApp request**: its own `method` + `paramsJson`, the site's `origin`;
  `calls` = the calls the operation carries before its fee leg (empty for a
  message).
- the **wallet's own send**: `method = ""`, `origin = ""`, `calls` = the send's
  calls (`UserOpCall`, value in base units, as `userOpWithCalls` takes them).
  The core builds `wallet_sendCalls` from them.
- `draft` = the ASSEMBLED operation (after `userOpWithCalls` with the settled
  fee) for a transaction; `null` for a message.

`ClearSignerOutcome = Accepted{credentialIdHex, assertion: WebAuthnAssertion} | Refused{refusal}`.
An `Accepted` assertion goes to `userOpSign(draft, assertion, credentialIdHex, keys)`
or `eip1271Signature(assertion, credentialIdHex, keys)` exactly as a passkey's does.

Desktop (Rust) uses `vela_core::clear_signer` directly: `request`, `url_launch`,
`callback_query`, `parse_callback`, `verify`, `CALLBACK_PATH`.
Web uses wasm: `clearSignerRequest(inputJson)` (same fields, camelCase, `calls`
as `[{to, value, data}]`), `clearSignerVerify(resultJson, digest, keysJson)`
(`{accepted:{credentialIdHex, signatureDer, authenticatorData, clientDataJSON}}`
or `{refused:{code, detail}}`), `clearSignerUrl`, `clearSignerUsesWalletPasskeys`,
and `SignPrefCore`.

## 2. The phone flow (Android, iOS)

```
sign step reached with method == "clear_signer"
  request = clearSignerRequest(input, draft)
  listener = TCP 127.0.0.1:0            (one per ceremony)
  token = 16 random bytes → base64url;  id = random
  url = clearSignerWsLaunch(signPref.signerUrl, listener.port, token)
  open url in the in-app browser tab (Custom Tab / SFSafariViewController)
  loop: accept → conn = ClearSignerConnection(signerUrl, token, id, request, digest, keys)
        on bytes: step = conn.feed(bytes); write step.write; if step.close close socket
                  if step.outcome → done
        on socket end: if conn.closed() → done(Refused(declined))
  done: close the listener and every socket; bring the app back over the tab
        (Android: re-launch the activity with CLEAR_TOP|SINGLE_TOP; iOS: dismiss)
  timeout 5 min → done(timeout)
```

Cancel on the waiting sheet = done(declined). "Open the page again" re-opens
the same URL (same port, same token) — the core accepts a new connection that
proves itself while none has an outcome.

## 3. The desktop flow

`url_launch(signerUrl, request, "http://127.0.0.1:<port>" + CALLBACK_PATH, token)`
opened in the default browser. The listener reads each request head;
`callback_query(head)` → `parse_callback(query, token)` → `verify(…)`. It
replies `200 text/html` with `componentsUi.signing.clearSignerDoneTab` (or the
closed/refused sentence) as the page's body, and `404` to anything else
(`/favicon.ico`). Timeout 5 min.

## 4. The web flow

`window.open(signerUrl + 'sign.html?ch=post')`; wait for `{vela:'ready'}` from
`event.origin === origin(signerUrl)`; post `{vela:'intent', id, intent, context}`
with that `targetOrigin`; accept `{vela:'result'|'error', id}` only from that
origin and `event.source === popup`; `clearSignerVerify`. A popup closed without
an answer (poll `closed`) = declined. Timeout 5 min.

## 5. Words (i18n)

| State | Key |
|---|---|
| Picker option / its line | `componentsUi.signing.clearSignerTitle` / `clearSignerBody` |
| Waiting sheet | `clearSignerWaiting` + `clearSignerWaitingHint`, buttons `clearSignerReopen`, `common.cancel` |
| Declined / closed | `clearSignerClosed` |
| `PageRefused` | `clearSignerRefused` |
| `WrongChallenge`, `ForeignKey`, `BadSignature`, `NotVerified`, `WrongToken`, `Malformed` | `clearSignerMismatch` |
| Timeout | `clearSignerTimeout` |
| Desktop's browser tab after signing | `clearSignerDoneTab` |
| Other picker titles | `common.automatic`, `onboarding.create.methodPlatformTitle`, `methodHybridTitle`, `methodSecurityKeyTitle` |

Declined behaves like a cancelled passkey sheet (the request stays open, the
person can sign another way); every other refusal is shown and the request
stays open as well — nothing was signed, nothing submitted.

## 6. Settings (every shell, next to "Transaction speed")

| Row | Key | Behaviour |
|---|---|---|
| Sign with | `settings.signing.title`, sheet subtitle `settings.signing.subtitle` | options = `SignPrefView.offered` with the picker titles above; `MethodChosen` |
| Clear Signer page | `settings.signing.pageTitle`, value = host of `signerUrl` or `settings.signing.pageOfficial` | sheet: `pageSubtitle`, a text field, `pageSave` (`SignerUrlSubmitted`), `pageReset` (`SignerUrlReset`, shown when not default); under the field `pageInvalid` / `pageInsecure` from `signerUrlError`; `pageForeign` whenever `!signerUsesWalletPasskeys` |

Signing sheets start each request at `SignPrefView.method`; the per-request
picker never dispatches to `sign_pref`.
