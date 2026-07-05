import { APP_VERSION, GIT_COMMIT } from '@/constants/build-info';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { scaleFont, color, createStyles, font, inter, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { getAllNetworksSync } from '@/models/network';
import { hapticSuccess, openURL } from '@/services/platform';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { ArrowLeft, ExternalLink } from 'lucide-react-native';
import React, { useMemo, useRef } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useTranslation } from 'react-i18next';

export default function AboutScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();
  const networks = useMemo(() => getAllNetworksSync(), []);
  const devTapRef = useRef({ count: 0, lastTap: 0 });

  const handleLogoTap = () => {
    const now = Date.now();
    const ref = devTapRef.current;
    if (now - ref.lastTap > 3000) ref.count = 0;
    ref.lastTap = now;
    ref.count++;
    if (ref.count >= 6) {
      ref.count = 0;
      AsyncStorage.setItem('dev_unlocked', '1');
      hapticSuccess();
    }
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.title}>{t('about.title')}</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* Logo — tap 6 times to unlock developer options */}
        <Animated.View style={styles.logoSection} entering={fadeIn(0, 400)}>
          <Pressable onPress={handleLogoTap}>
            <Text style={styles.logo}>
              vel<Text style={styles.logoAccent}>a</Text>
            </Text>
          </Pressable>
          <Text style={styles.version}>{t('about.version', { version: APP_VERSION, commit: GIT_COMMIT })}</Text>
          <Text style={styles.tagline}>{t('about.tagline')}</Text>
        </Animated.View>

        {/* Technical */}
        <Animated.View entering={fadeInDown(150, 400)}>
          <Text style={styles.sectionTitle}>{t('about.sectionTechnical')}</Text>
          <VelaCard style={styles.techCard}>
            <TechRow label={t('about.techWalletLabel')} value={t('about.techWalletValue')} />
            <TechRow label={t('about.techAuthLabel')} value={t('about.techAuthValue')} />
            <TechRow label={t('about.techAccountTypeLabel')} value={t('about.techAccountTypeValue')} />
            <TechRow label={t('about.techSignerLabel')} value={t('about.techSignerValue')} />
            <TechRow label={t('about.techNetworksLabel')} value={t('about.techNetworksValue', { count: networks.length })} />
          </VelaCard>
        </Animated.View>

        {/* Links */}
        <Animated.View entering={fadeInDown(200, 400)}>
          <VelaCard style={styles.linksCard}>
            <LinkRow label={t('about.linkWebsite')} url="https://getvela.app" />
            <View style={styles.separator} />
            <LinkRow label={t('about.linkGitHub')} url="https://github.com/atshelchin/vela-wallet" />
            <View style={styles.separator} />
            <LinkRow label={t('about.linkSafeWallet')} url="https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1" />
          </VelaCard>
        </Animated.View>

        <Text style={styles.footer}>
          {t('about.footer')}
        </Text>
      </ScrollView>
    </ScreenContainer>
  );
}

function TechRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.techRow}>
      <Text style={styles.techLabel}>{label}</Text>
      <Text style={styles.techValue}>{value}</Text>
    </View>
  );
}

function LinkRow({ label, url }: { label: string; url: string }) {
  return (
    <Pressable style={styles.linkRow} onPress={() => openURL(url)}>
      <Text style={styles.linkLabel}>{label}</Text>
      <ExternalLink size={14} color={color.fg.subtle} strokeWidth={2} />
    </Pressable>
  );
}

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  headerSpacer: { minWidth: 50 },

  // Logo
  logoSection: {
    alignItems: 'center',
    marginBottom: space['3xl'],
  },
  logo: {
    fontSize: scaleFont(40),
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: 3,
  },
  logoAccent: {
    color: color.accent.base,
  },
  version: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
    marginTop: space.sm,
  },
  tagline: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    marginTop: space.md,
  },

  // Section
  sectionTitle: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase',
    letterSpacing: 1,
    marginBottom: space.lg,
    paddingHorizontal: space.sm,
  },

  // Tech
  techCard: {
    padding: space.xl,
    marginBottom: space['2xl'],
  },
  techRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.md,
  },
  techLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },
  techValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    fontFamily: font.mono,
  },

  // Links
  linksCard: {
    marginBottom: space['3xl'],
  },
  linkRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.xl,
    paddingHorizontal: space['2xl'],
  },
  linkLabel: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
  },

  // Footer
  footer: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 20,
  },
}));
