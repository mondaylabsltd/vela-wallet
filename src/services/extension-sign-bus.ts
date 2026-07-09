// Hands a Safari-extension sign rid from the inbound /sign deep-link route (a
// trampoline) to the always-mounted root <ExtensionSignController>. This lets the
// WHOLE sign flow (the SigningRequestModal sheet + the result confirmation) render
// as OVERLAYS over the wallet home — never a standalone navigated page.
//
// Single-listener singleton: exactly one controller is mounted at the root.
type Listener = (rid: string) => void;

let listener: Listener | null = null;

export function onExtensionSign(fn: Listener): () => void {
  listener = fn;
  return () => {
    if (listener === fn) listener = null;
  };
}

export function requestExtensionSign(rid: string): void {
  listener?.(rid);
}
