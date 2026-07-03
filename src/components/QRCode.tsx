import QRCodeLib from 'qrcode';
import React, { useMemo } from 'react';
import Svg, { Path, Rect } from 'react-native-svg';

import { buildQrPath } from './qr-path';

interface Props {
  value: string;
  size?: number;
  color?: string;
  backgroundColor?: string;
}

/**
 * Renders the whole matrix as a single SVG path. Drawing each module as its
 * own View lets React Native round every cell to the pixel grid
 * independently, which shows up as hairline white gridlines on most devices
 * whenever size / moduleCount is fractional. One vector path has no per-cell
 * layout, so it stays seamless at any scale.
 */
export function QRCode({ value, size = 200, color = '#000000', backgroundColor = '#FFFFFF' }: Props) {
  const { path, moduleCount } = useMemo(() => {
    try {
      const code = QRCodeLib.create(value, { errorCorrectionLevel: 'M' });
      const modules = code.modules;
      return { path: buildQrPath(modules.data, modules.size), moduleCount: modules.size };
    } catch {
      return { path: 'M0 0h1v1h-1z', moduleCount: 1 };
    }
  }, [value]);

  return (
    <Svg width={size} height={size} viewBox={`0 0 ${moduleCount} ${moduleCount}`}>
      <Rect x={0} y={0} width={moduleCount} height={moduleCount} fill={backgroundColor} />
      <Path d={path} fill={color} />
    </Svg>
  );
}
