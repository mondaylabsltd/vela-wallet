/**
 * Font guard — fonts must come from the theme, never raw string literals.
 *
 * A serif/`fontFamily: 'Times'` slipping into a screen is exactly the regression
 * that produced a serif headline on the signing sheet before. The single source
 * of truth is constants/theme.ts (the `font`/`inter` objects); everywhere else
 * must reference those, so a `fontFamily: '<literal>'` outside theme.ts fails CI.
 */
import { readdirSync, readFileSync, statSync } from 'fs';
import { join } from 'path';

const SRC = join(__dirname, '..');
const THEME = join('constants', 'theme.ts');

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === '__tests__') continue;
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (/\.(ts|tsx)$/.test(entry)) out.push(full);
  }
  return out;
}

describe('font guard', () => {
  const files = walk(SRC).filter((f) => !f.endsWith(THEME));

  it('has no fontFamily string literals outside theme.ts', () => {
    const offenders: string[] = [];
    // Matches `fontFamily: 'x'` / `fontFamily: "x"` but NOT `fontFamily: font.mono`.
    const re = /fontFamily\s*:\s*['"]/;
    for (const f of files) {
      readFileSync(f, 'utf8').split('\n').forEach((line, i) => {
        if (re.test(line)) offenders.push(`${f.replace(SRC, 'src')}:${i + 1}  ${line.trim()}`);
      });
    }
    expect(offenders).toEqual([]);
  });

  it('has no serif/Times/Georgia font literals outside theme.ts', () => {
    const offenders: string[] = [];
    const re = /['"](serif|ui-serif|Times|Times New Roman|Georgia)['"]/;
    for (const f of files) {
      readFileSync(f, 'utf8').split('\n').forEach((line, i) => {
        if (re.test(line)) offenders.push(`${f.replace(SRC, 'src')}:${i + 1}  ${line.trim()}`);
      });
    }
    expect(offenders).toEqual([]);
  });
});
