# The page ↔ shell channel (044)

Unchanged from the extension and the desktop: `inpage.js` posts
`{ ch: "vela-1193", dir: "req", id, method, params }` on `window`; the
bridge script (the desktop's `BRIDGE_JS` with `VelaHost.post(json)` in
place of `window.ipc.postMessage`) forwards `{ id, method, params }` — and
nothing the page claims about itself — to Kotlin, only from the top frame.

Kotlin delivers with `evaluateJavascript("window.__velaDeliver(<json>)")`:

- answers `{ dir: "res", id, result }` / `{ dir: "res", id, error: { code, message } }`
- events `{ dir: "evt", event: "accountsChanged" | "chainChanged" | "disconnect", data }`

The bridge also reports `{ vela: "meta", title, favicon }` at
`DOMContentLoaded` and `load`; Android additionally has the WebView's own
title/favicon callbacks and prefers them (the desktop bridge exists
because wry has none).

The shell attaches `origin = origin_of(webView.url)` (the core's rule) and
`tab_id` before dispatching `ProviderRequest`.
