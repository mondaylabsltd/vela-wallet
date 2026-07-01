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
