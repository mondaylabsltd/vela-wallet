// Hands a Safari-extension sign rid from the inbound /sign deep-link route (a
// trampoline) to the always-mounted root <ExtensionSignController>. This lets the
// WHOLE sign flow (the SigningRequestModal sheet + the result confirmation) render
// as OVERLAYS over the wallet home — never a standalone navigated page.
//
// Single-listener singleton: exactly one controller is mounted at the root.
type Listener = (rid: string) => void;

let listener: Listener | null = null;
// A rid that arrived BEFORE the controller subscribed (cold start: the deep link /
// UL fires requestExtensionSign while the root controller is still mounting). Buffer
// it and deliver on subscribe, so the sign is never silently dropped → app-on-home.
let pending: string | null = null;

export function onExtensionSign(fn: Listener): () => void {
  listener = fn;
  if (pending !== null) {
    const rid = pending;
    pending = null;
    fn(rid);
  }
  return () => {
    if (listener === fn) listener = null;
  };
}

export function requestExtensionSign(rid: string): void {
  if (listener) listener(rid);
  else pending = rid; // deliver when the controller subscribes
}
