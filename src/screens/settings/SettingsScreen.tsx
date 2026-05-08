import React, { useState, useEffect, useCallback } from 'react';
import {
  View,
  Text,
  ScrollView,
  Pressable,
  Alert,
  TextInput,
} from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { useRouter } from 'expo-router';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { ChainLogo } from '@/components/ChainLogo';
import { color, text, weight, space, radius, font, shadow, createStyles, useStyles } from '@/constants/theme';
import { TEXT_SCALE_LEVELS, useTextScale } from '@/constants/text-scale';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { DEFAULT_NETWORKS } from '@/models/network';
import { loadAccounts, saveNetworkConfig, loadNetworkConfigs, clearAll } from '@/services/storage';
import { User as UserIcon, Globe as NetworkIcon, Info as InfoIcon, LogOut as LogOutIcon, Check, ChevronRight, Type as TypeIcon, X } from 'lucide-react-native';
import type { NetworkConfig } from '@/models/types';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';

// ---------------------------------------------------------------------------
// Sub-component styles — use createStyles (update on next mount/focus).
// The main SettingsScreen uses useStyles for instant text-scale feedback.
// ---------------------------------------------------------------------------

type IconConfig = { bg: string; fg: string; Icon: React.ComponentType<{ size: number; color: string }> };

function SettingsRow({
  icon,
  title,
  subtitle,
  showDivider = true,
  onPress,
}: {
  icon: IconConfig;
  title: string;
  subtitle?: string;
  showDivider?: boolean;
  onPress?: () => void;
}) {
  return (
    <Pressable
      style={subStyles.settingsRow}
      onPress={onPress}
      disabled={!onPress}
    >
      <View style={[subStyles.settingsIcon, { backgroundColor: icon.bg }]}>
        <icon.Icon size={16} color={icon.fg} />
      </View>
      <View style={subStyles.settingsRowContent}>
        <Text style={subStyles.settingsRowTitle}>{title}</Text>
        {subtitle ? <Text style={subStyles.settingsRowSubtitle}>{subtitle}</Text> : null}
      </View>
      {onPress ? <ChevronRight size={16} color={color.fg.subtle} /> : null}
      {showDivider ? <View style={subStyles.settingsRowDivider} /> : null}
    </Pressable>
  );
}

// ---------------------------------------------------------------------------
// Network Config Card (expandable)
// ---------------------------------------------------------------------------

