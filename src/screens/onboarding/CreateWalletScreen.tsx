import React, { useState, useRef } from 'react';
import { View, Text, TextInput, Pressable, ScrollView, Keyboard } from 'react-native';
import { useRouter } from 'expo-router';
import { useTranslation } from 'react-i18next';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { useWallet } from '@/models/wallet-state';
import { saveAccount, savePendingUpload } from '@/services/storage';
import { computeAddress } from '@/services/safe-address';
import { getAllNetworksSync } from '@/models/network';
import { extractPublicKey } from '@/services/attestation-parser';
import { fromHex, toHex } from '@/services/hex';
import * as Passkey from '@/modules/passkey';
import { PasskeyError, PasskeyErrorCode } from '@/modules/passkey';
import { uploadPublicKey } from '@/services/public-key-upload';
import { verifySafeWebAuthn } from '@/services/webauthn-verify';
import type { StoredAccount } from '@/models/types';
import {
  ArrowLeft, CheckCircle2, AlertTriangle, Loader, Copy, Check, Square, CheckSquare,
  ShieldCheck, Fingerprint, RefreshCw,
} from 'lucide-react-native';
import { showAlert, copyToClipboard, openURL } from '@/services/platform';

interface Props {
  onCreated?: (address: string, name: string) => void;
  onBack?: () => void;
  onOpenSettings?: () => void;
}

// Stable label keys for the acknowledgment checklist; t() is called inside the component.
const ACKNOWLEDGMENT_KEYS = [
  'onboarding.create.ack0',
  'onboarding.create.ack1',
  'onboarding.create.ack2',
  'onboarding.create.ack3', // last item is rendered with inline links — handled specially in JSX
] as const;

