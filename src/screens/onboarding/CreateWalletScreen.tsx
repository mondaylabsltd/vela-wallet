import React, { useState, useRef } from 'react';
import { View, Text, TextInput, Pressable, ScrollView, Keyboard } from 'react-native';
import { useRouter } from 'expo-router';
import { useTranslation } from 'react-i18next';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { VelaButton } from '@/components/ui/VelaButton';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { BugReportModal } from '@/components/ui/BugReportModal';
import { useLanguagePreference } from '@/i18n/language';
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
  const [showBugReport, setShowBugReport] = useState(false);
  // A passkey that registered OK but hasn't proven it can sign yet. Kept in
  // state so a cancelled verification can resume (re-sign only) without
  // minting a second passkey, and so the button label reflects the resume.
  const [pendingReg, setPendingReg] = useState<{
    registration: Passkey.PasskeyRegistrationResult;
    name: string;
  } | null>(null);
  const [showErrorDetail, setShowErrorDetail] = useState(false);
  const { resolved: language } = useLanguagePreference();

  // WebAuthn user.id caps at 64 bytes; the UTF-8 name gets 27 of them (see
  // MAX_USER_NAME_BYTES). Validate live — without this, a long (esp. CJK)
  // name only fails deep inside the passkey ceremony with a cryptic
  // "User handle exceeds 64 bytes."
  const nameTooLong =
    new TextEncoder().encode(name.trim()).length > Passkey.MAX_USER_NAME_BYTES;
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
    const trimmed = pendingReg?.name ?? name.trim();
    if (!trimmed || loading) return;
    if (!pendingReg && nameTooLong) return; // resumed names were validated at registration
    setLoading(true);
    setStatus('');
    setUploadFailed(false);
    setUploadError('');

    // ------------------------------------------------------------------
    // Stage 1 — register the passkey (skipped when resuming a cancelled
    // verification, so we never mint a second passkey for the same wallet).
    // ------------------------------------------------------------------
    let registration = pendingReg?.registration ?? null;
    if (!registration) {
      try {
        const supported = await Passkey.isSupported();
        if (!supported) {
          showAlert(t('onboarding.create.alertNotSupportedTitle'), t('onboarding.create.alertNotSupportedBody'));
          setLoading(false);
          return;
        }

        setStatus(t('onboarding.create.statusSettingUpIdentity'));
        registration = await Passkey.register(trimmed);
        setPendingReg({ registration, name: trimmed });
      } catch (error) {
        if (error instanceof PasskeyError && error.code === PasskeyErrorCode.CANCELLED) {
          setStatus(t('onboarding.create.statusSetupCancelled'));
        } else if (error instanceof PasskeyError && error.code === PasskeyErrorCode.NOT_DISCOVERABLE) {
          // The authenticator created a device-local (non-discoverable) passkey.
          // It would sign fine on this device but never appear at sign-in or sync
          // for recovery — nothing was saved, so guide the user to a compatible
          // provider instead (issue #1).
          showAlert(
            t('onboarding.create.alertNotDiscoverableTitle'),
            t('onboarding.create.alertNotDiscoverableBody'),
          );
        } else {
          showAlert(t('onboarding.create.alertErrorTitle'), error instanceof Error ? error.message : String(error));
        }
        setLoading(false);
        return;
      }
    }

    // ------------------------------------------------------------------
    // Stage 2 — prove the passkey can actually SIGN before anything is
    // persisted or the address is ever shown. A provider can report a
    // successful create() and still fail to durably store the credential
    // (issue #1: "created successfully" yet absent from Google Password
    // Manager, with nowhere to sign). Verifying up front means a dead
    // passkey aborts cleanly: no index record, no local account, no
    // fundable address — instead of a permanently unusable wallet.
    // ------------------------------------------------------------------
    try {
      setStatus(t('onboarding.create.statusVerifyingIdentity'));
      const testChallenge = toHex(new TextEncoder().encode('vela-verify-' + Date.now()));
      const assertion = await Passkey.sign(testChallenge, registration.credentialId);
      const compat = verifySafeWebAuthn(assertion);
      if (!compat.ok) {
        // Non-retryable: the provider's response format can't work with the
        // Safe contracts. The user needs a different provider (B05).
        setPendingReg(null);
        showAlert(
          t('onboarding.login.alertIncompatibleTitle'),
          t('onboarding.login.alertIncompatibleBodyCreate'),
        );
        setLoading(false);
        setStatus('');
        return;
      }

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

      // Persist a pending upload (drives retry) but DO NOT save the account
      // locally yet. The account is saved only once the public key is confirmed
      // on the index server (see below). Otherwise a sync failure would leave a
      // wallet that's usable on THIS device but unrecoverable on any other —
      // boot auto-enters on any saved account, and login checks local first, so
      // the server-side gap would stay silent. No funds risk: the address is
      // only shown on the success screen — after signing is proven and the key
      // is synced — so an unverified or unsynced wallet is never funded.
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

      await saveAccount(account); // only now that signing is proven AND the key is confirmed server-side
      setCreated(true);
      setLoading(false);
      setStatus('');

    } catch (error) {
      if (error instanceof PasskeyError && error.code === PasskeyErrorCode.CANCELLED) {
        // Verification cancelled — pendingReg is kept, so the button resumes
        // from the signature (never a second registration).
        setStatus(t('onboarding.create.statusVerifyCancelled'));
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
      await saveAccount(pending.account); // confirmed server-side — now safe to persist locally
      setCreated(true);
      setUploadFailed(false);
    }
    setLoading(false);
    setStatus('');
  }

  function handleStartOver() {
    // Abandon the unverified passkey and let the user mint a fresh one.
    // Nothing about the old one was persisted (no account, no upload), so
    // this is a clean reset; the orphaned authenticator entry is inert.
    setPendingReg(null);
    setStatus('');
  }

  function handleEnter() {
    // Signing was already proven during creation (stage 2 of handleCreate) —
    // entering the wallet is now just a state transition.
    const pending = pendingRef.current;
    if (!pending || loading) return;
    dispatch({ type: 'ADD_ACCOUNT', account: pending.account });
    onCreated?.(pending.account.address, pending.account.name);
    router.replace('/(tabs)/wallet');
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
            <Text style={styles.hint}>
              {t('onboarding.create.syncFailedHint')}
            </Text>
            {onOpenSettings && (
              <Pressable style={styles.settingsLink} onPress={onOpenSettings}>
                <Text style={styles.settingsLinkText}>{t('onboarding.create.openSettings')}</Text>
              </Pressable>
            )}
            <Pressable style={styles.reportLink} onPress={() => setShowBugReport(true)}>
              <Text style={styles.reportLinkText}>{t('onboarding.create.reportError')}</Text>
            </Pressable>
            {/* Raw error text is for the bug report, not the user — keep it
                behind a quiet disclosure instead of an alarming red box. */}
            {uploadError ? (
              <>
                <Pressable style={styles.detailsToggle} onPress={() => setShowErrorDetail(v => !v)}>
                  <Text style={styles.detailsToggleText}>{t('onboarding.create.technicalDetails')}</Text>
                </Pressable>
                {showErrorDetail ? (
                  <View style={styles.errorDetail}>
                    <Text style={styles.errorDetailText}>{uploadError}</Text>
                  </View>
                ) : null}
              </>
            ) : null}
          </Animated.View>
        ) : (
          <ScrollView showsVerticalScrollIndicator={false} keyboardShouldPersistTaps="handled">
            <Animated.View entering={fadeIn(0, 400)}>
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
                editable={!loading && !pendingReg}
              />
              {nameTooLong ? (
                <Text style={styles.nameTooLongText}>
                  {t('onboarding.create.nameTooLong')}
                </Text>
              ) : (
                <Text style={styles.hint}>
                  {t('onboarding.create.accountNameHint')}
                </Text>
              )}

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
                  title={pendingReg ? t('onboarding.create.finishVerifyBtn') : t('onboarding.create.createWalletBtn')}
                  onPress={handleCreate}
                  disabled={(!pendingReg && (!name.trim() || nameTooLong)) || loading || !checks.every(Boolean)}
                  loading={loading}
                />
                {/* Escape hatch for a passkey that keeps failing verification
                    (e.g. the provider reported success but never durably
                    stored it — issue #1): discard it and start over, instead
                    of being trapped retrying a signature that can never
                    succeed. */}
                {pendingReg && !loading ? (
                  <>
                    <Text style={styles.startOverHint}>{t('onboarding.create.verifyStuckHint')}</Text>
                    <Pressable style={styles.startOverLink} onPress={handleStartOver}>
                      <Text style={styles.startOverText}>{t('onboarding.create.startOverBtn')}</Text>
                    </Pressable>
                  </>
                ) : null}
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
            title={t('onboarding.create.enterWalletBtn')}
            onPress={handleEnter}
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

      <BugReportModal
        visible={showBugReport}
        language={language}
        area="onboarding-sync"
        prefillWhat={t('onboarding.create.reportPrefill') + (uploadError ? `\n\n${uploadError}` : '')}
        onClose={() => setShowBugReport(false)}
      />
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
  nameTooLongText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.accent.base,
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
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
  },
  errorDetailText: {
    fontSize: text.xs,
    color: color.fg.muted,
    fontFamily: font.mono,
    lineHeight: 16,
  },
  detailsToggle: {
    paddingVertical: space.sm,
    paddingHorizontal: space.xl,
    marginTop: space.md,
  },
  detailsToggleText: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    textDecorationLine: 'underline',
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
  reportLink: {
    paddingVertical: space.sm,
    paddingHorizontal: space.xl,
  },
  reportLinkText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
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

  // Start-over escape hatch (verification stuck on a dead passkey)
  startOverHint: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    lineHeight: 18,
    textAlign: 'center',
    marginTop: space['2xl'],
  },
  startOverLink: {
    alignSelf: 'center',
    paddingVertical: space.md,
    paddingHorizontal: space.xl,
    marginTop: space.xs,
  },
  startOverText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
    textDecorationLine: 'underline',
  },
}));
