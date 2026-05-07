import React, { useState, useEffect, useCallback } from 'react';
import {
  View,
  Text,
  ScrollView,
  TouchableOpacity,
  StyleSheet,
  Alert,
  TextInput,
} from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { useRouter } from 'expo-router';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { ChainLogo } from '@/components/ChainLogo';
import { color, weight, space, radius, font } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { DEFAULT_NETWORKS } from '@/models/network';
import { loadAccounts, saveNetworkConfig, loadNetworkConfigs, clearAll } from '@/services/storage';
import { User as UserIcon, Globe as NetworkIcon, Info as InfoIcon, LogOut as LogOutIcon, Check } from 'lucide-react-native';
import type { NetworkConfig } from '@/models/types';

// ---------------------------------------------------------------------------
// Settings Row
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
    <TouchableOpacity
      style={styles.settingsRow}
      onPress={onPress}
      activeOpacity={onPress ? 0.7 : 1}
      disabled={!onPress}
    >
      <View style={[styles.settingsIcon, { backgroundColor: icon.bg }]}>
        <icon.Icon size={18} color={icon.fg} />
      </View>
      <View style={styles.settingsRowContent}>
        <Text style={styles.settingsRowTitle}>{title}</Text>
        {subtitle ? <Text style={styles.settingsRowSubtitle}>{subtitle}</Text> : null}
      </View>
      {onPress ? <Text style={styles.chevron}>›</Text> : null}
      {showDivider ? <View style={styles.settingsRowDivider} /> : null}
    </TouchableOpacity>
  );
}

// ---------------------------------------------------------------------------
// Settings Section
// ---------------------------------------------------------------------------

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <View style={styles.sectionContainer}>
      <Text style={styles.sectionTitle}>{title.toUpperCase()}</Text>
      <VelaCard>{children}</VelaCard>
    </View>
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
    <VelaCard style={styles.networkCard}>
      <TouchableOpacity
        style={styles.networkHeader}
        onPress={() => setExpanded(!expanded)}
        activeOpacity={0.7}
      >
        <ChainLogo
          label={network.iconLabel}
          color={network.iconColor}
          bgColor={network.iconBg}
          size={36}
        />
        <View style={styles.networkHeaderText}>
          <Text style={styles.networkName}>{network.displayName}</Text>
          <Text style={styles.networkChainId}>Chain {network.chainId}</Text>
        </View>
        <Text style={[styles.chevronSmall, expanded && styles.chevronRotated]}>›</Text>
      </TouchableOpacity>

      {expanded && (
        <View style={styles.networkFields}>
          <View style={styles.dividerFull} />
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
    <View style={styles.configField}>
      <Text style={styles.configLabel}>{label}</Text>
      <TextInput
        style={styles.configInput}
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
      <View style={styles.modalContainer}>
        <View style={styles.modalHeader}>
          <Text style={styles.modalTitle}>Accounts</Text>
          <TouchableOpacity onPress={onClose}>
            <Text style={styles.modalClose}>Done</Text>
          </TouchableOpacity>
        </View>

        <ScrollView style={styles.modalScroll} contentContainerStyle={styles.modalScrollContent}>
          {state.accounts.map((account, index) => {
            const isActive = index === state.activeAccountIndex;
            return (
              <TouchableOpacity
                key={account.id}
                style={[styles.accountItem, isActive && styles.accountItemActive]}
                onPress={() => {
                  dispatch({ type: 'SWITCH_ACCOUNT', index });
                  onClose();
                }}
                activeOpacity={0.7}
              >
                <View style={styles.accountAvatar}>
                  <Text style={styles.accountAvatarText}>
                    {(account.name[0] ?? 'V').toUpperCase()}
                  </Text>
                </View>
                <View style={styles.accountInfo}>
                  <Text style={styles.accountName}>{account.name}</Text>
                  <Text style={styles.accountAddress}>{shortAddress(account.address)}</Text>
                </View>
                {isActive && <Check size={18} color={color.accent.base} />}
              </TouchableOpacity>
            );
          })}

          <View style={styles.accountActions}>
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
// Language Picker Modal
// ---------------------------------------------------------------------------

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
      <View style={styles.modalContainer}>
        <View style={styles.modalHeader}>
          <Text style={styles.modalTitle}>Networks</Text>
          <TouchableOpacity onPress={onClose}>
            <Text style={styles.modalClose}>Done</Text>
          </TouchableOpacity>
        </View>

        <ScrollView
          style={styles.modalScroll}
          contentContainerStyle={styles.networkScrollContent}
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
  const { state, dispatch, activeAccount } = useWallet();
  const router = useRouter();

  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [showNetworkEditor, setShowNetworkEditor] = useState(false);

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
        <Text style={styles.screenTitle}>Settings</Text>

        {/* Account Section */}
        <SettingsSection title="Account">
          <SettingsRow
            icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: UserIcon }}
            title={accountName}
            subtitle={address ? shortAddress(address) : 'Switch account'}
            showDivider={false}
            onPress={() => setShowAccountSwitcher(true)}
          />
        </SettingsSection>

        {/* Networks Section */}
        <SettingsSection title="Networks">
          <SettingsRow
            icon={{ bg: color.info.soft, fg: color.info.base, Icon: NetworkIcon }}
            title="Networks"
            subtitle="Edit RPC, Explorer & Bundler URLs"
            showDivider={false}
            onPress={() => setShowNetworkEditor(true)}
          />
        </SettingsSection>

        {/* General Section */}
        <SettingsSection title="General">
          <SettingsRow
            icon={{ bg: color.bg.sunken, fg: color.fg.muted, Icon: InfoIcon }}
            title="About"
            subtitle="Vela Wallet v1.0.0"
            showDivider={false}
          />
        </SettingsSection>

        {/* Logout Button */}
        <TouchableOpacity style={styles.logoutButton} onPress={handleLogout} activeOpacity={0.7}>
          <LogOutIcon size={16} color={color.accent.base} />
          <Text style={styles.logoutText}>Logout</Text>
        </TouchableOpacity>
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
// Styles
// ---------------------------------------------------------------------------

