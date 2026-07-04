import QRCodeLib from 'qrcode';

import { buildQrPath } from '@/components/qr-path';

describe('buildQrPath', () => {
  it('covers every dark module exactly once and no light modules', () => {
    const code = QRCodeLib.create('https://getvela.app/pay/0xD40086b0Fc99C0BfEc7d3ECba3Bd2Cd2Cdde130b', {
      errorCorrectionLevel: 'M',
    });
    const { data, size } = code.modules;
    const path = buildQrPath(data, size);

    // Replay the path onto a grid and compare with the source matrix.
    const filled = new Set<string>();
    const runRe = /M(\d+) (\d+)h(\d+)v1h-\3z/g;
    let m: RegExpExecArray | null;
    let consumed = 0;
    while ((m = runRe.exec(path)) !== null) {
      const [, xs, ys, runs] = m;
      const x = Number(xs);
      const y = Number(ys);
      const run = Number(runs);
      consumed += m[0].length;
      for (let i = 0; i < run; i++) {
        const key = `${x + i},${y}`;
        expect(filled.has(key)).toBe(false);
        filled.add(key);
      }
    }
    expect(consumed).toBe(path.length); // nothing unparsed in the path

    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        expect(filled.has(`${x},${y}`)).toBe(data[y * size + x] === 1);
      }
    }
  });

  it('merges consecutive dark modules into a single run', () => {
    // 2x2 matrix: top row dark, bottom row light/dark
    const data = [1, 1, 0, 1];
    expect(buildQrPath(data, 2)).toBe('M0 0h2v1h-2zM1 1h1v1h-1z');
  });

  it('returns an empty path for an all-light matrix', () => {
    expect(buildQrPath([0, 0, 0, 0], 2)).toBe('');
  });
});
