/**
 * ActivityRow — one entry in the payment-first Activity feed.
 *
 * Two columns: a neutral avatar (direction arrow + small chain badge) and a
 * content block of three lines:
 *   1. title ............... amount     (label left, figure right)
 *   2. counterparty ........ fiat       (address/alias left, value right)
 *   3. time
 * Putting the amount on the title's line — not in its own column — frees the
 * full row width for the counterparty address, so it never gets tail-clipped by
 * a long figure. Incoming rows can play a brief "just arrived" glow when `isNew`.
 *
 * Theme-driven (light/dark). Spring press + staggered entrance per the design system.
 */
import React, { useEffect, useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';
import { Swipeable } from 'react-native-gesture-handler';
import { useTranslation } from 'react-i18next';
import { ArrowDownLeft, ArrowUpRight, Trash2 } from 'lucide-react-native';
import { ChainLogo } from '@/components/ChainLogo';
import { fadeInDown } from '@/constants/entering';
import type { Network } from '@/models/network';
import { hapticLight } from '@/services/platform';
import { color, createStyles, font, inter, motion, radius, space, text } from '@/constants/theme';

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
  /** Swipe-left reveals a Delete action that removes this record from the local feed. */
  onDelete?: () => void;
}

export function ActivityRow({ direction, title, subtitle, amount, fiat, time, chain, onPress, index = 0, isNew, onDelete }: ActivityRowProps) {
  const { t } = useTranslation();
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

  // Play the staggered entrance ONCE, on first mount. The Home feed re-renders in
  // a burst when the account switcher opens (it refreshes every account's balance);
  // without this gate each re-render re-applies `entering` and the whole list
  // appears to slide/flicker behind the sheet. Mirrors HomeScreen's header gate.
  const hasEntered = useRef(false);
  useEffect(() => { hasEntered.current = true; }, []);

  const Arrow = incoming ? ArrowDownLeft : ArrowUpRight;

  // Split "−0,001193 ETH" → number + ticker. Fitting only the NUMBER to the box
  // (the ticker is subordinated) keeps the figure a consistent size row-to-row;
  // including the ticker made short amounts large and long ones shrink.
  const sp = amount.lastIndexOf(' ');
  const numberPart = sp > 0 ? amount.slice(0, sp) : amount;
  const ticker = sp > 0 ? amount.slice(sp + 1) : '';

  // One spoken label for the whole row instead of five separate text nodes,
  // e.g. "Sent, 0.05 ETH, to 0x12…ab, ≈$90, 2h ago".
  const a11yLabel = [title, amount, subtitle, fiat, time].filter(Boolean).join(', ');

  const handlePress = onPress
    ? () => { hapticLight(); onPress(); }
    : undefined;

  // Swipe-left reveals a single Delete action that removes this record from the
  // local feed (on-chain history is untouched). Mirrors the Connections per-row
  // swipe-to-delete so the gesture reads the same everywhere in the app.
  const swipeRef = useRef<Swipeable>(null);
  const canSwipe = !!onDelete;

  const handleDelete = () => {
    hapticLight();
    swipeRef.current?.close();
    onDelete?.();
  };

  const renderRightActions = () => (
    <View style={styles.swipeActions}>
      <Pressable
        style={[styles.swipeAction, styles.swipeDelete]}
        onPress={handleDelete}
        accessibilityRole="button"
        accessibilityLabel={t('activity.delete', { defaultValue: 'Delete' })}
      >
        <Trash2 size={17} color={color.fg.inverse} strokeWidth={2.2} />
        <Text style={[styles.swipeActionText, styles.swipeActionTextLight]}>
          {t('activity.delete', { defaultValue: 'Delete' })}
        </Text>
      </Pressable>
    </View>
  );

  const rowInner = (
      <AnimatedPressable
        style={[styles.row, pressStyle]}
        onPress={handlePress}
        onPressIn={() => { if (onPress) scale.value = withSpring(0.98, motion.spring); }}
        onPressOut={() => { if (onPress) scale.value = withSpring(1, motion.spring); }}
        accessible
        accessibilityRole={onPress ? 'button' : undefined}
        accessibilityLabel={a11yLabel}
      >
        <Animated.View pointerEvents="none" style={[styles.glow, glowStyle]} />

        <View style={[styles.avatar, incoming && styles.avatarIn]}>
          <Arrow size={19} color={incoming ? color.success.base : color.fg.subtle} strokeWidth={2.2} />
          {chain && (
            <View style={styles.badge}>
              <ChainLogo label={chain.iconLabel} color={chain.iconColor} bgColor={chain.iconBg} logoURL={chain.logoURL} size={18} />
            </View>
          )}
        </View>

        <View style={styles.content}>
          {/* Line 1: title ↔ amount */}
          <View style={styles.line}>
            <Text style={styles.title} numberOfLines={1}>{title}</Text>
            <Text
              style={[styles.amount, incoming && styles.amountIn]}
              numberOfLines={1}
              adjustsFontSizeToFit
              minimumFontScale={0.85}
            >
              {numberPart}
              {ticker ? <Text style={styles.ticker}> {ticker}</Text> : null}
            </Text>
          </View>

          {/* Line 2: counterparty (address/alias) ↔ fiat */}
          <View style={styles.line}>
            <Text style={styles.subtitle} numberOfLines={1}>{subtitle}</Text>
            {fiat ? <Text style={styles.fiat} numberOfLines={1}>{fiat}</Text> : null}
          </View>

          {/* Line 3: time */}
          {time ? <Text style={styles.time} numberOfLines={1}>{time}</Text> : null}
        </View>
      </AnimatedPressable>
  );

  return (
    <Animated.View entering={hasEntered.current ? undefined : fadeInDown(index * 40, 300)}>
      {/* Shadow lives on this wrapper — OUTSIDE Swipeable's overflow:hidden, which
          would otherwise clip the card shadow. Swipeable is always rendered (only
          the actions are gated) so the row subtree stays structurally stable across
          a pending→confirmed update and the entrance animation never replays. */}
      <View style={styles.shadowWrap}>
        <Swipeable
          ref={swipeRef}
          overshootRight={false}
          friction={2}
          rightThreshold={36}
          renderRightActions={canSwipe ? renderRightActions : undefined}
        >
          {rowInner}
        </Swipeable>
      </View>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  // De-boxed (Apple Wallet / Wise minimal): rows are edge-to-edge on the page,
  // separated by a hairline in the list — no per-row card, border, or shadow.
  // Wrapper stays for the Swipeable + entrance-animation structure; its bg matches
  // the page so a swipe reveals the page (not a card) behind the row.
  shadowWrap: {
    backgroundColor: color.bg.base,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    backgroundColor: color.bg.base,
    paddingVertical: space.xl,
    paddingHorizontal: space.xs,
  },
  glow: {
    // "Just arrived" — a soft success wash across the row, no bordered box.
    position: 'absolute',
    top: 0, left: 0, right: 0, bottom: 0,
    backgroundColor: color.success.soft,
  },
  avatar: {
    width: 44,
    height: 44,
    borderRadius: 22,
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
  content: {
    flex: 1,
    minWidth: 0,
    gap: 3,
  },
  line: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: space.md,
  },
  // Direction label is supporting context (the arrow + amount color/sign carry
  // it) — kept calm so the amount and counterparty lead the eye.
  title: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
    flexShrink: 1,
  },
  amount: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'right',
    flexShrink: 1,
  },
  amountIn: {
    color: color.success.base,
  },
  ticker: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  // The counterparty owns the second line and only shares it with the short
  // fiat value, so the (fixed-width mono) address has room and isn't clipped.
  subtitle: {
    flexGrow: 1,
    flexShrink: 1,
    minWidth: 0,
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
  },
  fiat: {
    flexShrink: 0,
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  time: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  swipeActions: {
    flexDirection: 'row',
    alignItems: 'stretch',
  },
  swipeAction: {
    width: 72,
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.xs,
    borderRadius: radius.xl,
    marginLeft: space.sm,
  },
  swipeDelete: {
    backgroundColor: color.error.base,
  },
  swipeActionText: {
    fontSize: text.xs,
    ...inter.semibold,
  },
  swipeActionTextLight: { color: color.fg.inverse },
}));
