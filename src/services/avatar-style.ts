/**
 * Avatar-style preference — how account/contact avatars render app-wide:
 * a colored initial (default) or a Nimiq identicon derived from the address.
 *
 * Same module-cache + AsyncStorage pattern as the display currency, plus a
 * listener set so `useAvatarStyle` (useSyncExternalStore) re-renders every
 * mounted avatar the instant the preference changes — no focus round-trip.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';

export type AvatarStyle = 'initials' | 'identicon';

const KEY = 'vela.avatarStyle';

let _style: AvatarStyle = 'initials';
// Bumped on every set; an in-flight load only adopts the stored value if no
// set happened while its read was pending, so a late read can never clobber
// a fresher user choice.
let _version = 0;
const listeners = new Set<() => void>();

function notify(): void {
  for (const l of listeners) l();
}

export function getAvatarStyle(): AvatarStyle {
  return _style;
}

export async function loadAvatarStyle(): Promise<AvatarStyle> {
  const startVersion = _version;
  try {
    const v = await AsyncStorage.getItem(KEY);
    if (_version === startVersion && (v === 'initials' || v === 'identicon') && v !== _style) {
      _style = v;
      notify();
    }
  } catch { /* keep default */ }
  return _style;
}

export async function setAvatarStyle(style: AvatarStyle): Promise<void> {
  _version++;
  if (style !== _style) {
    _style = style;
    notify();
  }
  try { await AsyncStorage.setItem(KEY, style); } catch { /* best effort */ }
}

export function subscribeAvatarStyle(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
