/**
 * Builds a single SVG path (in module units) covering every dark module of a
 * QR matrix. Consecutive dark modules in a row are merged into one h-run so
 * the path stays small. Rendering the whole code as one path avoids the
 * per-cell pixel-grid rounding that produces hairline white gridlines when
 * each module is its own view.
 */
export function buildQrPath(data: ArrayLike<number>, moduleCount: number): string {
  let d = '';
  for (let y = 0; y < moduleCount; y++) {
    for (let x = 0; x < moduleCount; x++) {
      if (data[y * moduleCount + x] !== 1) continue;
      let run = 1;
      while (x + run < moduleCount && data[y * moduleCount + x + run] === 1) run++;
      d += `M${x} ${y}h${run}v1h-${run}z`;
      x += run - 1;
    }
  }
  return d;
}
