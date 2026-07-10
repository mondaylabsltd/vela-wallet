import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { hapticLight } from '@/services/platform';
import { ArrowLeft, Compass, Puzzle, ShieldCheck, Wallet, Zap } from 'lucide-react-native';
import React from 'react';
import { Linking, Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useTranslation } from 'react-i18next';

// New-user guide for enabling + using the Vela iOS Safari extension. Reached from
// Settings ("Use Vela in Safari"). Walks the smooth all-in-Safari enable path (the
// "Aa" menu → Manage Extensions → enable → Allow) — no Settings-app detour needed —
// then how to connect on a dApp and the one-tap upgrade.
export default function SafariExtensionScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();

  const steps: { icon: React.ComponentType<{ size: number; color: string }>; body: string }[] = [
    { icon: Compass, body: t('safariExt.step1') },
    { icon: Puzzle, body: t('safariExt.step2') },
    { icon: ShieldCheck, body: t('safariExt.step3') },
    { icon: Wallet, body: t('safariExt.step4') },
  ];

  const openSafari = () => {
    hapticLight();
    // Linking.openURL opens the DEFAULT browser (real Safari) — where extensions live
    // (an in-app SFSafariViewController can't run extensions), so the user lands where
    // the "Aa" menu + Vela toggle actually are.
    Linking.openURL('https://getvela.app').catch(() => {});
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.navTitle}>{t('safariExt.navTitle')}</Text>
          <View style={styles.headerSpacer} />
        </View>

        <Animated.View style={styles.hero} entering={fadeIn(0, 400)}>
          <Text style={styles.heroTitle}>{t('safariExt.heroTitle')}</Text>
          <Text style={styles.heroBody}>{t('safariExt.heroBody')}</Text>
        </Animated.View>

        <Animated.View entering={fadeInDown(120, 400)}>
          <Text style={styles.sectionLabel}>{t('safariExt.stepsLabel')}</Text>
          <VelaCard style={styles.stepsCard}>
            {steps.map((step, i) => {
              const Icon = step.icon;
              return (
                <View key={i} style={[styles.step, i < steps.length - 1 && styles.stepDivider]}>
                  <View style={styles.stepNumWrap}>
                    <Text style={styles.stepNum}>{i + 1}</Text>
                  </View>
                  <View style={styles.stepIconWrap}>
                    <Icon size={17} color={color.accent.base} />
                  </View>
                  <Text style={styles.stepBody}>{step.body}</Text>
                </View>
              );
            })}
          </VelaCard>
        </Animated.View>

        <Animated.View entering={fadeInDown(220, 400)}>
          <VelaCard style={styles.oneTapCard}>
            <View style={styles.oneTapHead}>
              <Zap size={16} color={color.accent.base} fill={color.accent.base} />
              <Text style={styles.oneTapTitle}>{t('safariExt.oneTapTitle')}</Text>
            </View>
            <Text style={styles.oneTapBody}>{t('safariExt.oneTapBody')}</Text>
          </VelaCard>
        </Animated.View>

        <Animated.View entering={fadeInDown(300, 400)}>
          <Pressable style={({ pressed }) => [styles.cta, pressed && styles.ctaPressed]} onPress={openSafari}>
            <Text style={styles.ctaLabel}>{t('safariExt.cta')}</Text>
          </Pressable>
          <Text style={styles.ctaHint}>{t('safariExt.ctaHint')}</Text>
        </Animated.View>
      </ScrollView>
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  content: { paddingHorizontal: space['2xl'], paddingBottom: space['5xl'] },
  header: { flexDirection: 'row', alignItems: 'center', height: 44, marginBottom: space.lg },
  navBtn: { width: 40, height: 40, alignItems: 'flex-start', justifyContent: 'center' },
  navTitle: { flex: 1, textAlign: 'center', fontSize: text.lg, color: color.fg.base, ...inter.semibold },
  headerSpacer: { width: 40 },

  hero: { marginTop: space.lg, marginBottom: space['3xl'] },
  heroTitle: { fontSize: text['3xl'], color: color.fg.base, ...inter.bold, letterSpacing: -0.02 },
  heroBody: { fontSize: text.base, lineHeight: 22, color: color.fg.muted, marginTop: space.lg, ...inter.regular },

  sectionLabel: {
    fontSize: text.sm, ...inter.semibold, letterSpacing: 0.04, textTransform: 'uppercase',
    color: color.fg.subtle, marginBottom: space.md, marginLeft: space.xs,
  },
  stepsCard: { padding: 0, marginBottom: space['2xl'] },
  step: { flexDirection: 'row', alignItems: 'flex-start', gap: space.lg, padding: space.xl },
  stepDivider: { borderBottomWidth: 1, borderBottomColor: color.border.base },
  stepNumWrap: {
    width: 22, height: 22, borderRadius: 11, backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center', marginTop: 1,
  },
  stepNum: { fontSize: text.sm, color: color.accent.base, ...inter.bold },
  stepIconWrap: { marginTop: 1 },
  stepBody: { flex: 1, fontSize: text.base, lineHeight: 21, color: color.fg.base, ...inter.regular },

  oneTapCard: { backgroundColor: color.accent.soft, borderColor: 'transparent', marginBottom: space['2xl'] },
  oneTapHead: { flexDirection: 'row', alignItems: 'center', gap: space.sm, marginBottom: space.sm },
  oneTapTitle: { fontSize: text.base, color: color.fg.base, ...inter.semibold },
  oneTapBody: { fontSize: text.sm, lineHeight: 20, color: color.fg.muted, ...inter.regular },

  cta: {
    backgroundColor: color.accent.base, borderRadius: 15, paddingVertical: space.xl,
    alignItems: 'center', marginTop: space.md,
  },
  ctaPressed: { opacity: 0.92 },
  ctaLabel: { fontSize: text.lg, color: '#fff', ...inter.semibold },
  ctaHint: { fontSize: text.sm, color: color.fg.subtle, textAlign: 'center', marginTop: space.md, ...inter.regular },
}));
