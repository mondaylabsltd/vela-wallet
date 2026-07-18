import React from 'react';
import { View } from 'react-native';
import { MoveDown } from 'lucide-react-native';
import { color, createStyles, space } from '@/constants/theme';

/**
 * The sender → recipient connector on the Send confirm screen. One shared arrow
 * across all three modes (single, split, sweep) so the flow always reads the
 * same: a thin hairline shaft flowing into a light down-arrow — longer and finer
 * than a bare icon, so it reads as a real "money moves this way" connector.
 */
export function FlowArrow() {
  return (
    <View style={styles.connector}>
      <View style={styles.line} />
      <View style={styles.head}>
        <MoveDown size={20} color={color.border.base} strokeWidth={1.5} />
      </View>
    </View>
  );
}

const styles = createStyles(() => ({
  // Width matches the 38px avatar column so the arrow sits centred under it.
  connector: {
    width: 38,
    alignItems: 'center',
    paddingVertical: space.xs,
  },
  line: {
    width: 1.5,
    height: 16,
    borderRadius: 1,
    backgroundColor: color.border.base,
  },
  // Pull the arrowhead up so its own shaft continues the hairline seamlessly.
  head: {
    marginTop: -4,
  },
}));
