/**
 * Modal for funding the gas account.
 *
 * Two-step flow:
 * 1. "Request Free Gas" — user explicitly requests sponsorship
 * 2. If denied → shows deposit address for manual funding
 *
 * This makes the process transparent: the user understands what's
 * happening instead of seeing an unexplained deposit screen.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { copyToClipboard, hapticSuccess, hapticLight } from '@/services/platform';
import { Check, Copy, RefreshCw, Fuel, Gift, ChevronDown } from 'lucide-react-native';

import { AppModal } from './AppModal';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, getAllNetworksSync } from '@/models/network';
import { QRCode } from '@/components/QRCode';
import { VelaCard } from './VelaCard';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import {
  fetchBundlerAccountInfo,
  clearBundlerCache,
  requestGasSponsorship,
  type FundingNeeded,
} from '@/services/bundler-service';

interface Props {
  visible: boolean;
  funding: FundingNeeded;
  onFunded: () => void;
  onCancel: () => void;
}

const POLL_INTERVAL = 10_000; // 10s

type Step = 'request' | 'deposit';

/** Human-readable denial reason after sponsorship request fails. */
function denialText(reason?: string): string {
  if (!reason) return 'Free activation is not available right now.';
  if (reason === 'custom_network')
    return 'Free activation is not supported on custom networks.';
  if (reason === 'nonce_exceeded')
    return 'Free activation is for your first few transactions. You\'ve used your free quota.';
  if (reason === 'treasury_depleted')
    return 'Our activation fund is temporarily empty. Please try again later or deposit manually.';
  if (reason === 'wallet_balance_too_low')
    return 'Your wallet balance is too low to qualify for free activation.';
  if (reason === 'no_passkey_registered')
    return 'Free activation requires a passkey. Set up a passkey first.';
  if (reason === 'rate_limited')
    return 'Too many requests. Please wait a moment and try again.';
  if (reason.startsWith('transfer_failed'))
    return 'Activation transfer failed. Please try again or deposit manually.';
  return 'Free activation is not available right now.';
}

export function BundlerFundingModal({ visible, funding, onFunded, onCancel }: Props) {
  // Skip sponsorship step for custom networks — they don't support it.
  const [step, setStep] = useState<Step>(funding.sponsorshipAvailable ? 'request' : 'deposit');
  const [requesting, setRequesting] = useState(false);
  const [denialReason, setDenialReason] = useState<string | undefined>(
    funding.sponsorshipAvailable ? undefined : 'custom_network',
  );

  const [copied, setCopied] = useState(false);
  const [checking, setChecking] = useState(false);
  const [currentBalance, setCurrentBalance] = useState(funding.currentFormatted);
  const [funded, setFunded] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const requiredWei = funding.thresholdWei;

  const checkBalance = useCallback(async () => {
    setChecking(true);
    clearBundlerCache(funding.chainId, funding.safeAddress);
    try {
      const info = await fetchBundlerAccountInfo(funding.chainId, funding.safeAddress);
      if (info && info.spendableBalance >= requiredWei) {
        setFunded(true);
        setCurrentBalance(formatWei(info.spendableBalance));
        hapticSuccess();
      } else if (info) {
        setCurrentBalance(formatWei(info.spendableBalance));
      }
    } catch { /* ignore */ }
    setChecking(false);
  }, [funding.chainId, funding.safeAddress, requiredWei]);

  // Auto-poll for balance changes (only on deposit step)
  useEffect(() => {
    if (!visible || step !== 'deposit') return;
    pollRef.current = setInterval(checkBalance, POLL_INTERVAL);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [visible, step, checkBalance]);

  const handleRequestSponsorship = async () => {
    setRequesting(true);
    try {
      const result = await requestGasSponsorship(
        funding.chainId,
        funding.safeAddress,
        funding.thresholdWei,
      );
      if (result.sponsored) {
        // Re-check balance to confirm
        clearBundlerCache(funding.chainId, funding.safeAddress);
        const info = await fetchBundlerAccountInfo(funding.chainId, funding.safeAddress);
        if (info && info.spendableBalance >= requiredWei) {
          setFunded(true);
          setCurrentBalance(formatWei(info.spendableBalance));
          hapticSuccess();
          // Auto-continue after brief delay so user sees the success
          setTimeout(() => onFunded(), 600);
          setRequesting(false);
          return;
        }
      }
      // Sponsorship denied — show deposit step
      setDenialReason(result.reason);
      setStep('deposit');
    } catch {
      setDenialReason('network_error');
      setStep('deposit');
    }
    setRequesting(false);
  };

  const copyAddress = async () => {
    await copyToClipboard(funding.depositAddress);
    setCopied(true);
    hapticLight();
    setTimeout(() => setCopied(false), 2000);
  };

  const net = getAllNetworksSync().find(n => n.chainId === funding.chainId);

  return (
    <AppModal visible={visible} onClose={onCancel}>
      <View style={styles.container}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.iconWrap}>
            <Fuel size={24} color={color.accent.base} strokeWidth={2} />
          </View>
          <Text style={styles.title}>Gas Account</Text>
          <View style={styles.networkChip}>
            {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={18} />}
            <Text style={styles.networkLabel}>{chainName(funding.chainId)}</Text>
          </View>
        </View>

        {/* Explanation */}
        <Text style={styles.explainText}>
          To send transactions on {chainName(funding.chainId)}, the gas account needs a minimum {funding.nativeSym} balance. You can request free activation or deposit manually.
        </Text>

        {/* Balance card */}
        <VelaCard style={styles.balanceCard}>
          <Text style={styles.balanceLabel}>Gas Balance</Text>
          <Text style={[styles.balanceValue, funded && styles.balanceValueGreen]}>
            {currentBalance} {funding.nativeSym}
          </Text>
        </VelaCard>

        {step === 'request' && !funded && (
          <>
            {/* Step 1: Request sponsorship */}
            <Pressable
              style={[styles.btn, styles.btnSponsor]}
              onPress={handleRequestSponsorship}
              disabled={requesting}
            >
              <Gift size={18} color={color.fg.inverse} strokeWidth={2} />
              <Text style={styles.btnSponsorText}>
                {requesting ? 'Requesting...' : 'Request Free Activation'}
              </Text>
            </Pressable>

            <Pressable style={styles.skipBtn} onPress={() => setStep('deposit')}>
              <Text style={styles.skipText}>I'll deposit manually</Text>
              <ChevronDown size={14} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>

            <Pressable style={styles.cancelBtn} onPress={onCancel}>
              <Text style={styles.cancelText}>Cancel</Text>
            </Pressable>
          </>
        )}

        {step === 'deposit' && !funded && (
          <>
            {/* Denial reason (if came from failed sponsorship) */}
            {denialReason && (
              <VelaCard style={styles.denialCard}>
                <Text style={styles.denialText}>
                  {denialText(denialReason)}
                </Text>
              </VelaCard>
            )}

            {/* Deposit info */}
            <Text style={styles.sectionLabel}>
              Send {funding.nativeSym} to your gas account:
            </Text>

            <View style={styles.qrWrap}>
              <QRCode value={funding.depositAddress} size={120} />
            </View>

            <Pressable style={styles.addressCard} onPress={copyAddress}>
              <View style={styles.addressRow}>
                <Text style={styles.addressLabel}>Address</Text>
                {copied ? (
                  <Check size={14} color={color.accent.base} strokeWidth={3} />
                ) : (
                  <Copy size={14} color={color.fg.subtle} strokeWidth={2} />
                )}
              </View>
              <Text style={styles.addressText} selectable>
                {funding.depositAddress}
              </Text>
            </Pressable>

            <Pressable
              style={[styles.btn, styles.btnSecondary]}
              onPress={checkBalance}
              disabled={checking}
            >
              <RefreshCw size={16} color={color.accent.base} strokeWidth={2} />
              <Text style={styles.btnSecondaryText}>
                {checking ? 'Checking...' : 'Check Balance'}
              </Text>
            </Pressable>

            <Pressable style={styles.cancelBtn} onPress={onCancel}>
              <Text style={styles.cancelText}>Cancel</Text>
            </Pressable>
          </>
        )}

        {funded && (
          <Pressable style={[styles.btn, styles.btnPrimary]} onPress={onFunded}>
            <Check size={18} color={color.fg.inverse} strokeWidth={2.5} />
            <Text style={styles.btnPrimaryText}>Continue</Text>
          </Pressable>
        )}
      </View>
    </AppModal>
  );
}

