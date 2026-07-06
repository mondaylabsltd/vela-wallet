/**
 * App Group module — shared-container IPC between the Vela app process and the
 * Safari Web Extension process (App Group `group.app.getvela.wallet`).
 *
 * - iOS:  VelaAppGroup native module (RCTEventEmitter). Files are written
 *         atomically with NSFileProtectionCompleteUntilFirstUserAuthentication;
 *         Darwin notifications are payload-less cross-process "pokes".
 * - Web / Android: no App Group / Darwin equivalent — every call is a no-op
 *         (write throws; read/list return empty; observe returns a no-op
 *         unsubscribe) so callers can stay platform-agnostic.
 *
 * Mirrors the src/modules/passkey import/guard idiom (`const { VelaAppGroup } =
 * NativeModules`) plus NativeEventEmitter for the onDarwin event stream.
 */

import { NativeModules, NativeEventEmitter, Platform } from 'react-native';

const { VelaAppGroup } = NativeModules;
const isNative = Platform.OS === 'ios' && !!VelaAppGroup;

// One emitter instance for the single "VelaAppGroup_darwin" channel.
const emitter = isNative ? new NativeEventEmitter(VelaAppGroup) : null;

/** True only when the native module + App Group container are reachable. */
export function isSupportedSync(): boolean {
  return isNative;
}

/** Whether the App Group container URL resolves (entitlement provisioned). */
export async function isSupported(): Promise<boolean> {
  if (!isNative) return false;
  try {
    return await VelaAppGroup.isSupported();
  } catch {
    return false;
  }
}

export async function writeFile(name: string, json: string): Promise<void> {
  if (!isNative) throw new Error('VelaAppGroup unavailable on this platform');
  return VelaAppGroup.writeFile(name, json);
}

export async function readFile(name: string): Promise<string | null> {
  if (!isNative) return null;
  return VelaAppGroup.readFile(name);
}

export async function list(dir = ''): Promise<string[]> {
  if (!isNative) return [];
  return VelaAppGroup.list(dir);
}

export async function remove(name: string): Promise<void> {
  if (!isNative) return;
  return VelaAppGroup.remove(name);
}

export async function postDarwin(name: string): Promise<void> {
  if (!isNative) return;
  return VelaAppGroup.postDarwin(name);
}

/**
 * Observe a payload-less Darwin notification. Returns an unsubscribe fn.
 * The native side multiplexes every observed name onto one JS event channel,
 * so we register the name natively and filter by name here.
 */
export function onDarwin(name: string, cb: () => void): () => void {
  if (!isNative || !emitter) return () => {};
  VelaAppGroup.observeDarwin(name);
  const sub = emitter.addListener('VelaAppGroup_darwin', (e: { name: string }) => {
    if (e?.name === name) cb();
  });
  return () => {
    sub.remove();
    VelaAppGroup.unobserveDarwin(name);
  };
}