const styles = StyleSheet.create({
  scrollContent: {
    paddingTop: 8,
    paddingBottom: 40,
  },
  screenTitle: {
    fontSize: 17,
    fontWeight: weight.semibold,
    color: color.fg.base,
    textAlign: 'center',
    marginBottom: 20,
  },

  // Settings Section
  sectionContainer: {
    marginBottom: 24,
  },
  sectionTitle: {
    fontSize: 11,
    fontWeight: weight.semibold,
    color: color.fg.subtle,
    letterSpacing: 1.5,
    marginBottom: 10,
    paddingHorizontal: 14,
  },

  // Settings Row
  settingsRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 18,
    paddingVertical: 16,
    position: 'relative',
  },
  settingsIcon: {
    width: 34,
    height: 34,
    borderRadius: 10,
    alignItems: 'center',
    justifyContent: 'center',
  },
  settingsIconText: {
    fontSize: 15,
  },
  settingsRowContent: {
    flex: 1,
    marginLeft: 14,
    gap: 1,
  },
  settingsRowTitle: {
    fontSize: 15,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  settingsRowSubtitle: {
    fontSize: 12,
    fontWeight: weight.regular,
    color: color.fg.subtle,
  },
  chevron: {
    fontSize: 18,
    color: color.fg.subtle,
    fontWeight: '500',
  },
  settingsRowDivider: {
    position: 'absolute',
    bottom: 0,
    left: 66,
    right: 0,
    height: 1,
    backgroundColor: color.border.base,
  },

  // Logout Button
  logoutButton: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: 16,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    gap: 8,
  },
  logoutIcon: {
    fontSize: 15,
    color: color.accent.base,
    fontWeight: '700',
  },
  logoutText: {
    fontSize: 15,
    fontWeight: weight.semibold,
    color: color.accent.base,
  },

  // Modal
  modalContainer: {
    flex: 1,
    backgroundColor: color.bg.base,
  },
  modalHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space['3xl'],
    paddingVertical: 16,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
  },
  modalTitle: {
    fontSize: 17,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  modalClose: {
    fontSize: 15,
    fontWeight: weight.semibold,
    color: color.accent.base,
  },
  modalScroll: {
    flex: 1,
  },
  modalScrollContent: {
    padding: space['3xl'],
    paddingBottom: 40,
  },

  // Account Switcher
  accountItem: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 16,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    marginBottom: 10,
    gap: 14,
  },
  accountItemActive: {
    borderColor: color.accent.base,
    borderWidth: 1.5,
  },
  accountAvatar: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  accountAvatarText: {
    fontSize: 16,
    fontWeight: weight.semibold,
    color: color.accent.base,
  },
  accountInfo: {
    flex: 1,
    gap: 2,
  },
  accountName: {
    fontSize: 15,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  accountAddress: {
    fontSize: 12,
    fontWeight: weight.medium,
    fontFamily: font.mono,
    color: color.fg.subtle,
  },
  checkmark: {
    fontSize: 20,
    color: color.accent.base,
    fontWeight: '700',
  },
  accountActions: {
    marginTop: 16,
    gap: 10,
  },

  // Language Picker
  languageList: {
    padding: 20,
    gap: 8,
  },
  languageItem: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 20,
    paddingVertical: 16,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    gap: 14,
  },
  languageItemActive: {
    backgroundColor: color.accent.soft,
    borderColor: color.accent.base,
    borderWidth: 1.5,
  },
  languageFlag: {
    fontSize: 24,
  },
  languageName: {
    fontSize: 16,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  languageSpacer: {
    flex: 1,
  },
  checkmarkAccent: {
    fontSize: 20,
    color: color.accent.base,
    fontWeight: '700',
  },

  // Network Editor
  networkScrollContent: {
    padding: 16,
    paddingBottom: 40,
    gap: 12,
  },
  networkCard: {
    overflow: 'hidden',
  },
  networkHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 16,
    gap: 12,
  },
  networkHeaderText: {
    flex: 1,
    gap: 1,
  },
  networkName: {
    fontSize: 15,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  networkChainId: {
    fontSize: 12,
    fontWeight: weight.regular,
    color: color.fg.subtle,
  },
  chevronSmall: {
    fontSize: 16,
    color: color.fg.subtle,
    fontWeight: '500',
  },
  chevronRotated: {
    transform: [{ rotate: '90deg' }],
  },
  networkFields: {
    paddingHorizontal: 16,
    paddingBottom: 16,
    gap: 14,
  },
  dividerFull: {
    height: 1,
    backgroundColor: color.border.base,
    marginHorizontal: -16,
    marginBottom: 2,
  },
  configField: {
    gap: 6,
  },
  configLabel: {
    fontSize: 11,
    fontWeight: weight.semibold,
    color: color.fg.subtle,
    letterSpacing: 1,
  },
  configInput: {
    fontSize: 12,
    fontWeight: weight.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    padding: 12,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.md,
  },
});
