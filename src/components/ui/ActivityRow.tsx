/**
 * ActivityRow — one entry in the payment-first Activity feed.
 *
 * Neutral avatar + direction arrow (in = success/down-left, out = muted/up-right),
 * a small chain badge, title/subtitle, and amount/time. Incoming rows can play a
 * brief "just arrived" success-glow when `isNew` is set.
 *
 * Theme-driven (light/dark). Spring press + staggered entrance per the design system.
 */
import React, { useEffect } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';
import { ArrowDownLeft, ArrowUpRight } from 'lucide-react-native';
import { ChainLogo } from '@/components/ChainLogo';
import { fadeInDown } from '@/constants/entering';
import type { Network } from '@/models/network';
import { color, createStyles, font, inter, motion, radius, shadow, space, text } from '@/constants/theme';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

export interface ActivityRowProps {
  direction: 'in' | 'out';
  title: string;
  subtitle: string;
  amount: string;
  /** Value in the user's display currency, shown under the amount (e.g. "AR$1,428.20"). */
  fiat?: string;
  time: string;
  chain?: Network | null;
  onPress?: () => void;
  index?: number;
  isNew?: boolean;
}

export function ActivityRow({ direction, title, subtitle, amount, fiat, time, chain, onPress, index = 0, isNew }: ActivityRowProps) {
  const incoming = direction === 'in';
  const scale = useSharedValue(1);
  const pressStyle = useAnimatedStyle(() => ({ transform: [{ scale: scale.value }] }));

  const glow = useSharedValue(isNew ? 1 : 0);
  useEffect(() => {
    if (isNew) {
      glow.value = 1;
      glow.value = withTiming(0, { duration: 1600 });
    }
    // run once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const glowStyle = useAnimatedStyle(() => ({ opacity: glow.value }));

  const Arrow = incoming ? ArrowDownLeft : ArrowUpRight;

  // Split "−0,001193 ETH" → number + ticker. Fitting only the NUMBER to the box
  // (the ticker is subordinated) keeps the figure a consistent size row-to-row;
  // including the ticker made short amounts large and long ones shrink.
  const sp = amount.lastIndexOf(' ');
  const numberPart = sp > 0 ? amount.slice(0, sp) : amount;
  const ticker = sp > 0 ? amount.slice(sp + 1) : '';

  return (
    <Animated.View entering={fadeInDown(index * 40, 300)}>
      <AnimatedPressable
        style={[styles.row, pressStyle]}
        onPress={onPress}
        onPressIn={() => { if (onPress) scale.value = withSpring(0.98, motion.spring); }}
        onPressOut={() => { if (onPress) scale.value = withSpring(1, motion.spring); }}
      >
        <Animated.View pointerEvents="none" style={[styles.glow, glowStyle]} />

        <View style={[styles.avatar, incoming && styles.avatarIn]}>
          <Arrow size={24} color={incoming ? color.success.base : color.fg.muted} strokeWidth={2.4} />
          {chain && (
            <View style={styles.badge}>
              <ChainLogo label={chain.iconLabel} color={chain.iconColor} bgColor={chain.iconBg} logoURL={chain.logoURL} size={18} />
            </View>
          )}
        </View>

        <View style={styles.info}>
          <Text style={styles.title} numberOfLines={1}>{title}</Text>
          <Text style={styles.subtitle} numberOfLines={1}>{subtitle}</Text>
          {time ? <Text style={styles.time} numberOfLines={1}>{time}</Text> : null}
        </View>

        <View style={styles.right}>
          {/* Fixed font size — every figure renders identically; only a rare
              over-long amount shrinks (down to 85%) so it never overflows. */}
          <Text
            style={[styles.amount, incoming && styles.amountIn]}
            numberOfLines={1}
            adjustsFontSizeToFit
            minimumFontScale={0.85}
          >
            {numberPart}
            {ticker ? <Text style={styles.ticker}> {ticker}</Text> : null}
          </Text>
          {fiat ? <Text style={styles.fiat} numberOfLines={1}>{fiat}</Text> : null}
        </View>
      </AnimatedPressable>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  row: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.lg,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingVertical: space.lg,
    paddingHorizontal: space.lg,
    ...shadow.sm,
  },
  glow: {
    position: 'absolute',
    top: 0, left: 0, right: 0, bottom: 0,
    borderRadius: radius.xl,
    borderWidth: 2,
    borderColor: color.success.base,
    backgroundColor: color.success.soft,
  },
  avatar: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  avatarIn: {
    backgroundColor: color.success.soft,
  },
  badge: {
    position: 'absolute',
    right: -2,
    bottom: -2,
    borderRadius: radius.full,
    borderWidth: 2,
    borderColor: color.bg.raised,
  },
  info: {
    flex: 1,
    minWidth: 0,
    gap: 3,
  },
  // Direction label is supporting context now (arrow + amount color/sign carry
  // it) — keep it calm so the amount and counterparty lead the eye.
  title: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
  },
  subtitle: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
  },
  time: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  right: {
    alignItems: 'flex-end',
    gap: 2,
    flexShrink: 0,
    maxWidth: '55%', // content-sized; only a pathologically long amount hits this cap
  },
  amount: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'right',
  },
  amountIn: {
    color: color.success.base,
  },
  ticker: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  fiat: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
}));
