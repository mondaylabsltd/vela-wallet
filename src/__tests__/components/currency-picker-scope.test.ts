/**
 * Issue #80 — the display (quote) currency looked settable in two places that
 * didn't stay in sync. There is exactly ONE app-wide display currency (Settings →
 * setCurrency, persisted). The payroll "Priced in" picker in BatchImportSheet is a
 * legitimately separate PER-BATCH override — it must stay local (never call
 * setCurrency) and must be visually distinct so it doesn't read as the global
 * setting. These source-level guards lock both invariants (the components aren't
 * render-testable in this runner).
 */
import { readFileSync } from 'fs';
import { resolve } from 'path';

const read = (p: string) => readFileSync(resolve(__dirname, '../../..', p), 'utf8');

describe('CurrencySheet title override (issue #80)', () => {
  const src = read('src/components/ui/CurrencySheet.tsx');

  it('accepts an optional title prop, defaulting to the global "Display currency"', () => {
    expect(src).toMatch(/title\?:\s*string/);
    expect(src).toContain("title ?? t('componentsUi.currency.title')");
  });
});

describe('BatchImportSheet payroll currency stays a local per-batch override (issue #80)', () => {
  const src = read('src/components/send/BatchImportSheet.tsx');

  it('gives its CurrencySheet a distinct title (not the global display-currency header)', () => {
    expect(src).toMatch(/<CurrencySheet[\s\S]*?title=\{t\('send\.batchCurrencyLabel'/);
  });

  it('never mutates the app-wide display currency (no setCurrency import/call)', () => {
    // The batch picker must only touch local fiatCode; importing setCurrency here
    // would risk a payroll pick silently flipping the whole wallet's currency.
    expect(src).not.toMatch(/\bsetCurrency\b/);
  });
});

describe('the app-wide display currency is set in exactly one place (Settings)', () => {
  it('SettingsScreen persists via setCurrency', () => {
    const src = read('src/screens/settings/SettingsScreen.tsx');
    expect(src).toMatch(/\bsetCurrency\b/);
  });
});
