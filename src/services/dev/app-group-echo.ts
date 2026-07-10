/**
 * Increment 2 — App Group echo (app side). DEV-ONLY, no UI.
 *
 * Proves FACT-2 / R4: the app process and the Safari extension process share the
 * App Group container (`group.app.getvela.wallet`) via immutable files + Darwin
 * notifications. Pairs with targets/safari/SafariWebExtensionHandler.swift.
 *
 * On each app foreground/boot it:
 *   Direction B (app writes -> ext reads): writes echo-from-app-<id>.json, which
 *     the extension handler reads back and returns in its native response.
 *   Direction A (ext writes -> app reads): reads the newest echo-from-ext-*.json
 *     the extension wrote and logs it — app-read of an extension-written file.
 * It also observes the ext's Darwin poke and logs it (proves Darwin cross-process).
 *
 * Everything is logged with timestamps; on iOS these console logs surface in the
 * Xcode console for the app process. Wired from src/app/_layout.tsx's __DEV__ block.
 */

import { AppState } from 'react-native';
import * as AppGroup from '@/modules/app-group';

const TAG = '[app-group-echo]';
// Must match kDarwinExtWrote in SafariWebExtensionHandler.swift.
const DARWIN_EXT_WROTE = 'app.getvela.wallet.ext-wrote';

let darwinUnsub: (() => void) | null = null;
let appStateSub: { remove: () => void } | null = null;

function ts(): string {
  return new Date().toISOString();
}

function uuid(): string {
  // Cheap unique id — this is a dev probe, not crypto.
  return `${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
}

/** Direction A: read + log the newest echo the EXTENSION wrote. */
async function readNewestFromExt(): Promise<void> {
  try {
    const names = await AppGroup.list();
    const extFiles = names.filter((n) => n.startsWith('echo-from-ext-')).sort();
    const newest = extFiles[extFiles.length - 1];
    if (!newest) {
      console.log(`${TAG} @${ts()} no echo-from-ext-* yet (tap "R1 echo" on a page first)`);
      return;
    }
    const json = await AppGroup.readFile(newest);
    console.log(`${TAG} @${ts()} READ ext file ${newest}:`, json);
  } catch (e) {
    console.log(`${TAG} @${ts()} readNewestFromExt failed`, e);
  }
}

/** Direction B: write an echo the EXTENSION will read back into its response. */
async function writeFromApp(): Promise<void> {
  const id = uuid();
  const name = `echo-from-app-${id}.json`;
  const record = { id, source: 'app', payload: `hello-from-app @${ts()}`, ts: Date.now() };
  try {
    await AppGroup.writeFile(name, JSON.stringify(record));
    console.log(`${TAG} @${ts()} WROTE app file ${name}`);
  } catch (e) {
    console.log(`${TAG} @${ts()} writeFromApp failed`, e);
  }
}

/**
 * Run the app-side half of the echo. Call once on boot and again on every
 * foreground. Safe to call repeatedly; installs the Darwin observer once.
 */
export async function runAppGroupEcho(): Promise<void> {
  if (!AppGroup.isSupportedSync()) {
    console.log(`${TAG} @${ts()} VelaAppGroup unavailable on this platform — skipping`);
    return;
  }

  const supported = await AppGroup.isSupported();
  console.log(`${TAG} @${ts()} container available: ${supported}`);

  // Install the Darwin observer once (proves cross-process notify).
  if (!darwinUnsub) {
    darwinUnsub = AppGroup.onDarwin(DARWIN_EXT_WROTE, () => {
      console.log(`${TAG} @${ts()} DARWIN observed: ${DARWIN_EXT_WROTE} — re-reading ext file`);
      void readNewestFromExt();
    });
  }

  // The boot effect only fires on COLD launch, so re-run on every foreground:
  // after tapping "R1 echo" in Safari, switching back to the app reliably
  // re-reads the ext file (iOS may have suspended the process, so the Darwin
  // observer alone isn't enough). Install once.
  if (!appStateSub) {
    appStateSub = AppState.addEventListener('change', (s) => {
      if (s === 'active') {
        console.log(`${TAG} @${ts()} app foregrounded — re-running echo`);
        void writeFromApp();
        void readNewestFromExt();
      }
    });
  }

  await writeFromApp();       // Direction B
  await readNewestFromExt();  // Direction A
}
