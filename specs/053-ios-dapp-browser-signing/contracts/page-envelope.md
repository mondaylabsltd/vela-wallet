# The page ↔ shell channel (053)

**Unchanged from the extension, the desktop and Android.** That is the whole
point: four clients, one provider script, one envelope. A second envelope would
be a second set of answers to questions about somebody's money.

## The page's side

`extension/inpage.js` posts on `window`:

```js
window.postMessage({ ch: 'vela-1193', dir: 'req', id, method, params: params ?? [] },
                   window.location.origin)
```

`id` is `SESSION_UUID + ':' + seq`, single-use: the page's own listener keeps a
`pending` map and **drops an answer whose id it does not hold**. A second answer
to one id therefore reaches nobody — which is why the shell must answer exactly
once and must never re-answer "just in case".

## The bridge script

The desktop's `BRIDGE_JS` with one substitution. Android replaced
`window.ipc.postMessage` with `VelaHost.post`; iOS replaces it with
`window.webkit.messageHandlers.VelaHost.postMessage`. Everything else is
verbatim, **including the top-frame guard**:

```js
(() => {
  if (window.top !== window) return;
  const CHANNEL = 'vela-1193';
  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
    // The host adds the origin. Anything this envelope claims about who it
    // is would be the page describing itself.
    window.webkit.messageHandlers.VelaHost.postMessage(
      JSON.stringify({ id: d.id, method: d.method, params: d.params }));
  });
  window.__velaDeliver = (json) => {
    const m = JSON.parse(json);
    window.postMessage({ ch: CHANNEL, ...m }, window.location.origin);
  };
})();
```

`window.top !== window` is the **entire** `is_main_frame` proof, on every
client. `forMainFrameOnly:` is belt as well as braces on iOS, but the guard is
what the core's invariant rests on, so it stays in the script rather than being
replaced by a `WKUserScript` flag.

## The shell's side

Delivery is `evaluateJavaScript("window.__velaDeliver(<json string literal>)")`
in the `.page` content world, on the main thread.

- answer — `{ dir: "res", id, result }`
- refusal — `{ dir: "res", id, error: { code, message } }`
- event — `{ dir: "evt", event: "accountsChanged" | "chainChanged" | "disconnect", data }`

The page also understands an `evt` named **`connect`**, which no shell emits:
`inpage.js` synthesises it itself the first time `applyChain` learns a chain.
Nothing to send, and nothing to add.

## What the shell attaches, and what it refuses to read

Before dispatching `ProviderRequest` the shell attaches:

| field | source | never |
|---|---|---|
| `origin` | `dappOriginOf(webView.url)`, read **on the main thread at the moment the message arrives** | anything in the envelope |
| `is_main_frame` | `true` by construction — the guard above | a page-supplied flag |
| the owning tab | the handler's own web view | the "current" tab |

**Why the URL is read at message time and not from a navigation callback**:
a document-start script asks before the navigation delegate has reported where
it is. Android found this on the device — the first cut attached an empty
origin to the provider's own warm-up requests. `WKWebView.url` has the same
race: it is set before `didStartProvisionalNavigation` returns.

## Bounds, before anything else sees it

| | |
|---|---|
| `params` JSON | > 256 KiB → dropped silently |
| blank `id` or blank `method` | dropped |
| non-main-frame | cannot arrive (the guard) |

These four checks are the **only** shell-side validation. Everything after them
is the core's judgement.

## The two delivery rules in one class

- **`respond`** goes to the tab that **asked** — the shell keeps request id →
  tab id.
- **`emit`** goes to the **current** tab only. A page that is not in front of
  the person is not told about an account switch it never asked about.

They differ on purpose, and the difference is easy to lose in a refactor.
