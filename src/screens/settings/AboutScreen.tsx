import { openURL, hapticSuccess } from '@/services/platform';
import AsyncStorage from '@react-native-async-storage/async-storage';
import React, { useMemo, useRef } from 'react';
import { View, Text, ScrollView, Pressable } from 'react-native';
import { getAllNetworksSync } from '@/models/network';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { ArrowLeft, ExternalLink, Code, Star } from 'lucide-react-native';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';

export default function AboutScreen() {
  const router = useSafeRouter();
  const networks = useMemo(() => getAllNetworksSync(), []);
  const networkNames = networks.map(n => n.displayName);
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
          <Text style={styles.title}>About</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* Logo — tap 6 times to unlock developer options */}
        <Animated.View style={styles.logoSection} entering={fadeIn(0, 400)}>
          <Pressable onPress={handleLogoTap}>
            <Text style={styles.logo}>
              vel<Text style={styles.logoAccent}>a</Text>
            </Text>
          </Pressable>
          <Text style={styles.version}>v1.0.0</Text>
        </Animated.View>

        {/* Description */}
        <Animated.View entering={fadeInDown(100, 400)}>
          <VelaCard style={styles.descCard}>
            <Text style={styles.descTitle}>A simpler way to own crypto</Text>
            <Text style={styles.descBody}>
              Vela is an Ethereum wallet that uses your device's biometric authentication (Face ID, fingerprint) instead of seed phrases. Your keys are protected by hardware-level security — no 12 words to write down, no private keys to leak.
            </Text>
          </VelaCard>
        </Animated.View>

        {/* Open Source */}
        <Animated.View entering={fadeInDown(125, 400)}>
          <Pressable
            style={styles.githubCard}
            onPress={() => openURL('https://github.com/atshelchin/vela-wallet-mobile')}
          >
            <View style={styles.githubLeft}>
              <View style={styles.githubIconWrap}>
                <Code size={20} color={color.fg.base} strokeWidth={2} />
              </View>
              <View style={styles.githubText}>
                <Text style={styles.githubTitle}>Open Source</Text>
                <Text style={styles.githubRepo}>atshelchin/vela-wallet-mobile</Text>
              </View>
            </View>
            <View style={styles.githubRight}>
              <Star size={12} color={color.fg.subtle} strokeWidth={2} />
              <Text style={styles.githubAction}>Star</Text>
              <ExternalLink size={12} color={color.fg.subtle} strokeWidth={2} />
            </View>
          </Pressable>
        </Animated.View>

        {/* How it works */}
        <Animated.View entering={fadeInDown(150, 400)}>
          <Text style={styles.sectionTitle}>How it works</Text>
          <VelaCard style={styles.howCard}>
            <InfoRow number="1" title="Device-native security" body="Your wallet key lives inside your device's secure enclave, protected by Face ID or fingerprint." />
            <View style={styles.separator} />
            <InfoRow number="2" title="Smart contract wallet" body="Powered by Safe — the most battle-tested smart contract wallet, securing $100B+ in assets." />
            <View style={styles.separator} />
            <InfoRow number="3" title="Works across networks" body={`One address on ${networkNames.slice(0, -1).join(', ')}, and ${networkNames[networkNames.length - 1]}.`} />
            <View style={styles.separator} />
            <InfoRow number="4" title="Cross-device sign-in" body="Your passkey syncs across devices via iCloud or Google. Vela Wallet never has access to your private key." />
          </VelaCard>
        </Animated.View>

        {/* Technical */}
        <Animated.View entering={fadeInDown(200, 400)}>
          <Text style={styles.sectionTitle}>Technical details</Text>
          <VelaCard style={styles.techCard}>
            <TechRow label="Wallet" value="Safe v1.4.1" />
            <TechRow label="Authentication" value="WebAuthn / P-256" />
            <TechRow label="Account type" value="ERC-4337 (Smart Account)" />
            <TechRow label="Signer module" value="SafeWebAuthnSharedSigner" />
            <TechRow label="Networks" value={`${networks.length} EVM chains`} />
          </VelaCard>
        </Animated.View>

        {/* Links */}
        <Animated.View entering={fadeInDown(250, 400)}>
          <Text style={styles.sectionTitle}>Links</Text>
          <VelaCard style={styles.linksCard}>
            <LinkRow label="Website" url="https://getvela.app" />
            <View style={styles.separator} />
            <LinkRow label="Safe Wallet" url="https://safe.global" />
          </VelaCard>
        </Animated.View>

        <Text style={styles.footer}>
          Built with care. Your keys, your coins.
        </Text>
      </ScrollView>
    </ScreenContainer>
  );
}

function InfoRow({ number, title, body }: { number: string; title: string; body: string }) {
  return (
    <View style={styles.infoRow}>
      <View style={styles.infoNumber}>
        <Text style={styles.infoNumberText}>{number}</Text>
      </View>
      <View style={styles.infoContent}>
        <Text style={styles.infoTitle}>{title}</Text>
        <Text style={styles.infoBody}>{body}</Text>
      </View>
    </View>
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
    fontSize: 40,
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

  // Description
  descCard: {
    padding: space['2xl'],
    marginBottom: space['2xl'],
  },
  descTitle: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.lg,
  },
  descBody: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 22,
  },

  // GitHub card
  githubCard: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    padding: space.xl,
    marginBottom: space['2xl'],
  },
  githubLeft: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    flex: 1,
  },
  githubIconWrap: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  githubText: {
    gap: 2,
    flex: 1,
  },
  githubTitle: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  githubRepo: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.subtle,
  },
  githubRight: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    backgroundColor: color.bg.sunken,
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
  },
  githubAction: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
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

  // How it works
  howCard: {
    padding: space.xl,
    marginBottom: space['2xl'],
  },
  infoRow: {
    flexDirection: 'row',
    gap: space.lg,
    paddingVertical: space.lg,
  },
  infoNumber: {
    width: 28,
    height: 28,
    borderRadius: 14,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  infoNumberText: {
    fontSize: text.sm,
    ...inter.bold,
    color: color.accent.base,
  },
  infoContent: {
    flex: 1,
    gap: space.xs,
  },
  infoTitle: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  infoBody: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
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

  // Footer
  footer: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 20,
  },
}));
