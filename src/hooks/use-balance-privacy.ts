/**
 * useBalancePrivacy — the app-wide persisted "hide amounts" flag.
 *
 * One module-level store (useSyncExternalStore) so every surface that shows
 * money — hero, activity feed, holdings, account switcher, receipt toast —
 * masks together; a leak in one surface defeats the mask everywhere else.
 *
 * Persisted because the threat model is handing the phone over: someone who
 * hides before doing so must not get silently reset to visible on relaunch.
 * A user toggle always wins over the async hydrate (no race: hydrate commits
 * only while the store is still untouched).
 */
import { useCallback, useSyncExternalStore } from 'react';
import AsyncStorage from '@react-native-async-storage/async-storage';

const KEY = 'vela.balanceHidden';

let _hidden = false;
let _touched = false; // set by hydrate OR a user toggle — whichever lands first wins
let _hydrateStarted = false;
const subs = new Set<() => void>();

function emit() { subs.forEach((fn) => fn()); }

function hydrate() {
  if (_hydrateStarted) return;
  _hydrateStarted = true;
  AsyncStorage.getItem(KEY).then((v) => {
    if (_touched) return; // a toggle raced the read — the user's tap wins
    _touched = true;
    if (v === '1') { _hidden = true; emit(); }
  }).catch(() => {});
}

export function setBalanceHidden(next: boolean): void {
  _touched = true;
  _hidden = next;
  AsyncStorage.setItem(KEY, next ? '1' : '0').catch(() => {});
  emit();
}

export function useBalancePrivacy(): { hidden: boolean; toggle: () => void } {
  const hidden = useSyncExternalStore(
    (cb) => { subs.add(cb); hydrate(); return () => { subs.delete(cb); }; },
    () => _hidden,
    () => _hidden,
  );
  const toggle = useCallback(() => setBalanceHidden(!_hidden), []);
  return { hidden, toggle };
}
