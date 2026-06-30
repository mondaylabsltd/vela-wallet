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
import React, { useEffect, useRef, useState } from 'react';
import { AccessibilityInfo, Pressable, Text, View } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withSpring, withTiming } from 'react-native-reanimated';
import { Swipeable } from 'react-native-gesture-handler';
import { useTranslation } from 'react-i18next';
import { ArrowDownLeft, ArrowUpRight, Copy, Check, ExternalLink } from 'lucide-react-native';
import { ChainLogo } from '@/components/ChainLogo';
import { fadeInDown } from '@/constants/entering';
import type { Network } from '@/models/network';
import { hapticLight, openBrowser, copyToClipboard } from '@/services/platform';
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
  /** On-chain tx hash; when present the row reveals copy / explorer swipe actions. */
  txHash?: string;
}

export function ActivityRow({ direction, title, subtitle, amount, fiat, time, chain, onPress, index = 0, isNew, txHash }: ActivityRowProps) {
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

  // Swipe-left reveals copy-hash (always, when there's a hash) + view-on-explorer
  // (when the chain has an explorer). Mirrors the dApp/history detail actions.
  const swipeRef = useRef<Swipeable>(null);
  const [copied, setCopied] = useState(false);
  const mountedRef = useRef(true);
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    mountedRef.current = false;
    if (copyTimer.current) clearTimeout(copyTimer.current);
  }, []);

  const explorerUrl = txHash && chain?.explorerURL
    ? `${chain.explorerURL.replace(/\/$/, '')}/tx/${txHash}`
    : undefined;
  const canSwipe = !!txHash;

  const handleViewExplorer = () => {
    hapticLight();
    swipeRef.current?.close();
    if (explorerUrl) openBrowser(explorerUrl);
  };
  const handleCopyHash = async () => {
    if (!txHash) return;
    hapticLight();
    await copyToClipboard(txHash);
    if (!mountedRef.current) return;
    setCopied(true);
    AccessibilityInfo.announceForAccessibility(t('receive.copied', { defaultValue: 'Copied' }));
    if (copyTimer.current) clearTimeout(copyTimer.current);
    copyTimer.current = setTimeout(() => {
      if (mountedRef.current) setCopied(false);
      swipeRef.current?.close();
    }, 1200);
  };

  const renderRightActions = () => (
    <View style={styles.swipeActions}>
      <Pressable
        style={[styles.swipeAction, styles.swipeCopy]}
        onPress={handleCopyHash}
        accessibilityRole="button"
        accessibilityLabel={t('activity.copyHash', { defaultValue: 'Copy transaction hash' })}
      >
        {copied
          ? <Check size={17} color={color.fg.inverse} strokeWidth={2.6} />
          : <Copy size={17} color={color.fg.inverse} strokeWidth={2.2} />}
        <Text style={[styles.swipeActionText, styles.swipeActionTextLight]}>
          {copied ? t('receive.copied', { defaultValue: 'Copied' }) : t('activity.copy', { defaultValue: 'Copy' })}
        </Text>
      </Pressable>
      {explorerUrl && (
        <Pressable
          style={[styles.swipeAction, styles.swipeExplorer]}
          onPress={handleViewExplorer}
          accessibilityRole="button"
          accessibilityLabel={t('history.viewOnExplorer', { defaultValue: 'View on Explorer' })}
        >
          {/* Dark glyph + label on the orange pill — white failed WCAG AA (3.6:1). */}
          <ExternalLink size={17} color={color.fg.base} strokeWidth={2.2} />
          <Text style={[styles.swipeActionText, styles.swipeActionTextDark]}>{t('componentsTx.explorer', { defaultValue: 'Explorer' })}</Text>
        </Pressable>
      )}
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
          <Arrow size={24} color={incoming ? color.success.base : color.fg.muted} strokeWidth={2.4} />
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
    <Animated.View entering={fadeInDown(index * 40, 300)}>
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
  // Shadow moved to shadowWrap (Swipeable clips it); the card keeps bg + border.
  shadowWrap: {
    borderRadius: radius.xl,
    backgroundColor: color.bg.raised,
    ...shadow.sm,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingVertical: space.lg,
    paddingHorizontal: space.lg,
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
  swipeCopy: {
    backgroundColor: color.fg.muted,
  },
  swipeExplorer: {
    backgroundColor: color.accent.base,
  },
  swipeActionText: {
    fontSize: text.xs,
    ...inter.semibold,
  },
  swipeActionTextLight: { color: color.fg.inverse },
  swipeActionTextDark: { color: color.fg.base },
}));
