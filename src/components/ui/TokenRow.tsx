import { TokenLogo } from '@/components/TokenLogo';
import { fadeIn } from '@/constants/entering';
import { color, createStyles, font, inter, motion, radius, space, text } from '@/constants/theme';
import type { Network } from '@/models/network';
import React, { useCallback } from 'react';
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
  balance: string;
  usdValue?: string;
  onPress: () => void;
  index?: number;
}

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

export function TokenRow({ symbol, chainLabel, logoUrl, logoUrls, chain, balance, usdValue, onPress, index = 0 }: Props) {
  const scale = useSharedValue(1);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const handlePressIn = useCallback(() => {
    scale.value = withSpring(0.98, motion.spring);
  }, [scale]);

  const handlePressOut = useCallback(() => {
    scale.value = withSpring(1, motion.spring);
  }, [scale]);

  return (
    <Animated.View entering={fadeIn(index * 40, 300)}>
      <AnimatedPressable
        onPress={onPress}
        onPressIn={handlePressIn}
        onPressOut={handlePressOut}
        style={[styles.container, animatedStyle]}
      >
        <TokenLogo symbol={symbol} logoUrl={logoUrl} logoUrls={logoUrls} chain={chain} size={40} />
        <View style={styles.info}>
          <Text style={styles.symbol} numberOfLines={1}>{symbol}</Text>
          <Text style={styles.chain}>{chainLabel}</Text>
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
  info: {
    flex: 1,
    gap: 2,
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
