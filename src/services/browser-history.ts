// Recently-opened dApps for the in-app browser.
//
// One entry PER ORIGIN (not per page) so the list reads as "dApps I've used", not a
// raw page log — revisiting a different path of the same site updates the single
// entry in place (latest url/title/favicon/time). Newest-first, capped, persisted in
// AsyncStorage. Nothing sensitive is stored: just the public site url/title/favicon.
import AsyncStorage from '@react-native-async-storage/async-storage';

const KEY = 'vela.browserHistory';
const CAP = 40;

export interface BrowserHistoryEntry {
  /** Origin (`scheme://host`) — the dedupe key. */
  origin: string;
  /** The full last-visited URL (revisiting reopens where the user left off). */
  url: string;
  host: string;
  title: string;
  /** Favicon URL ('' if none resolved). */
  favicon: string;
  /** epoch ms of the last visit. */
  lastVisited: number;
}

function originOf(url: string): string {
  try {
    const u = new URL(url);
    return `${u.protocol}//${u.host}`;
  } catch {
    return '';
  }
}
function hostOf(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

async function read(): Promise<BrowserHistoryEntry[]> {
  try {
    const raw = await AsyncStorage.getItem(KEY);
    const arr = raw ? (JSON.parse(raw) as BrowserHistoryEntry[]) : [];
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

async function write(entries: BrowserHistoryEntry[]): Promise<void> {
  try {
    await AsyncStorage.setItem(KEY, JSON.stringify(entries.slice(0, CAP)));
  } catch {
    /* best-effort */
  }
}

/** Newest-first list of visited dApps. */
export async function getBrowserHistory(): Promise<BrowserHistoryEntry[]> {
  const entries = await read();
  return entries.sort((a, b) => b.lastVisited - a.lastVisited);
}

/**
 * Record (or refresh) a visit. Deduped by origin: an existing entry is updated with
 * the latest url/title/favicon and bumped to now; a new origin is prepended.
 * `now` is passed in (the module can't call Date.now() deterministically in tests).
 */
export async function recordBrowserVisit(
  visit: { url: string; title?: string; favicon?: string },
  now: number,
): Promise<void> {
  const origin = originOf(visit.url);
  if (!origin) return; // only real web origins
  const entries = await read();
  const prev = entries.find((e) => e.origin === origin);
  const next: BrowserHistoryEntry = {
    origin,
    url: visit.url,
    host: hostOf(visit.url),
    // Keep a previously-captured title/favicon if this update lacks one (favicon
    // often resolves a beat after the title).
    title: visit.title || prev?.title || hostOf(visit.url),
    favicon: visit.favicon || prev?.favicon || '',
    lastVisited: now,
  };
  const rest = entries.filter((e) => e.origin !== origin);
  await write([next, ...rest]);
}

/** Remove a single origin's entry. */
export async function deleteBrowserHistory(origin: string): Promise<void> {
  const entries = await read();
  await write(entries.filter((e) => e.origin !== origin));
}

/** Clear the whole history. */
export async function clearBrowserHistory(): Promise<void> {
  try {
    await AsyncStorage.removeItem(KEY);
  } catch {
    /* best-effort */
  }
}
