import React, { useState, useRef } from 'react';
import { View, Text, TextInput, Alert, Pressable } from 'react-native';
import { useRouter } from 'expo-router';
import Animated, { FadeIn, FadeInDown } from 'react-native-reanimated';
import { color, text, weight, space, radius, createStyles } from '@/constants/theme';
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
  ArrowLeft, CheckCircle2, AlertTriangle, Loader,
} from 'lucide-react-native';

interface Props {
  onCreated?: (address: string, name: string) => void;
  onBack?: () => void;
}

export function CreateWalletScreen({ onCreated, onBack }: Props) {
  const [name, setName] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState('');
  const [uploadFailed, setUploadFailed] = useState(false);
  const [uploadError, setUploadError] = useState('');
  const [created, setCreated] = useState(false);
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
      setStatus('Saving public key...');
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
        Alert.alert('Not Supported', 'Passkeys are not supported on this device.');
        setLoading(false);
        return;
      }

      setStatus('Creating passkey...');
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
        setStatus('Passkey creation was cancelled.');
      } else {
        Alert.alert('Error', error instanceof Error ? error.message : String(error));
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
    setStatus('Verifying passkey...');

    try {
      const testChallenge = toHex(new TextEncoder().encode('vela-verify-' + Date.now()));
      const assertion = await Passkey.sign(testChallenge, pending.credentialId);
      const compat = verifySafeWebAuthn(assertion);

      if (!compat.ok) {
        Alert.alert(
          'Device Not Compatible',
          'Your passkey provider is not compatible with this wallet. Please switch to Google Password Manager in system settings and try again.',
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
        Alert.alert('Error', error instanceof Error ? error.message : String(error));
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
          {created ? 'Wallet Created' : uploadFailed ? 'Save Public Key' : 'Create Wallet'}
        </Text>
        {onBack && !uploadFailed && <View style={styles.headerSpacer} />}
      </View>

      <View style={styles.content}>
        {created ? (
          <Animated.View style={styles.stateContainer} entering={FadeInDown.duration(400)}>
            <View style={styles.stateIconWrap}>
              <CheckCircle2 size={40} color={color.success.base} strokeWidth={1.5} />
            </View>
            <Text style={styles.successTitle}>Your wallet is ready!</Text>
            <Text style={styles.successMessage}>
              Please sign in to verify your passkey works correctly before using your wallet.
            </Text>
          </Animated.View>
        ) : uploadFailed ? (
          <Animated.View style={styles.stateContainer} entering={FadeInDown.duration(400)}>
            <View style={styles.stateIconWrapError}>
              <AlertTriangle size={32} color={color.accent.base} strokeWidth={2} />
            </View>
            <Text style={styles.errorTitle}>Public key upload failed</Text>
            <Text style={styles.errorMessage}>
              Your passkey was created successfully, but the public key could not be saved to the
              cloud server. Without this, you won't be able to recover your wallet on another device.
            </Text>
            {uploadError ? (
              <VelaCard style={styles.errorDetail}>
                <Text style={styles.errorDetailText}>{uploadError}</Text>
              </VelaCard>
            ) : null}
            <Text style={styles.hint}>
              Please check your network connection and try again.
            </Text>
          </Animated.View>
        ) : (
          <Animated.View entering={FadeIn.duration(400)}>
            <Text style={styles.label}>Account Name</Text>
            <TextInput
              style={styles.input}
              value={name}
              onChangeText={setName}
              placeholder="Enter a name for your account"
              placeholderTextColor={color.fg.subtle}
              autoFocus
              returnKeyType="done"
              onSubmitEditing={handleCreate}
              editable={!loading}
            />
            <Text style={styles.hint}>
              This name is stored locally and helps you identify your accounts.
            </Text>
          </Animated.View>
        )}

        {status ? (
          <Animated.View style={styles.statusRow} entering={FadeIn.duration(200)}>
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
        ) : (
          <VelaButton
            title="Create with Passkey"
            onPress={handleCreate}
            disabled={!name.trim() || loading}
            loading={loading}
          />
        )}
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
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  content: {
    flex: 1,
    paddingTop: space['4xl'],
  },
  label: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.muted,
    marginBottom: space.md,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },
  input: {
    fontSize: text.lg,
    fontWeight: weight.regular,
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
    fontWeight: weight.regular,
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
    fontWeight: weight.medium,
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
    fontWeight: weight.bold,
    color: color.success.base,
  },
  successMessage: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
    lineHeight: 20,
    textAlign: 'center',
  },
  errorTitle: {
    fontSize: text.xl,
    fontWeight: weight.bold,
    color: color.accent.base,
  },
  errorMessage: {
    fontSize: text.base,
    fontWeight: weight.regular,
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
    fontWeight: weight.regular,
    color: color.accent.base,
  },
}));
