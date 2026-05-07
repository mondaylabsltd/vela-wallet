import React, { useState, useRef } from 'react';
import { View, Text, TextInput, StyleSheet, Alert } from 'react-native';
import { useRouter } from 'expo-router';
import { VelaColor, VelaFont, VelaRadius, VelaSpacing } from '@/constants/theme';
import { VelaButton } from '@/components/ui/VelaButton';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { useWallet } from '@/models/wallet-state';
import { saveAccount, savePendingUpload } from '@/services/storage';
import { computeAddress } from '@/services/safe-address';
import { extractPublicKey } from '@/services/attestation-parser';
import { fromHex, toHex } from '@/services/hex';
import * as Passkey from '@/modules/passkey';
import { PasskeyError, PasskeyErrorCode } from '@/modules/passkey';
import { uploadPublicKey } from '@/services/public-key-upload';
import type { StoredAccount } from '@/models/types';

interface Props {
  onCreated?: (address: string, name: string) => void;
  onBack?: () => void;
}

export function CreateWalletScreen({ onCreated, onBack }: Props) {
  const [name, setName] = useState('');
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState('');
  // When passkey is created but upload failed, allow retry without re-registering
  const [uploadFailed, setUploadFailed] = useState(false);
  const [uploadError, setUploadError] = useState('');
  // When creation + upload succeeded, show success and prompt login
  const [created, setCreated] = useState(false);
  const pendingRef = useRef<{
    account: StoredAccount;
    credentialId: string;
    publicKeyHex: string;
    name: string;
  } | null>(null);
  const { dispatch } = useWallet();
  const router = useRouter();

  /** Attempt to upload public key to server. Returns true on success. */
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
      // 1. Check passkey support
      const supported = await Passkey.isSupported();
      if (!supported) {
        Alert.alert('Not Supported', 'Passkeys are not supported on this device.');
        setLoading(false);
        return;
      }

      // 2. Register passkey credential (triggers biometric)
      setStatus('Creating passkey...');
      const registration = await Passkey.register(trimmed);

      // 3. Extract P-256 public key from attestation object
      setStatus('Extracting public key...');
      const attestationBytes = fromHex(registration.attestationObjectHex);
      const pubKey = extractPublicKey(attestationBytes);
      if (!pubKey) {
        throw new Error('Failed to extract public key from attestation');
      }
      const publicKeyHex = '04' + toHex(pubKey.x) + toHex(pubKey.y);

      // 4. Compute deterministic Safe address
      setStatus('Computing wallet address...');
      const address = computeAddress(publicKeyHex);

      // 5. Save account locally first (ensures same-device login always works)
      const account: StoredAccount = {
        id: registration.credentialId,
        name: trimmed,
        address,
        publicKeyHex,
        createdAt: new Date().toISOString(),
      };
      await saveAccount(account);

      // 6. Save pending upload (safety net for retry)
      await savePendingUpload({
        id: registration.credentialId,
        name: trimmed,
        publicKeyHex,
        attestationObjectHex: registration.attestationObjectHex,
        createdAt: new Date().toISOString(),
      });

      // 7. Upload public key to server — MUST succeed before navigating
      const uploadParams = { credentialId: registration.credentialId, publicKeyHex, name: trimmed };
      pendingRef.current = { account, ...uploadParams };

      const uploadOk = await tryUpload(uploadParams);
      if (!uploadOk) {
        setUploadFailed(true);
        setLoading(false);
        setStatus('');
        return;
      }

      // Don't auto-login — require sign-in to verify passkey compatibility
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

  /** Retry upload after previous failure */
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
      finishCreation(pending.account);
    } else {
      setLoading(false);
      setStatus('');
    }
  }



  return (
    <ScreenContainer edges={['top', 'bottom']}>
      <View style={styles.header}>
        {onBack && !uploadFailed && !created && (
          <Text onPress={onBack} style={styles.backButton}>
            Back
          </Text>
        )}
        <Text style={styles.title}>
          {created ? 'Wallet Created' : uploadFailed ? 'Save Public Key' : 'Create Wallet'}
        </Text>
        {onBack && !uploadFailed && !created && <View style={styles.headerSpacer} />}
      </View>

      <View style={styles.content}>
        {created ? (
          <>
            <Text style={styles.successTitle}>Your wallet is ready!</Text>
            <Text style={styles.successMessage}>
              Please sign in to verify your passkey works correctly before using your wallet.
            </Text>
          </>
        ) : uploadFailed ? (
          <>
            <Text style={styles.errorTitle}>Public key upload failed</Text>
            <Text style={styles.errorMessage}>
              Your passkey was created successfully, but the public key could not be saved to the
              cloud server. Without this, you won't be able to recover your wallet on another device.
            </Text>
            {uploadError ? (
              <Text style={styles.errorDetail}>{uploadError}</Text>
            ) : null}
            <Text style={styles.hint}>
              Please check your network connection and try again.
            </Text>
          </>
        ) : (
          <>
            <Text style={styles.label}>Account Name</Text>
            <TextInput
              style={styles.input}
              value={name}
              onChangeText={setName}
              placeholder="Enter a name for your account"
              placeholderTextColor={VelaColor.textTertiary}
              autoFocus
              returnKeyType="done"
              onSubmitEditing={handleCreate}
              editable={!loading}
            />
            <Text style={styles.hint}>
              This name is stored locally and helps you identify your accounts.
            </Text>
          </>
        )}

        {status ? (
          <Text style={styles.status}>{status}</Text>
        ) : null}
      </View>

      <View style={styles.bottom}>
        {created ? (
          <VelaButton
            title="Sign in"
            onPress={() => onBack?.()}
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

const styles = StyleSheet.create({
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: 16,
  },
  backButton: {
    ...VelaFont.body(16),
    color: VelaColor.accent,
    position: 'absolute',
    left: 0,
  },
  headerSpacer: {
    width: 40,
  },
  title: {
    ...VelaFont.title(20),
    color: VelaColor.textPrimary,
  },
  content: {
    flex: 1,
    paddingTop: 32,
  },
  label: {
    ...VelaFont.label(14),
    color: VelaColor.textSecondary,
    marginBottom: 8,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  input: {
    ...VelaFont.body(17),
    color: VelaColor.textPrimary,
    backgroundColor: VelaColor.bgCard,
    borderWidth: 1,
    borderColor: VelaColor.border,
    borderRadius: VelaRadius.card,
    paddingHorizontal: VelaSpacing.cardPadding,
    paddingVertical: 16,
  },
  hint: {
    ...VelaFont.body(14),
    color: VelaColor.textTertiary,
    marginTop: 12,
    lineHeight: 20,
  },
  status: {
    ...VelaFont.body(14),
    color: VelaColor.blue,
    marginTop: 16,
    textAlign: 'center',
  },
  bottom: {
    paddingBottom: 24,
  },
  errorTitle: {
    ...VelaFont.title(18),
    color: VelaColor.accent,
    marginBottom: 12,
  },
  errorMessage: {
    ...VelaFont.body(15),
    color: VelaColor.textSecondary,
    lineHeight: 22,
    marginBottom: 12,
  },
  errorDetail: {
    ...VelaFont.body(13),
    color: VelaColor.accent,
    backgroundColor: VelaColor.bgCard,
    borderRadius: VelaRadius.card,
    padding: 12,
    marginBottom: 12,
    overflow: 'hidden',
  },
  successTitle: {
    ...VelaFont.title(18),
    color: VelaColor.green,
    marginBottom: 12,
  },
  successMessage: {
    ...VelaFont.body(15),
    color: VelaColor.textSecondary,
    lineHeight: 22,
  },
});
