/**
 * The web renderer puts every inline SVG in one DOM. SVG clipPath ids can then
 * collide when the same account avatar is mounted in Home and a hidden modal,
 * making a newly-created account flash as a square until refresh. Keep the
 * circular crop on the native/web wrapper instead of inside the SVG.
 */
import { readFileSync } from 'fs';
import { resolve } from 'path';

const source = readFileSync(resolve(__dirname, '../../components/ui/Identicon.tsx'), 'utf8');

describe('Identicon shape', () => {
  it('clips with an id-free circular wrapper', () => {
    expect(source).toContain("borderRadius: size / 2, overflow: 'hidden'");
    expect(source).not.toContain('<clipPath');
    expect(source).not.toContain('clip-path=');
  });
});
