/**
 * Source-level regression guards for the native / config cross-platform fixes.
 *
 * These fixes live in Kotlin, Swift, TSX render output, and app.json — none of
 * which this node/ts-jest runner can execute or render. Asserting on the source
 * is a deliberate, cheap net that fails loudly if a fix is reverted (e.g. the
 * WebAuthn JSON goes back to unescaped string interpolation, or the QR Modal
 * loses its Android back-button handler). Behavioral coverage for the JS Hermes
 * shims lives in polyfills.test.ts.
 */
import { readFileSync } from 'fs';
import { resolve } from 'path';

const ROOT = resolve(__dirname, '../..');
const read = (p: string) => readFileSync(resolve(ROOT, p), 'utf8');

describe('Hermes polyfill wiring (P0#1)', () => {
  it('_layout imports @/polyfills before anything else', () => {
    const src = read('src/app/_layout.tsx');
    const firstImport = src.split('\n').find((l) => l.trim().startsWith('import'));
    expect(firstImport).toContain('@/polyfills');
  });

  it('polyfills.web.ts is a no-op so native-only deps stay out of the web bundle', () => {
    const web = read('src/polyfills.web.ts');
    expect(web).not.toContain("import 'react-native-get-random-values'");
  });

  it('the three runtime polyfill deps are declared', () => {
    const pkg = JSON.parse(read('package.json'));
    expect(pkg.dependencies['react-native-get-random-values']).toBeTruthy();
    expect(pkg.dependencies['base-64']).toBeTruthy();
    expect(pkg.dependencies['buffer']).toBeTruthy();
  });
});

describe('Android passkey register() JSON injection fix (P1#3)', () => {
  const kt = read(
    'modules/vela-passkey/android/src/main/java/com/velawallet/passkey/VelaPasskeyModule.kt',
  );

  it('builds the WebAuthn request structurally with JSONObject/JSONArray', () => {
    expect(kt).toContain('JSONObject().apply');
    expect(kt).toContain('JSONArray().apply');
  });

  it('no longer interpolates the raw user name into a JSON string literal', () => {
    expect(kt).not.toContain('"name": "$userName"');
    expect(kt).not.toContain('"displayName": "$userName"');
  });

  it('cancels the coroutine scope via invalidate() (New-Arch-safe)', () => {
    expect(kt).toContain('override fun invalidate()');
  });
});

describe('QR scanner Android back-button dismissal (P2#9)', () => {
  it('every QRScanner Modal wires onRequestClose to onClose', () => {
    const src = read('src/components/QRScanner.tsx');
    const modalCount = (src.match(/<Modal\b/g) ?? []).length;
    const handlerCount = (src.match(/onRequestClose=\{onClose\}/g) ?? []).length;
    expect(modalCount).toBeGreaterThan(0);
    expect(handlerCount).toBe(modalCount);
  });
});

describe('Android keyboard avoidance (audit medium)', () => {
  it('ScreenContainer no longer uses the unreliable Android behavior="height"', () => {
    const src = read('src/components/ui/ScreenContainer.tsx');
    expect(src).not.toContain("? 'padding' : 'height'");
    expect(src).toContain("Platform.OS === 'ios' ? 'padding' : undefined");
  });

  it('app.json pins android softwareKeyboardLayoutMode to resize', () => {
    const app = JSON.parse(read('app.json'));
    expect(app.expo.android.softwareKeyboardLayoutMode).toBe('resize');
  });
});

describe('iOS passkey native fixes (audit medium)', () => {
  const swift = read('modules/vela-passkey/ios/VelaPasskeyModule.swift');

  it('presentation anchor targets the foreground-active key window', () => {
    expect(swift).toContain('foregroundActive');
    expect(swift).toContain('isKeyWindow');
  });

  it('maps benign dismissals (.notInteractive) to CANCELLED, not FAILED', () => {
    expect(swift).toContain('case .canceled, .notInteractive:');
  });
});

