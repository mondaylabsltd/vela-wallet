import { TokenLogo } from '@/components/TokenLogo';
import { fadeIn } from '@/constants/entering';
import { color, createStyles, font, inter, motion, radius, space, text } from '@/constants/theme';
import type { Network } from '@/models/network';
import { copyToClipboard, hapticLight } from '@/services/platform';
import { Check, Copy } from 'lucide-react-native';
import React, { useCallback, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

interface Props {
  symbol: string;
  chainLabel: string;
  logoUrl?: string | null;
  logoUrls?: string[];
  /** Network whose logo is badged onto the token logo's bottom-right corner. */
  chain?: Network | null;
  /** ERC-20 contract address — shown (tappable to copy) to disambiguate same-named tokens. */
  contractAddress?: string | null;
  balance: string;
  usdValue?: string;
  onPress: () => void;
  index?: number;
  /** When defined, renders a leading checkbox (multi-select / sweep mode). */
  selected?: boolean;
}

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

export function TokenRow({ symbol, chainLabel, logoUrl, logoUrls, chain, contractAddress, balance, usdValue, onPress, index = 0, selected }: Props) {
  const scale = useSharedValue(1);
  const [copied, setCopied] = useState(false);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const handlePressIn = useCallback(() => {
    scale.value = withSpring(0.98, motion.spring);
  }, [scale]);

  const handlePressOut = useCallback(() => {
    scale.value = withSpring(1, motion.spring);
  }, [scale]);

  const copyAddress = useCallback(async () => {
    if (!contractAddress) return;
    await copyToClipboard(contractAddress);
    hapticLight();
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [contractAddress]);

  const shortAddr = contractAddress
    ? `${contractAddress.slice(0, 6)}…${contractAddress.slice(-4)}`
    : '';

  return (
    <Animated.View entering={fadeIn(index * 40, 300)}>
      <AnimatedPressable
        onPress={onPress}
        onPressIn={handlePressIn}
        onPressOut={handlePressOut}
        style={[styles.container, animatedStyle, selected && styles.containerSelected]}
      >
        {selected !== undefined && (
          <View style={[styles.checkbox, selected && styles.checkboxOn]}>
            {selected && <Check size={13} color={color.bg.base} strokeWidth={3} />}
          </View>
        )}
        <TokenLogo symbol={symbol} logoUrl={logoUrl} logoUrls={logoUrls} chain={chain} size={40} />
        <View style={styles.info}>
          <Text style={styles.symbol} numberOfLines={1}>{symbol}</Text>
          <Text style={styles.chain} numberOfLines={1}>{chainLabel}</Text>
          {contractAddress ? (
            <Pressable onPress={copyAddress} hitSlop={8} style={styles.addrChip}>
              <Text style={[styles.addr, copied && styles.addrCopied]} numberOfLines={1}>
                {copied ? 'Copied' : shortAddr}
              </Text>
              {copied
                ? <Check size={11} color={color.success.base} strokeWidth={2.6} />
                : <Copy size={11} color={color.fg.subtle} strokeWidth={2} />}
            </Pressable>
          ) : null}
        </View>
        <View style={styles.values}>
          <Text style={styles.balance} numberOfLines={1} adjustsFontSizeToFit minimumFontScale={0.7}>
            {balance}
          </Text>
          {usdValue ? <Text style={styles.usd} numberOfLines={1}>{usdValue}</Text> : null}
        </View>
      </AnimatedPressable>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  container: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: space.lg,
    paddingHorizontal: space.sm,
    gap: space.lg,
    borderRadius: radius.lg,
  },
  containerSelected: {
    backgroundColor: color.accent.soft,
  },
  checkbox: {
    width: 22,
    height: 22,
    borderRadius: 11,
    borderWidth: 2,
    borderColor: color.border.strong,
    alignItems: 'center',
    justifyContent: 'center',
  },
  checkboxOn: {
    backgroundColor: color.accent.base,
    borderColor: color.accent.base,
  },
  info: {
    flex: 1,
    gap: 3,
  },
  symbol: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },
  chain: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    flexShrink: 0,
  },
  addrChip: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: 4,
    marginTop: 1,
    paddingHorizontal: space.sm,
    paddingVertical: 2,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    maxWidth: '100%',
  },
  addr: {
    fontSize: text.xs,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
    flexShrink: 1,
  },
  addrCopied: {
    color: color.success.base,
  },
  values: {
    alignItems: 'flex-end',
    gap: 2,
  },
  balance: {
    fontSize: text.lg,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },
  usd: {
    fontSize: text.sm,
    ...inter.regular,
    fontFamily: font.numeric,
    color: color.fg.muted,
  },
}));
