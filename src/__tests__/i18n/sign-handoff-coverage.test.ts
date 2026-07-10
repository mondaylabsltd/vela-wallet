/**
 * Coverage lock for the Safari-extension sign hand-off screen (src/app/sign.tsx).
 *
 * sign.tsx is the native half of a strategic, user-facing flow shown in 15
 * languages. This pins that EVERY locale carries the full `signHandoff` key set —
 * a missing key would silently fall back to English mid-flow (or render the raw
 * key). Fails loudly if a locale or key is dropped when translations are edited.
 */
import { resources } from '@/i18n/resources';

const SIGN_HANDOFF_KEYS = [
  'signing', 'signed', 'rejected', 'pending', 'expired',
  'returnHint', 'oneTapTitle', 'oneTapHint', 'done',
] as const;

// The app's full supported locale set (mirrors src/i18n/locales/*).
const LOCALES = [
  'en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'de', 'fr', 'it', 'ru', 'tr', 'vi', 'id', 'es-MX', 'pt-BR',
];

test('every locale is present in resources', () => {
  for (const loc of LOCALES) {
    expect(resources[loc as keyof typeof resources]).toBeDefined();
  }
});

test('every locale has all signHandoff keys, non-empty', () => {
  const gaps: string[] = [];
  for (const loc of LOCALES) {
    const bundle = (resources as any)[loc]?.translation?.signHandoff;
    if (!bundle) { gaps.push(`${loc}: signHandoff MISSING`); continue; }
    for (const k of SIGN_HANDOFF_KEYS) {
      const v = bundle[k];
      if (typeof v !== 'string' || v.trim() === '') gaps.push(`${loc}.${k}`);
    }
  }
  expect(gaps).toEqual([]);
});

test('non-English locales are actually translated (not English placeholders left behind)', () => {
  // A few keys that MUST differ from English in CJK locales — catches a copy-paste
  // that left English strings in a localized bundle.
  const en = (resources as any).en.translation.signHandoff;
  for (const loc of ['zh', 'zh-TW', 'zh-HK', 'ja', 'ko']) {
    const b = (resources as any)[loc].translation.signHandoff;
    // 'signed' / 'rejected' are short, high-signal, and never share glyphs with EN here.
    expect(b.signed).not.toBe(en.signed);
    expect(b.rejected).not.toBe(en.rejected);
  }
});
