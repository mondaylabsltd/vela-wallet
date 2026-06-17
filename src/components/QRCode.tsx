import QRCodeLib from 'qrcode';
import React, { useMemo } from 'react';
import { StyleSheet, View } from 'react-native';

interface Props {
  value: string;
  size?: number;
  color?: string;
  backgroundColor?: string;
}

export function QRCode({ value, size = 200, color = '#000000', backgroundColor = '#FFFFFF' }: Props) {
  const matrix = useMemo(() => {
    try {
      const segments = QRCodeLib.create(value, { errorCorrectionLevel: 'M' });
      const modules = segments.modules;
      const moduleCount = modules.size;
      const data = modules.data;
      const rows: boolean[][] = [];
      for (let y = 0; y < moduleCount; y++) {
        const row: boolean[] = [];
        for (let x = 0; x < moduleCount; x++) {
          row.push(data[y * moduleCount + x] === 1);
        }
        rows.push(row);
      }
      return rows;
    } catch {
      return [[true]];
    }
  }, [value]);

  const moduleSize = size / matrix.length;

  return (
    <View style={[styles.container, { width: size, height: size, backgroundColor }]}>
      {matrix.map((row, y) => (
        <View key={y} style={styles.row}>
          {row.map((cell, x) => (
            <View
              key={x}
              style={{
                width: moduleSize,
                height: moduleSize,
                backgroundColor: cell ? color : backgroundColor,
              }}
            />
          ))}
        </View>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flexDirection: 'column' },
  row: { flexDirection: 'row' },
});