describe('decision defaults B1/B2 (config plugin)', () => {
  const plugin = read('plugins/with-native-modules.js');

  it('B1: unused vela-cloud-sync module is no longer wired', () => {
    expect(plugin).not.toContain("'vela-cloud-sync'");
    expect(plugin).not.toContain('VelaCloudSyncModule.swift');
    expect(plugin).not.toContain('VelaCloudSyncPackage');
  });

  it('B1: the unbacked iCloud KV entitlement is not emitted', () => {
    expect(plugin).not.toContain("mod.modResults['com.apple.developer.ubiquity-kvstore-identifier']");
  });

  it('B1: passkey associated-domains entitlement is still present', () => {
    expect(plugin).toContain('webcredentials:getvela.app');
  });

  it('B2: android allowBackup is set to false', () => {
    expect(plugin).toContain("app.$['android:allowBackup'] = 'false'");
    expect(plugin).not.toContain("app.$['android:allowBackup'] = 'true'");
  });
});

describe('camera permanent-denial escape (audit low)', () => {
  it('QRScanner offers Open Settings when the permission can no longer be asked', () => {
    const src = read('src/components/QRScanner.tsx');
    expect(src).toContain('Linking.openSettings()');
    expect(src).toContain('canAskAgain');
  });
});

describe('batch/split send carries the recipient name to the address book (issue #81)', () => {
  it('RecipientDraft can carry an optional name', () => {
    const src = read('src/components/send/MultiRecipientEditor.tsx');
    expect(src).toMatch(/interface RecipientDraft[\s\S]*?name\?: string/);
  });

  it('the payroll importer keeps the parsed name when building drafts', () => {
    const src = read('src/components/send/BatchImportSheet.tsx');
    // apply() must forward r.name, not drop it.
    expect(src).toMatch(/makeRecipientId\(\)[\s\S]*?name: r\.name/);
  });

  it('the split-send path sets toName so deriveFromHistory can surface it', () => {
    const src = read('src/screens/wallet/SendScreen.tsx');
    // The split-mode lines map (recipients.map) must set toName from the draft
    // name — mirroring the single/multiSelect branches. (contacts.test.ts already
    // proves a persisted toName becomes the auto-contact's resolvedName.)
    expect(src).toContain('toName: r.name?.trim() || undefined');
  });
});

describe('WalletPair disconnect confirmation (issue #85)', () => {
  it('Home Connections tab gates disconnect behind a confirm, not a bare tap', () => {
    const src = read('src/screens/wallet/HomeScreen.tsx');
    // The disconnect control must route through confirmDisconnect (showAlert),
    // never wire conn.disconnectBridge straight to onDisconnect.
    expect(src).toContain('onDisconnect={confirmDisconnect}');
    expect(src).not.toContain('onDisconnect={conn.disconnectBridge}');
    expect(src).toMatch(/const confirmDisconnect = useCallback[\s\S]*?showAlert\(/);
  });

  it('legacy /connect screen also confirms before disconnecting', () => {
    const src = read('src/screens/connect/ConnectScreen.tsx');
    expect(src).toContain('onPress={confirmDisconnect}');
    expect(src).not.toContain('onPress={disconnectBridge}');
    expect(src).toMatch(/const confirmDisconnect = useCallback[\s\S]*?showAlert\(/);
  });
});

describe('AppModal in-sheet gestures (issue #87 — slide-to-confirm)', () => {
  const src = read('src/components/ui/AppModal.tsx');

  it('imports GestureHandlerRootView', () => {
    expect(src).toContain("import { GestureHandlerRootView } from 'react-native-gesture-handler'");
  });

  it('wraps every native <Modal> root in a GestureHandlerRootView, not a bare View', () => {
    // RN core <Modal> mounts a native root detached from the app-root
    // GestureHandlerRootView, so RNGH gestures inside (the approve slide-to-
    // confirm knob) die unless each native branch nests its own root.
    // There are 3 native branches (iOS pageSheet, Android sheet, fit sheet).
    const opens = src.match(/<GestureHandlerRootView/g) ?? [];
    const closes = src.match(/<\/GestureHandlerRootView>/g) ?? [];
    expect(opens.length).toBe(3);
    expect(closes.length).toBe(3);
    // The old bug: a bare <View> as the native modal root.
    expect(src).not.toContain('<View style={styles.nativeRoot}>');
    expect(src).not.toContain('<View style={styles.fitRoot}>');
  });
});
