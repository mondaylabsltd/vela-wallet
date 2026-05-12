import React, { useState, useRef } from 'react';
import { View, Text, TextInput, Pressable, ScrollView, Keyboard } from 'react-native';
import { useRouter } from 'expo-router';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, text, inter, space, radius, createStyles } from '@/constants/theme';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { useWallet } from '@/models/wallet-state';
import { saveAccount, savePendingUpload } from '@/services/storage';
import { computeAddress } from '@/services/safe-address';
import { extractPublicKey } from '@/services/attestation-parser';
import { fromHex, toHex } from '@/services/hex';
import * as Passkey from '@/modules/passkey';
import { PasskeyError, PasskeyErrorCode } from '@/modules/passkey';
import { uploadPublicKey } from '@/services/public-key-upload';
import { verifySafeWebAuthn } from '@/services/webauthn-verify';
import type { StoredAccount } from '@/models/types';
import {
  ArrowLeft, CheckCircle2, AlertTriangle, Loader, Copy, Check, Square, CheckSquare,
} from 'lucide-react-native';
import { showAlert, copyToClipboard, openURL } from '@/services/platform';

interface Props {
  onCreated?: (address: string, name: string) => void;
  onBack?: () => void;
  onOpenSettings?: () => void;
}

const ACKNOWLEDGMENTS = [
  'This is a self-custodial wallet. Your passkey private key is managed by your device\'s password manager (iCloud Keychain / Google Password Manager). Vela Wallet cannot access or recover it.',
  'If you lose your device, you can restore your wallet on a new device through your iCloud or Google account.',
  'If your iCloud or Google account is compromised, your wallet control may also be compromised. Protect it with a strong password and 2FA.',
  'I agree to the Privacy Policy and Terms of Service.',
] as const;

export function CreateWalletScreen({ onCreated, onBack, onOpenSettings }: Props) {
  const [name, setName] = useState('');
  const [checks, setChecks] = useState<boolean[]>(ACKNOWLEDGMENTS.map(() => false));
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
    try {
      setStatus('Syncing public key...');
      await uploadPublicKey(params);
      return true;
    } catch (err) {
      setUploadError(err instanceof Error ? err.message : String(err));
      return false;
    }
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
        showAlert('Not Supported', 'Biometric authentication is not available on this device.');
        setLoading(false);
        return;
      }

      setStatus('Setting up secure identity...');
      const registration = await Passkey.register(trimmed);

      setStatus('Extracting public key...');
      const attestationBytes = fromHex(registration.attestationObjectHex);
      const pubKey = extractPublicKey(attestationBytes);
      if (!pubKey) {
        throw new Error('Failed to extract public key from attestation');
      }
      const publicKeyHex = '04' + toHex(pubKey.x) + toHex(pubKey.y);

      setStatus('Computing wallet address...');
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
        setStatus('Setup was cancelled.');
      } else {
        showAlert('Error', error instanceof Error ? error.message : String(error));
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
    setStatus('Verifying identity...');

    try {
      const testChallenge = toHex(new TextEncoder().encode('vela-verify-' + Date.now()));
      const assertion = await Passkey.sign(testChallenge, pending.credentialId);
      const compat = verifySafeWebAuthn(assertion);

      if (!compat.ok) {
        showAlert(
          'Device Not Compatible',
          'Your device\'s identity provider is not compatible with this wallet. Please switch to Google Password Manager in system settings and try again.',
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
        setStatus('Verification was cancelled. Please try again.');
      } else {
        showAlert('Error', error instanceof Error ? error.message : String(error));
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
          {created ? 'Wallet Created' : uploadFailed ? 'Cross-Device Sync' : 'Create Wallet'}
        </Text>
        {onBack && !uploadFailed && <View style={styles.headerSpacer} />}
      </View>

      <View style={styles.content}>
        {created ? (
          <Animated.View style={styles.stateContainer} entering={fadeInDown(0, 400)}>
            <View style={styles.stateIconWrap}>
              <CheckCircle2 size={40} color={color.success.base} strokeWidth={1.5} />
            </View>
            <Text style={styles.successTitle}>Your wallet is ready!</Text>
            <Text style={styles.successMessage}>
              Your address works on all 7 supported networks.
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
              Tap below to verify your identity works before using the wallet.
            </Text>
          </Animated.View>
        ) : uploadFailed ? (
          <Animated.View style={styles.stateContainer} entering={fadeInDown(0, 400)}>
            <View style={styles.stateIconWrapError}>
              <AlertTriangle size={32} color={color.accent.base} strokeWidth={2} />
            </View>
            <Text style={styles.errorTitle}>Sync failed</Text>
            <Text style={styles.errorMessage}>
              Wallet created, but your public key wasn't synced to the server.
              You won't be able to sign in on other devices until this is resolved.
            </Text>
            {uploadError ? (
              <VelaCard style={styles.errorDetail}>
                <Text style={styles.errorDetailText}>{uploadError}</Text>
              </VelaCard>
            ) : null}
            <Text style={styles.hint}>
              Check your network, or configure a custom endpoint below.
            </Text>
            {onOpenSettings && (
              <Pressable style={styles.settingsLink} onPress={onOpenSettings}>
                <Text style={styles.settingsLinkText}>Open Settings</Text>
              </Pressable>
            )}
          </Animated.View>
        ) : (
          <ScrollView showsVerticalScrollIndicator={false} keyboardShouldPersistTaps="handled">
            <Animated.View entering={fadeIn(0, 400)}>
              <Text style={styles.label}>Account Name</Text>
              <TextInput
                style={styles.input}
                value={name}
                onChangeText={setName}
                placeholder="Enter a name for your account"
                placeholderTextColor={color.fg.subtle}
                autoFocus
                returnKeyType="done"
                onSubmitEditing={() => Keyboard.dismiss()}
                editable={!loading}
              />
              <Text style={styles.hint}>
                This name is stored with your public key on-chain for cross-device sign-in.
              </Text>

              {/* Acknowledgment checklist */}
              <View style={styles.checklistWrap}>
                {ACKNOWLEDGMENTS.map((item, i) => {
                  const checked = checks[i];
                  const isLast = i === ACKNOWLEDGMENTS.length - 1;
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
                            I agree to the{' '}
                            <Text style={styles.checkLink} onPress={() => openURL('https://getvela.app/privacy')}>Privacy Policy</Text>
                            {' '}and{' '}
                            <Text style={styles.checkLink} onPress={() => openURL('https://getvela.app/terms')}>Terms of Service</Text>.
                          </>
                        ) : item}
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
                  title="Create Wallet"
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
            title="Verify & Sign in"
            onPress={handleSignIn}
            loading={loading}
          />
        ) : uploadFailed ? (
          <VelaButton
            title="Retry Upload"
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
    fontFamily: 'monospace',
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
