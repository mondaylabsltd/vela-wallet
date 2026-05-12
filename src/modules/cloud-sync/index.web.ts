/**
 * Cloud Sync — Web implementation using IndexedDB.
 *
 * On native, cloud sync uses iCloud / Google Play Services.
 * On web, we use IndexedDB as a local persistent store.
 * This doesn't sync across devices, but preserves data across
 * browser sessions (unlike localStorage which has size limits).
 */

// ---------------------------------------------------------------------------
// Error model (matches native interface)
// ---------------------------------------------------------------------------

export const CloudSyncErrorCode = {
  NOT_AVAILABLE: 'CLOUD_NOT_AVAILABLE',
  NOT_SIGNED_IN: 'CLOUD_NOT_SIGNED_IN',
  QUOTA_EXCEEDED: 'CLOUD_QUOTA_EXCEEDED',
  NETWORK_ERROR: 'CLOUD_NETWORK_ERROR',
  FAILED: 'CLOUD_FAILED',
} as const;

export type CloudSyncErrorCode =
  (typeof CloudSyncErrorCode)[keyof typeof CloudSyncErrorCode];

export class CloudSyncError extends Error {
  readonly code: CloudSyncErrorCode;
  constructor(code: CloudSyncErrorCode, message: string) {
    super(message);
    this.name = 'CloudSyncError';
    this.code = code;
  }
}

// ---------------------------------------------------------------------------
// Availability
// ---------------------------------------------------------------------------

export const CloudSyncAvailability = {
  AVAILABLE: 'available',
  NOT_SIGNED_IN: 'notSignedIn',
  RESTRICTED: 'restricted',
  NOT_SUPPORTED: 'notSupported',
} as const;

export type CloudSyncAvailability =
  (typeof CloudSyncAvailability)[keyof typeof CloudSyncAvailability];

// ---------------------------------------------------------------------------
// Event model
// ---------------------------------------------------------------------------

export type CloudSyncEvent = 'syncCompleted' | 'syncFailed' | 'dataChanged';

export interface CloudSyncEventData {
  syncCompleted: {};
  syncFailed: { error: string };
  dataChanged: { changedKeys: string[] };
}

// ---------------------------------------------------------------------------
// IndexedDB helpers
// ---------------------------------------------------------------------------

const DB_NAME = 'vela-cloud-sync';
const STORE_NAME = 'kv';
const DB_VERSION = 1;

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(new CloudSyncError(CloudSyncErrorCode.FAILED, 'Failed to open IndexedDB'));
  });
}

function idbGet(db: IDBDatabase, key: string): Promise<string | null> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const req = store.get(key);
    req.onsuccess = () => resolve(req.result ?? null);
    req.onerror = () => reject(new CloudSyncError(CloudSyncErrorCode.FAILED, 'IndexedDB read failed'));
  });
}

function idbPut(db: IDBDatabase, key: string, value: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.put(value, key);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(new CloudSyncError(CloudSyncErrorCode.FAILED, 'IndexedDB write failed'));
  });
}

function idbDelete(db: IDBDatabase, key: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.delete(key);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(new CloudSyncError(CloudSyncErrorCode.FAILED, 'IndexedDB delete failed'));
  });
}

function idbAllKeys(db: IDBDatabase): Promise<string[]> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const req = store.getAllKeys();
    req.onsuccess = () => resolve(req.result.map(String));
    req.onerror = () => reject(new CloudSyncError(CloudSyncErrorCode.FAILED, 'IndexedDB keys failed'));
  });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export async function isSupported(): Promise<boolean> {
  return typeof indexedDB !== 'undefined';
}

export async function getAvailability(): Promise<CloudSyncAvailability> {
  if (typeof indexedDB === 'undefined') return CloudSyncAvailability.NOT_SUPPORTED;
  return CloudSyncAvailability.AVAILABLE;
}

export async function save(key: string, value: unknown): Promise<void> {
  const db = await openDB();
  await idbPut(db, key, JSON.stringify(value));
  db.close();
}

export async function get<T = unknown>(key: string): Promise<T | null> {
  const db = await openDB();
  const json = await idbGet(db, key);
  db.close();
  if (json == null) return null;
  try {
    return JSON.parse(json) as T;
  } catch {
    throw new CloudSyncError(CloudSyncErrorCode.FAILED, `Corrupt data for key "${key}"`);
  }
}

export async function remove(key: string): Promise<void> {
  const db = await openDB();
  await idbDelete(db, key);
  db.close();
}

export async function listKeys(): Promise<string[]> {
  const db = await openDB();
  const keys = await idbAllKeys(db);
  db.close();
  return keys;
}

export async function syncNow(): Promise<void> {
  // No-op on web — IndexedDB is local only
}

export function addListener<E extends CloudSyncEvent>(
  _event: E,
  _handler: (data: CloudSyncEventData[E]) => void,
): () => void {
  // No cross-device events on web
  return () => {};
}