function NetworkConfigCard({
  network,
  savedConfig,
  onSave,
}: {
  network: (typeof DEFAULT_NETWORKS)[0];
  savedConfig?: NetworkConfig;
  onSave: (config: NetworkConfig) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [rpcURL, setRpcURL] = useState(savedConfig?.rpcURL ?? network.rpcURL);
  const [explorerURL, setExplorerURL] = useState(savedConfig?.explorerURL ?? network.explorerURL);
  const [bundlerURL, setBundlerURL] = useState(savedConfig?.bundlerURL ?? network.bundlerURL);

  const handleSave = useCallback(() => {
    onSave({ chainId: network.chainId, rpcURL, explorerURL, bundlerURL });
  }, [network.chainId, rpcURL, explorerURL, bundlerURL, onSave]);

  return (
    <VelaCard style={subStyles.networkCard}>
      <Pressable
        style={subStyles.networkHeader}
        onPress={() => setExpanded(!expanded)}
      >
        <ChainLogo
          label={network.iconLabel}
          color={network.iconColor}
          bgColor={network.iconBg}
          logoURL={network.logoURL}
          size={36}
        />
        <View style={subStyles.networkHeaderText}>
          <Text style={subStyles.networkName}>{network.displayName}</Text>
          <Text style={subStyles.networkChainId}>Chain {network.chainId}</Text>
        </View>
        <ChevronRight
          size={16}
          color={color.fg.subtle}
          style={expanded ? { transform: [{ rotate: '90deg' }] } : undefined}
        />
      </Pressable>

      {expanded && (
        <View style={subStyles.networkFields}>
          <View style={subStyles.dividerFull} />
          <ConfigField label="RPC URL" value={rpcURL} onChangeText={setRpcURL} onBlur={handleSave} />
          <ConfigField label="EXPLORER" value={explorerURL} onChangeText={setExplorerURL} onBlur={handleSave} />
          <ConfigField label="BUNDLER" value={bundlerURL} onChangeText={setBundlerURL} onBlur={handleSave} />
        </View>
      )}
    </VelaCard>
  );
}

function ConfigField({
  label,
  value,
  onChangeText,
  onBlur,
}: {
  label: string;
  value: string;
  onChangeText: (text: string) => void;
  onBlur: () => void;
}) {
  return (
    <View style={subStyles.configField}>
      <Text style={subStyles.configLabel}>{label}</Text>
      <TextInput
        style={subStyles.configInput}
        value={value}
        onChangeText={onChangeText}
        onBlur={onBlur}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder={label}
        placeholderTextColor={color.fg.subtle}
      />
    </View>
  );
}

// ---------------------------------------------------------------------------
// Account Switcher Modal
// ---------------------------------------------------------------------------

function AccountSwitcherModal({
  visible,
  onClose,
}: {
  visible: boolean;
  onClose: () => void;
}) {
  const { state, dispatch } = useWallet();
  const router = useRouter();

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={subStyles.modalContainer}>
        <View style={subStyles.modalHeader}>
          <Text style={subStyles.modalTitle}>Accounts</Text>
          <Pressable onPress={onClose} hitSlop={8}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView style={subStyles.modalScroll} contentContainerStyle={subStyles.modalScrollContent}>
          {state.accounts.map((account, index) => {
            const isActive = index === state.activeAccountIndex;
            return (
              <Pressable
                key={account.id}
                style={[subStyles.accountItem, isActive && subStyles.accountItemActive]}
                onPress={() => {
                  dispatch({ type: 'SWITCH_ACCOUNT', index });
                  onClose();
                }}
              >
                <View style={subStyles.accountAvatar}>
                  <Text style={subStyles.accountAvatarText}>
                    {(account.name[0] ?? 'V').toUpperCase()}
                  </Text>
                </View>
                <View style={subStyles.accountInfo}>
                  <Text style={subStyles.accountNameModal}>{account.name}</Text>
                  <Text style={subStyles.accountAddress}>{shortAddress(account.address)}</Text>
                </View>
                {isActive && <Check size={18} color={color.accent.base} />}
              </Pressable>
            );
          })}

          <View style={subStyles.accountActions}>
            <VelaButton
              title="Create New Account"
              onPress={() => {
                onClose();
                router.push('/onboarding');
              }}
            />
            <VelaButton
              title="Login with Passkey"
              variant="secondary"
              onPress={() => {
                onClose();
                router.push('/onboarding');
              }}
            />
          </View>
        </ScrollView>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Network Editor Modal
// ---------------------------------------------------------------------------

function NetworkEditorModal({
  visible,
  onClose,
}: {
  visible: boolean;
  onClose: () => void;
}) {
  const [savedConfigs, setSavedConfigs] = useState<NetworkConfig[]>([]);

  useEffect(() => {
    if (visible) {
      loadNetworkConfigs().then(setSavedConfigs);
    }
  }, [visible]);

  const handleSave = useCallback(async (config: NetworkConfig) => {
    await saveNetworkConfig(config);
    const updated = await loadNetworkConfigs();
    setSavedConfigs(updated);
  }, []);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={subStyles.modalContainer}>
        <View style={subStyles.modalHeader}>
          <Text style={subStyles.modalTitle}>Networks</Text>
          <Pressable onPress={onClose} hitSlop={8}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView
          style={subStyles.modalScroll}
          contentContainerStyle={subStyles.networkScrollContent}
          keyboardShouldPersistTaps="handled"
        >
          {DEFAULT_NETWORKS.map((network) => {
            const saved = savedConfigs.find((c) => c.chainId === network.chainId);
            return (
              <NetworkConfigCard
                key={network.id}
                network={network}
                savedConfig={saved}
                onSave={handleSave}
              />
            );
          })}
        </ScrollView>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Main Settings Screen
// ---------------------------------------------------------------------------

export default function SettingsScreen() {
  const styles = useStyles(styleFactory);
  const { state, dispatch, activeAccount } = useWallet();
  const router = useRouter();

  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [showNetworkEditor, setShowNetworkEditor] = useState(false);
  const { levelIndex: currentScaleIndex, change: changeTextScale } = useTextScale();

  const accountName = activeAccount?.name ?? 'No Wallet';
  const address = activeAccount?.address ?? state.address;

  const handleLogout = () => {
    Alert.alert(
      'Logout',
      'Are you sure you want to logout? This will clear all local data.',
      [
        { text: 'Cancel', style: 'cancel' },
        {
          text: 'Logout',
          style: 'destructive',
          onPress: async () => {
            await clearAll();
            dispatch({ type: 'LOGOUT' });
            router.replace('/');
          },
        },
      ],
    );
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        {/* Title */}
        <Animated.View entering={fadeIn(0, 300)}>
          <Text style={styles.screenTitle}>Settings</Text>
        </Animated.View>

        {/* Account Section */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(50, 300)}>
          <Text style={styles.sectionTitle}>ACCOUNT</Text>
          <VelaCard>
            <SettingsRow
              icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: UserIcon }}
              title={accountName}
              subtitle={address ? shortAddress(address) : 'Switch account'}
              showDivider={false}
              onPress={() => setShowAccountSwitcher(true)}
            />
          </VelaCard>
        </Animated.View>

        {/* Networks Section */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(100, 300)}>
          <Text style={styles.sectionTitle}>NETWORKS</Text>
          <VelaCard>
            <SettingsRow
              icon={{ bg: color.info.soft, fg: color.info.base, Icon: NetworkIcon }}
              title="Networks"
              subtitle="RPC, Explorer & Bundler URLs"
              showDivider={false}
              onPress={() => setShowNetworkEditor(true)}
            />
          </VelaCard>
        </Animated.View>

        {/* Text Size */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(150, 300)}>
          <Text style={styles.sectionTitle}>TEXT SIZE</Text>
          <VelaCard>
          <View style={styles.textScaleStepper}>
            <Pressable
              style={[styles.textScaleBtn, currentScaleIndex === 0 && styles.textScaleBtnDisabled]}
              onPress={() => changeTextScale(-1)}
              disabled={currentScaleIndex === 0}
            >
              <Text style={[styles.textScaleBtnText, currentScaleIndex === 0 && styles.textScaleBtnTextDisabled]}>A−</Text>
            </Pressable>

            <View style={styles.textScaleTrack}>
              {TEXT_SCALE_LEVELS.map((_, i) => (
                <View
                  key={i}
                  style={[
                    styles.textScaleTick,
                    i <= currentScaleIndex && styles.textScaleTickActive,
                    i === currentScaleIndex && styles.textScaleTickCurrent,
                  ]}
                />
              ))}
            </View>

            <Pressable
              style={[styles.textScaleBtn, currentScaleIndex === TEXT_SCALE_LEVELS.length - 1 && styles.textScaleBtnDisabled]}
              onPress={() => changeTextScale(1)}
              disabled={currentScaleIndex === TEXT_SCALE_LEVELS.length - 1}
            >
              <Text style={[styles.textScaleBtnText, currentScaleIndex === TEXT_SCALE_LEVELS.length - 1 && styles.textScaleBtnTextDisabled]}>A+</Text>
            </Pressable>
          </View>
          <Text style={styles.textScaleLabel}>
            {TEXT_SCALE_LEVELS[currentScaleIndex].label}
          </Text>
          </VelaCard>
        </Animated.View>

        {/* General Section */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(200, 300)}>
          <Text style={styles.sectionTitle}>GENERAL</Text>
          <VelaCard>
            <SettingsRow
              icon={{ bg: color.bg.sunken, fg: color.fg.muted, Icon: InfoIcon }}
              title="About"
              subtitle="Vela Wallet v1.0.0"
              showDivider={false}
            />
          </VelaCard>
        </Animated.View>

        {/* Logout Button */}
        <Animated.View entering={fadeInDown(250, 300)}>
          <Pressable style={styles.logoutButton} onPress={handleLogout}>
            <LogOutIcon size={16} color={color.accent.base} />
            <Text style={styles.logoutText}>Logout</Text>
          </Pressable>
        </Animated.View>
      </ScrollView>

      {/* Modals */}
      <AccountSwitcherModal
        visible={showAccountSwitcher}
        onClose={() => setShowAccountSwitcher(false)}
      />
      <NetworkEditorModal
        visible={showNetworkEditor}
        onClose={() => setShowNetworkEditor(false)}
      />
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Styles — main screen uses useStyles (instant update), sub-components use createStyles
// ---------------------------------------------------------------------------

const styleFactory = () => ({
  scrollContent: {
    paddingTop: space.md,
    paddingBottom: space['5xl'],
  },
  screenTitle: {
    fontSize: text['2xl'],
    fontWeight: weight.bold,
    color: color.fg.base,
    marginBottom: space['3xl'],
  },
  sectionContainer: {
    marginBottom: space['2xl'],
  },
  sectionTitle: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.subtle,
    letterSpacing: 1.2,
    textTransform: 'uppercase' as const,
    marginBottom: space.md,
    paddingHorizontal: space.sm,
  },
  logoutButton: {
    flexDirection: 'row' as const,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
    paddingVertical: space.xl,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    gap: space.md,
    ...shadow.sm,
  },
  logoutText: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.accent.base,
  },
  textScaleStepper: {
    flexDirection: 'row' as const,
    alignItems: 'center' as const,
    justifyContent: 'space-between' as const,
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    gap: space.xl,
  },
  textScaleBtn: {
    width: 40,
    height: 40,
    borderRadius: radius.full,
    borderWidth: 1,
    borderColor: color.border.base,
    alignItems: 'center' as const,
    justifyContent: 'center' as const,
    backgroundColor: color.bg.base,
  },
  textScaleBtnDisabled: { opacity: 0.3 },
  textScaleBtnText: {
    fontSize: text.lg,
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  textScaleBtnTextDisabled: { color: color.fg.subtle },
  textScaleTrack: {
    flex: 1,
    flexDirection: 'row' as const,
    alignItems: 'center' as const,
    justifyContent: 'space-between' as const,
    height: 4,
    backgroundColor: color.border.base,
    borderRadius: 2,
    paddingHorizontal: space.sm,
  },
  textScaleTick: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.border.strong },
  textScaleTickActive: { backgroundColor: color.accent.base },
  textScaleTickCurrent: { width: 12, height: 12, borderRadius: 6, backgroundColor: color.accent.base, ...shadow.sm },
  textScaleLabel: {
    fontSize: text.sm,
    fontWeight: weight.medium,
    color: color.fg.muted,
    textAlign: 'center' as const,
    paddingBottom: space.lg,
  },
});

const subStyles = createStyles(() => ({
  // Settings Row
  settingsRow: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: space.xl, paddingVertical: space.xl, position: 'relative' },
  settingsIcon: { width: 34, height: 34, borderRadius: 10, alignItems: 'center', justifyContent: 'center' },
  settingsRowContent: { flex: 1, marginLeft: space.lg, gap: 2 },
  settingsRowTitle: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  settingsRowSubtitle: { fontSize: text.sm, fontWeight: weight.regular, color: color.fg.subtle },
  settingsRowDivider: { position: 'absolute', bottom: 0, left: 66, right: 0, height: 1, backgroundColor: color.border.base },
  // Modal
  modalContainer: { flex: 1, backgroundColor: color.bg.base },
  modalHeader: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', paddingHorizontal: space['3xl'], paddingVertical: space.xl, borderBottomWidth: 1, borderBottomColor: color.border.base },
  modalTitle: { fontSize: text.xl, fontWeight: weight.bold, color: color.fg.base },
  modalClose: { fontSize: text.lg, fontWeight: weight.semibold, color: color.accent.base },
  modalScroll: { flex: 1 },
  modalScrollContent: { padding: space['3xl'], paddingBottom: space['5xl'] },
  // Account Switcher
  accountItem: { flexDirection: 'row', alignItems: 'center', padding: space.xl, backgroundColor: color.bg.raised, borderRadius: radius.xl, borderWidth: 1, borderColor: color.border.base, marginBottom: space.lg, gap: space.lg, ...shadow.sm },
  accountItemActive: { borderColor: color.accent.base, borderWidth: 1.5 },
  accountAvatar: { width: 40, height: 40, borderRadius: 20, backgroundColor: color.accent.soft, alignItems: 'center', justifyContent: 'center' },
  accountAvatarText: { fontSize: text.lg, fontWeight: weight.semibold, color: color.accent.base },
  accountInfo: { flex: 1, gap: 2 },
  accountNameModal: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  accountAddress: { fontSize: text.sm, fontWeight: weight.medium, fontFamily: font.mono, color: color.fg.subtle },
  accountActions: { marginTop: space.xl, gap: space.lg },
  // Network Editor
  networkScrollContent: { padding: space.xl, paddingBottom: space['5xl'], gap: space.lg },
  networkCard: { overflow: 'hidden' },
  networkHeader: { flexDirection: 'row', alignItems: 'center', padding: space.xl, gap: space.lg },
  networkHeaderText: { flex: 1, gap: 2 },
  networkName: { fontSize: text.lg, fontWeight: weight.semibold, color: color.fg.base },
  networkChainId: { fontSize: text.sm, fontWeight: weight.regular, color: color.fg.subtle },
  networkFields: { paddingHorizontal: space.xl, paddingBottom: space.xl, gap: space.lg },
  dividerFull: { height: 1, backgroundColor: color.border.base, marginHorizontal: -space.xl, marginBottom: space.sm },
  configField: { gap: space.sm },
  configLabel: { fontSize: text.xs, fontWeight: weight.semibold, color: color.fg.subtle, letterSpacing: 1, textTransform: 'uppercase' },
  configInput: { fontSize: text.sm, fontWeight: weight.medium, fontFamily: font.mono, color: color.fg.base, padding: space.lg, backgroundColor: color.bg.sunken, borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.base },
}));