function formatWei(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.000001) return '< 0.000001';
  if (eth < 0.001) return eth.toFixed(6);
  if (eth < 1) return eth.toFixed(4);
  return eth.toFixed(3);
}

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
    paddingTop: space.xl,
    paddingBottom: space.lg,
  },
  header: {
    alignItems: 'center',
    marginBottom: space.lg,
  },
  iconWrap: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  networkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingHorizontal: space.lg,
    paddingVertical: space.xs,
  },
  networkLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },

  explainText: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 20,
    textAlign: 'center',
    marginBottom: space.lg,
  },

  balanceCard: {
    padding: space.lg,
    marginBottom: space.lg,
    alignItems: 'center',
  },
  balanceLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginBottom: space.xs,
  },
  balanceValue: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  balanceValueGreen: {
    color: color.success.base,
  },

  denialCard: {
    padding: space.lg,
    marginBottom: space.lg,
    backgroundColor: color.warning.soft,
    borderWidth: 1,
    borderColor: color.warning.border,
  },
  denialText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.warning.base,
    lineHeight: 20,
  },

  sectionLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    marginBottom: space.md,
  },

  qrWrap: {
    alignItems: 'center',
    padding: space.lg,
    backgroundColor: '#FFFFFF',
    borderRadius: radius.xl,
    alignSelf: 'center',
    marginBottom: space.md,
    ...shadow.sm,
  },

  addressCard: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.lg,
    marginBottom: space.lg,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  addressRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: space.sm,
  },
  addressLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  addressText: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    lineHeight: 20,
  },

  actions: {
    gap: space.md,
  },
  btn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    borderRadius: radius.xl,
    marginBottom: space.sm,
  },
  btnSponsor: {
    backgroundColor: color.accent.base,
    ...shadow.md,
  },
  btnSponsorText: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.inverse,
  },
  btnPrimary: {
    backgroundColor: color.accent.base,
    ...shadow.md,
  },
  btnPrimaryText: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.inverse,
  },
  btnSecondary: {
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  btnSecondaryText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
  skipBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.xs,
    paddingVertical: space.md,
  },
  skipText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  cancelBtn: {
    alignItems: 'center',
    paddingVertical: space.md,
  },
  cancelText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.subtle,
  },
}));
