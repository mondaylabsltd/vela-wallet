/**
 * Lightweight bar chart for balance history.
 * No third-party dependencies — pure RN Views.
 */

import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { formatBalance } from '@/models/types';
import type { BalancePoint } from '@/services/balance-history';
import React, { useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';

interface Props {
  data: BalancePoint[];
  symbol: string;
}

export function BarChart({ data, symbol }: Props) {
  const [selected, setSelected] = useState<number | null>(null);
  const { t } = useTranslation();

  if (data.length === 0) return null;

  const validBalances = data.filter(d => d.balance >= 0).map(d => d.balance);
  const maxBalance = Math.max(...validBalances, 0.0001);
  const selectedPoint = selected !== null ? data[selected] : null;

  return (
    <View style={styles.container}>
      {/* Selected day balance tooltip */}
      <View style={styles.tooltip}>
        {selectedPoint ? (
          selectedPoint.balance >= 0 ? (
            <>
              <Text style={styles.tooltipBalance}>{formatBalance(selectedPoint.balance)} {symbol}</Text>
              <Text style={styles.tooltipDate}>{selectedPoint.label}</Text>
            </>
          ) : (
            <Text style={styles.tooltipHint}>{t('componentsUi.chart.noDataFor', { label: selectedPoint.label })}</Text>
          )
        ) : (
          <Text style={styles.tooltipHint}>{t('componentsUi.chart.tapHint')}</Text>
        )}
      </View>

      <View style={styles.barsRow}>
        {data.map((point, i) => {
          const noData = point.balance < 0;
          const heightPct = noData ? 0 : Math.max((point.balance / maxBalance) * 100, 2);
          const isSelected = selected === i;
          return (
            <Pressable key={point.label} style={styles.barCol} onPress={() => setSelected(isSelected ? null : i)}>
              <View style={styles.barWrap}>
                {noData ? (
                  <View style={styles.barNoData} />
                ) : (
                <View
                  style={[
                    styles.bar,
                    { height: `${heightPct}%` },
                    isSelected && styles.barSelected,
                  ]}
                />
                )}
              </View>
              <Text style={[styles.label, isSelected && styles.labelSelected]}>{point.label}</Text>
            </Pressable>
          );
        })}
      </View>
    </View>
  );
}

const BAR_HEIGHT = 100;

const styles = createStyles(() => ({
  container: {
    marginTop: space.lg,
  },
  tooltip: {
    alignItems: 'center',
    marginBottom: space['3xl'],
    minHeight: 36,
    justifyContent: 'center',
  },
  tooltipBalance: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  tooltipDate: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  tooltipHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  barsRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    height: BAR_HEIGHT,
    gap: space.sm,
  },
  barCol: {
    flex: 1,
    alignItems: 'center',
  },
  barWrap: {
    width: '100%',
    height: BAR_HEIGHT,
    justifyContent: 'flex-end',
    alignItems: 'center',
  },
  bar: {
    width: '60%',
    minHeight: 2,
    backgroundColor: color.border.base,
    borderRadius: radius.sm,
  },
  barSelected: {
    backgroundColor: color.fg.base,
  },
  barNoData: {
    width: '60%',
    height: 3,
    backgroundColor: color.border.base,
    borderRadius: radius.sm,
  },
  label: {
    fontSize: 9,
    ...inter.medium,
    color: color.fg.subtle,
    marginTop: space.lg,
  },
  labelSelected: {
    color: color.fg.base,
    ...inter.bold,
  },
}));
