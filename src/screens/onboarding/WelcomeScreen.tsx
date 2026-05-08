import React from 'react';
import { View, Text, ActivityIndicator, Pressable } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withSpring,
  FadeIn,
  FadeInUp,
} from 'react-native-reanimated';
import { color, text, weight, space, radius, motion, createStyles } from '@/constants/theme';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

interface Props {
  onCreateWallet: () => void;
  onLogin: () => void;
  loginLoading?: boolean;
}

function AnimatedButton({
  onPress,
  style,
  children,
  disabled,
}: {
  onPress: () => void;
  style: any;
  children: React.ReactNode;
  disabled?: boolean;
}) {
  const scale = useSharedValue(1);
  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  return (
    <AnimatedPressable
      onPress={onPress}
      onPressIn={() => { scale.value = withSpring(0.97, motion.spring); }}
      onPressOut={() => { scale.value = withSpring(1, motion.spring); }}
      disabled={disabled}
      style={[style, animatedStyle]}
    >
      {children}
    </AnimatedPressable>
  );
}

export function WelcomeScreen({ onCreateWallet, onLogin, loginLoading }: Props) {
  return (
    <View style={styles.container}>
      <SafeAreaView style={styles.safeArea} edges={['top', 'bottom']}>
        <View style={styles.logoSection}>
          <Animated.View entering={FadeIn.delay(200).duration(600)}>
            <Text style={styles.logo}>
              vel<Text style={styles.logoAccent}>a</Text>
            </Text>
          </Animated.View>
          <Animated.View entering={FadeIn.delay(500).duration(600)}>
            <Text style={styles.tagline}>
              Your keys, your coins.{'\n'}Simple as a tap.
            </Text>
          </Animated.View>
        </View>

        <Animated.View style={styles.buttonSection} entering={FadeInUp.delay(700).duration(500)}>
          <AnimatedButton onPress={onCreateWallet} style={styles.primaryBtn}>
            <Text style={styles.primaryBtnText}>Create Wallet</Text>
          </AnimatedButton>

          <AnimatedButton
            onPress={onLogin}
            style={styles.secondaryBtn}
            disabled={loginLoading}
          >
            {loginLoading ? (
              <ActivityIndicator color={color.fg.subtle} />
            ) : (
              <Text style={styles.secondaryBtnText}>I already have a wallet</Text>
            )}
          </AnimatedButton>
        </Animated.View>
      </SafeAreaView>
    </View>
  );
}

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: color.fg.base,
  },
  safeArea: {
    flex: 1,
    paddingHorizontal: space['3xl'],
  },
  logoSection: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  logo: {
    fontSize: 48,
    fontWeight: weight.bold,
    color: color.fg.inverse,
    letterSpacing: 3,
  },
  logoAccent: {
    color: color.accent.base,
  },
  tagline: {
    fontSize: text.lg,
    fontWeight: weight.regular,
    color: 'rgba(255,255,255,0.45)',
    marginTop: space.xl,
    textAlign: 'center',
    lineHeight: 24,
  },
  buttonSection: {
    paddingBottom: space['3xl'],
    gap: space.lg,
  },
  primaryBtn: {
    paddingVertical: space['2xl'],
    borderRadius: radius.xl,
    backgroundColor: color.accent.base,
    alignItems: 'center',
    justifyContent: 'center',
  },
  primaryBtnText: {
    fontSize: text.lg,
    fontWeight: weight.bold,
    color: color.fg.inverse,
    letterSpacing: 0.3,
  },
  secondaryBtn: {
    paddingVertical: space['2xl'],
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: 'rgba(255,255,255,0.12)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  secondaryBtnText: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: 'rgba(255,255,255,0.5)',
  },
}));