export function CreateWalletScreen({ onCreated, onBack, onOpenSettings }: Props) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [checks, setChecks] = useState<boolean[]>(ACKNOWLEDGMENT_KEYS.map(() => false));
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState('');
  const [uploadFailed, setUploadFailed] = useState(false);
  const [uploadError, setUploadError] = useState('');
  const [created, setCreated] = useState(false);
  const [addressCopied, setAddressCopied] = useState(false);
  const pendingRef = useRef<{
    account: StoredAccount;
    credentialId: string;
    publicKeyHex: string;
    name: string;
  } | null>(null);
  const { dispatch } = useWallet();
  const router = useRouter();

  async function tryUpload(params: {
    credentialId: string;
    publicKeyHex: string;
    name: string;
  }): Promise<boolean> {
    // Auto-retry transient sync failures. The index server's on-chain queue can
    // briefly fail (e.g. a 5xx under load); a quick retry almost always succeeds.
    // uploadPublicKey is idempotent — createRecord dedupes server-side and the
    // pending record is only cleared on success — so re-running it is safe.
    const maxAttempts = 3;
    let lastErr: unknown;
    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      try {
        setStatus(t('onboarding.create.statusSyncingKey'));
        await uploadPublicKey(params);
        return true;
      } catch (err) {
        lastErr = err;
        if (attempt < maxAttempts) {
          await new Promise((resolve) => setTimeout(resolve, 1000 * attempt)); // 1s, then 2s
        }
      }
    }
    setUploadError(lastErr instanceof Error ? lastErr.message : String(lastErr));
    return false;
  }

  async function handleCreate() {
    const trimmed = name.trim();
    if (!trimmed || loading) return;
    setLoading(true);
    setStatus('');
    setUploadFailed(false);
    setUploadError('');

    try {
      const supported = await Passkey.isSupported();
      if (!supported) {
        showAlert(t('onboarding.create.alertNotSupportedTitle'), t('onboarding.create.alertNotSupportedBody'));
        setLoading(false);
        return;
      }

      setStatus(t('onboarding.create.statusSettingUpIdentity'));
      const registration = await Passkey.register(trimmed);

      setStatus(t('onboarding.create.statusExtractingKey'));
      const attestationBytes = fromHex(registration.attestationObjectHex);
      const pubKey = extractPublicKey(attestationBytes);
      if (!pubKey) {
        throw new Error('Failed to extract public key from attestation');
      }
      const publicKeyHex = '04' + toHex(pubKey.x) + toHex(pubKey.y);

      setStatus(t('onboarding.create.statusComputingAddress'));
      const address = computeAddress(publicKeyHex);

      const account: StoredAccount = {
        id: registration.credentialId,
        name: trimmed,
        address,
        publicKeyHex,
        createdAt: new Date().toISOString(),
      };
      await saveAccount(account);

      await savePendingUpload({
        id: registration.credentialId,
        name: trimmed,
        publicKeyHex,
        attestationObjectHex: registration.attestationObjectHex,
        createdAt: new Date().toISOString(),
      });

      const uploadParams = { credentialId: registration.credentialId, publicKeyHex, name: trimmed };
      pendingRef.current = { account, ...uploadParams };

      const uploadOk = await tryUpload(uploadParams);
      if (!uploadOk) {
        setUploadFailed(true);
        setLoading(false);
        setStatus('');
        return;
      }

      setCreated(true);
      setLoading(false);
      setStatus('');

    } catch (error) {
      if (error instanceof PasskeyError && error.code === PasskeyErrorCode.CANCELLED) {
        setStatus(t('onboarding.create.statusSetupCancelled'));
      } else {
        showAlert(t('onboarding.create.alertErrorTitle'), error instanceof Error ? error.message : String(error));
      }
      setLoading(false);
    }
  }

  async function handleRetryUpload() {
    const pending = pendingRef.current;
    if (!pending) return;
    setLoading(true);
    setUploadError('');

    const ok = await tryUpload({
      credentialId: pending.credentialId,
      publicKeyHex: pending.publicKeyHex,
      name: pending.name,
    });
    if (ok) {
      setCreated(true);
      setUploadFailed(false);
    }
    setLoading(false);
    setStatus('');
  }

  async function handleSignIn() {
    const pending = pendingRef.current;
    if (!pending || loading) return;
    setLoading(true);
    setStatus(t('onboarding.create.statusVerifyingIdentity'));

    try {
      const testChallenge = toHex(new TextEncoder().encode('vela-verify-' + Date.now()));
      const assertion = await Passkey.sign(testChallenge, pending.credentialId);
      const compat = verifySafeWebAuthn(assertion);

      if (!compat.ok) {
        showAlert(
          t('onboarding.login.alertIncompatibleTitle'),
          t('onboarding.login.alertIncompatibleBodyCreate'),
        );
        setLoading(false);
        setStatus('');
        return;
      }

      dispatch({ type: 'ADD_ACCOUNT', account: pending.account });
      onCreated?.(pending.account.address, pending.account.name);
      router.replace('/(tabs)/wallet');
    } catch (error) {
      if (error instanceof PasskeyError && error.code === PasskeyErrorCode.CANCELLED) {
        setStatus(t('onboarding.create.statusVerifyCancelled'));
      } else {
        showAlert(t('onboarding.create.alertErrorTitle'), error instanceof Error ? error.message : String(error));
        setStatus('');
      }
      setLoading(false);
    }
  }

  return (
    <ScreenContainer edges={['top', 'bottom']}>
      {/* Header */}
      <View style={styles.header}>
        {onBack && !uploadFailed && (
          <Pressable onPress={onBack} hitSlop={8} style={styles.backButton}>
            <ArrowLeft size={20} color={color.accent.base} strokeWidth={2.5} />
          </Pressable>
        )}
        <Text style={styles.title}>
          {created ? t('onboarding.create.headerCreated') : uploadFailed ? t('onboarding.create.headerSyncFailed') : t('onboarding.create.headerDefault')}
        </Text>
        {onBack && !uploadFailed && <View style={styles.headerSpacer} />}
      </View>

      <View style={styles.content}>
        {created ? (
          <Animated.View style={styles.stateContainer} entering={fadeInDown(0, 400)}>
            <View style={styles.stateIconWrap}>
              <CheckCircle2 size={40} color={color.success.base} strokeWidth={1.5} />
            </View>
            <Text style={styles.successTitle}>{t('onboarding.create.successTitle')}</Text>
            <Text style={styles.successMessage}>
              {t('onboarding.create.successMessage', { count: getAllNetworksSync().length })}
            </Text>

            {/* Address display */}
            {pendingRef.current?.account.address && (
              <Pressable
                style={styles.addressBox}
                onPress={async () => {
                  await copyToClipboard(pendingRef.current!.account.address);
                  setAddressCopied(true);
                  setTimeout(() => setAddressCopied(false), 2000);
                }}
              >
                <Text style={styles.addressText} numberOfLines={1} ellipsizeMode="middle">
                  {pendingRef.current.account.address}
                </Text>
                {addressCopied ? (
                  <Check size={14} color={color.success.base} strokeWidth={2.5} />
                ) : (
                  <Copy size={14} color={color.fg.subtle} strokeWidth={2} />
                )}
              </Pressable>
            )}

            <Text style={styles.verifyHint}>
              {t('onboarding.create.verifyHint')}
            </Text>
          </Animated.View>
        ) : uploadFailed ? (
          <Animated.View style={styles.stateContainer} entering={fadeInDown(0, 400)}>
            <View style={styles.stateIconWrapError}>
              <AlertTriangle size={32} color={color.accent.base} strokeWidth={2} />
            </View>
            <Text style={styles.errorTitle}>{t('onboarding.create.syncFailedTitle')}</Text>
            <Text style={styles.errorMessage}>
              {t('onboarding.create.syncFailedMessage')}
            </Text>
            {uploadError ? (
              <VelaCard style={styles.errorDetail}>
                <Text style={styles.errorDetailText}>{uploadError}</Text>
              </VelaCard>
            ) : null}
            <Text style={styles.hint}>
              {t('onboarding.create.syncFailedHint')}
            </Text>
            {onOpenSettings && (
              <Pressable style={styles.settingsLink} onPress={onOpenSettings}>
                <Text style={styles.settingsLinkText}>{t('onboarding.create.openSettings')}</Text>
              </Pressable>
            )}
          </Animated.View>
        ) : (
          <ScrollView showsVerticalScrollIndicator={false} keyboardShouldPersistTaps="handled">
            <Animated.View entering={fadeIn(0, 400)}>
              {/* Reassurance — reframe the no-seed-phrase model as a strength
                  before the legal acknowledgments below. */}
              <VelaCard style={styles.reassureCard}>
                <View style={styles.reassureHead}>
                  <ShieldCheck size={18} color={color.success.base} strokeWidth={2} />
                  <Text style={styles.reassureTitle}>
                    {t('onboarding.create.reassure.title', { defaultValue: 'No seed phrase to lose' })}
                  </Text>
                </View>
                <View style={styles.reassureRow}>
                  <Fingerprint size={15} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.reassureText}>
                    {t('onboarding.create.reassure.point1', { defaultValue: 'Your key is created and held by Face ID / fingerprint — never typed, never shown.' })}
                  </Text>
                </View>
                <View style={styles.reassureRow}>
                  <RefreshCw size={15} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.reassureText}>
                    {t('onboarding.create.reassure.point2', { defaultValue: 'It syncs via iCloud Keychain or Google Password Manager — sign in on a new device to restore.' })}
                  </Text>
                </View>
              </VelaCard>

              <Text style={styles.label}>{t('onboarding.create.accountNameLabel')}</Text>
              <TextInput
                style={styles.input}
                value={name}
                onChangeText={setName}
                placeholder={t('onboarding.create.accountNamePlaceholder')}
                placeholderTextColor={color.fg.subtle}
                autoFocus
                returnKeyType="done"
                onSubmitEditing={() => Keyboard.dismiss()}
                editable={!loading}
              />
              <Text style={styles.hint}>
                {t('onboarding.create.accountNameHint')}
              </Text>

              {/* Acknowledgment checklist */}
              <View style={styles.checklistWrap}>
                {ACKNOWLEDGMENT_KEYS.map((labelKey, i) => {
                  const checked = checks[i];
                  const isLast = i === ACKNOWLEDGMENT_KEYS.length - 1;
                  return (
                    <Pressable
                      key={i}
                      style={styles.checkRow}
                      onPress={() => setChecks(prev => { const next = [...prev]; next[i] = !next[i]; return next; })}
                    >
                      {checked
                        ? <CheckSquare size={18} color={color.accent.base} strokeWidth={2} />
                        : <Square size={18} color={color.fg.subtle} strokeWidth={1.5} />
                      }
                      <Text style={styles.checkText}>
                        {isLast ? (
                          <>
                            {t('onboarding.create.ack3')}
                            <Text style={styles.checkLink} onPress={() => openURL('https://getvela.app/privacy')}>{t('onboarding.create.ack3PrivacyPolicy')}</Text>
                            {t('onboarding.create.ack3And')}
                            <Text style={styles.checkLink} onPress={() => openURL('https://getvela.app/terms')}>{t('onboarding.create.ack3Terms')}</Text>
                            {t('onboarding.create.ack3Period')}
                          </>
                        ) : t(labelKey)}
                      </Text>
                    </Pressable>
                  );
                })}
              </View>

              {status ? (
                <Animated.View style={styles.statusRow} entering={fadeIn(0, 200)}>
                  <Loader size={14} color={color.info.base} />
                  <Text style={styles.status}>{status}</Text>
                </Animated.View>
              ) : null}

              <View style={styles.inlineBottom}>
                <VelaButton
                  title={t('onboarding.create.createWalletBtn')}
                  onPress={handleCreate}
                  disabled={!name.trim() || loading || !checks.every(Boolean)}
                  loading={loading}
                />
              </View>
            </Animated.View>
          </ScrollView>
        )}

        {/* Status + buttons for created/uploadFailed states (not in ScrollView) */}
        {(created || uploadFailed) && status ? (
          <Animated.View style={styles.statusRow} entering={fadeIn(0, 200)}>
            <Loader size={14} color={color.info.base} />
            <Text style={styles.status}>{status}</Text>
          </Animated.View>
        ) : null}
      </View>

      <View style={styles.bottom}>
        {created ? (
          <VelaButton
            title={t('onboarding.create.verifySignInBtn')}
            onPress={handleSignIn}
            loading={loading}
          />
        ) : uploadFailed ? (
          <VelaButton
            title={t('onboarding.create.retryUploadBtn')}
            onPress={handleRetryUpload}
            loading={loading}
          />
        ) : null}
      </View>
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: space.xl,
  },
  backButton: {
    position: 'absolute',
    left: 0,
    width: 44,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  headerSpacer: {
    width: 44,
  },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  content: {
    flex: 1,
    paddingTop: space['4xl'],
  },
  label: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    marginBottom: space.md,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },
  input: {
    fontSize: text.lg,
    ...inter.regular,
    color: color.fg.base,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    paddingHorizontal: space['2xl'],
    paddingVertical: space.xl,
  },
  hint: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.lg,
    lineHeight: 18,
  },
  reassureCard: {
    padding: space.xl,
    gap: space.lg,
    marginBottom: space['3xl'],
  },
  reassureHead: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
  },
  reassureTitle: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  reassureRow: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.md,
  },
  reassureText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 18,
  },
  statusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    marginTop: space.xl,
  },
  status: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.info.base,
  },
  bottom: {
    paddingBottom: space['3xl'],
  },

  // State containers
  stateContainer: {
    alignItems: 'center',
    gap: space.lg,
  },
  stateIconWrap: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: color.success.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.md,
  },
  stateIconWrapError: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.md,
  },
  successTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.success.base,
  },
  successMessage: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
    textAlign: 'center',
  },
  errorTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.accent.base,
  },
  errorMessage: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
    textAlign: 'center',
  },
  errorDetail: {
    padding: space.xl,
    width: '100%',
  },
  errorDetailText: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.accent.base,
  },

  // Wallet ready address
  addressBox: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    width: '100%',
  },
  addressText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  verifyHint: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 18,
    marginTop: space.sm,
  },
  settingsLink: {
    marginTop: space.md,
    paddingVertical: space.md,
    paddingHorizontal: space.xl,
  },
  settingsLinkText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
    textDecorationLine: 'underline',
  },

  // Acknowledgment checklist
  checklistWrap: {
    marginTop: space['3xl'],
    gap: space.xl,
  },
  checkRow: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.lg,
  },
  checkText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
  },
  checkLink: {
    color: color.accent.base,
    ...inter.semibold,
    textDecorationLine: 'underline',
  },
  inlineBottom: {
    marginTop: space['3xl'],
    paddingBottom: space['3xl'],
  },
}));
