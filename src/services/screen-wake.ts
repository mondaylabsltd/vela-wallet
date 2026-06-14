/**
 * Keep-screen-awake preference + scoped locks.
 *
 * Two layers:
 *   - A user setting ("Keep screen on") that holds a wake lock while the app is
 *     foregrounded — for people who leave the wallet open as a payment display.
 *   - Imperative scoped locks (holdScreenAwake / releaseScreenAwake) for critical
 *     moments (e.g. signing) regardless of the setting.
 *
 * Uses expo-keep-awake (bundled with expo; web via the Wake Lock API). Every
 * call is wrapped: on web the Wake Lock API can reject without a user gesture or
 * while the tab is hidden — that's fine, expo-keep-awake re-acquires on
 * visibility change, and the user toggle re-applies on the next tap.
 */
import AsyncStorage from '@react-native-async-storage/async-storage';
import { activateKeepAwakeAsync, deactivateKeepAwake } from 'expo-keep-awake';

const STORAGE_KEY = 'vela.keepAwake.enabled';
const USER_TAG = 'vela-user';

let _enabled = false; // default OFF — opt in from Settings

/** Load the persisted preference and apply it. Call once at app start. */
export async function loadKeepAwakePreference(): Promise<void> {
  try {
    const v = await AsyncStorage.getItem(STORAGE_KEY);
    if (v !== null) _enabled = v === '1';
  } catch { /* keep default */ }
  apply();
}

export function isKeepAwakeEnabled(): boolean {
  return _enabled;
}

/** Toggle and persist the user "keep screen on" setting. Returns the new value. */
export async function setKeepAwakeEnabled(enabled: boolean): Promise<boolean> {
  _enabled = enabled;
  try { await AsyncStorage.setItem(STORAGE_KEY, enabled ? '1' : '0'); } catch { /* best effort */ }
  apply();
  return _enabled;
}

function apply(): void {
  if (_enabled) hold(USER_TAG); else release(USER_TAG);
}

/** Hold a scoped wake lock (e.g. during signing). Safe to call repeatedly. */
export function holdScreenAwake(tag: string): void { hold(tag); }
/** Release a scoped wake lock acquired with the same tag. */
export function releaseScreenAwake(tag: string): void { release(tag); }

/** Tolerate both promise-rejection and sync-throw from the native module. */
function settle(p: unknown): void {
  if (p && typeof (p as { catch?: unknown }).catch === 'function') {
    (p as Promise<unknown>).catch(() => {});
  }
}

function hold(tag: string): void {
  try { settle(activateKeepAwakeAsync(tag)); } catch { /* ignore */ }
}

function release(tag: string): void {
  try { settle(deactivateKeepAwake(tag)); } catch { /* ignore */ }
}
