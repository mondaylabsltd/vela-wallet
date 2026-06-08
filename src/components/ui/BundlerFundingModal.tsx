/**
 * Gas account activation modal.
 *
 * Step 1: Choose activation method — free (bundler-sponsored) or self-funded
 * Step 2 (if self-funded): Show amount + address + QR
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { copyToClipboard, hapticSuccess, hapticLight } from '@/services/platform';
import { Check, Copy, RefreshCw, Fuel, Gift } from 'lucide-react-native';

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

const POLL_INTERVAL = 10_000;

type Step = 'choose' | 'self-fund';

function denialText(reason?: string): string {
  if (!reason) return 'Free activation unavailable.';
  if (reason === 'nonce_exceeded') return 'Free quota used up.';
  if (reason === 'treasury_depleted') return 'Free fund temporarily empty.';
  if (reason === 'wallet_balance_too_low') return 'Wallet balance too low to qualify.';
  if (reason === 'no_passkey_registered') return 'Passkey required for free activation.';
  if (reason === 'rate_limited') return 'Too many requests. Try later.';
  if (reason.startsWith('transfer_failed')) return 'Transfer failed. Try again.';
  return 'Free activation unavailable.';
}

export function BundlerFundingModal({ visible, funding, onFunded, onCancel }: Props) {
  const [step, setStep] = useState<Step>('choose');
  const [requesting, setRequesting] = useState(false);
  const [denialReason, setDenialReason] = useState<string | undefined>();

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

  useEffect(() => {
    if (!visible || step !== 'self-fund') return;
    pollRef.current = setInterval(checkBalance, POLL_INTERVAL);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [visible, step, checkBalance]);

  const handleFreeActivation = async () => {
    setRequesting(true);
    try {
      const result = await requestGasSponsorship(funding.chainId, funding.safeAddress, funding.thresholdWei);
      if (result.sponsored) {
        clearBundlerCache(funding.chainId, funding.safeAddress);
        const info = await fetchBundlerAccountInfo(funding.chainId, funding.safeAddress);
        if (info && info.spendableBalance >= requiredWei) {
          setFunded(true);
          setCurrentBalance(formatWei(info.spendableBalance));
          hapticSuccess();
          setRequesting(false);
          return;
        }
      }
      setDenialReason(result.reason);
      setStep('self-fund');
    } catch {
      setDenialReason('network_error');
      setStep('self-fund');
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
  // Calculate a practical activation amount.
  // On cheap chains (Gnosis, BSC), the actual deficit can be < 0.000001 — useless to display.
  // Show at least 0.001 (enough for hundreds of txs on cheap chains).
  const MIN_DISPLAY_WEI = 100_000_000_000_000n; // 0.0001
  const deficit = funding.thresholdWei > funding.currentBalance
    ? funding.thresholdWei - funding.currentBalance
    : 0n;
  const rawAmount = deficit > 0n ? (deficit * 12n) / 10n : funding.recommendedWei;
  const displayAmount = rawAmount < MIN_DISPLAY_WEI ? MIN_DISPLAY_WEI : rawAmount;
  const activationAmount = formatWei(displayAmount);

  return (
    <AppModal visible={visible} onClose={onCancel}>
      <View style={styles.container}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.iconWrap}>
            <Fuel size={22} color={color.accent.base} strokeWidth={2} />
          </View>
          <Text style={styles.title}>Activate Gas Account</Text>
          <View style={styles.networkChip}>
            {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
            <Text style={styles.networkLabel}>{chainName(funding.chainId)}</Text>
          </View>
        </View>

        {/* Balance */}
        <View style={styles.balanceRow}>
          <Text style={styles.balanceLabel}>Balance</Text>
          <Text style={[styles.balanceValue, funded && styles.balanceGreen]}>
            {currentBalance} {funding.nativeSym}
          </Text>
        </View>

        {step === 'choose' && !funded && (
          <>
            {/* Two options */}
            <Pressable
              style={[styles.optionCard, styles.optionFree]}
              onPress={handleFreeActivation}
              disabled={requesting}
            >
              <View style={styles.optionHeader}>
                <Gift size={18} color={color.success.base} strokeWidth={2} />
                <Text style={styles.optionTitle}>Free Activation</Text>
                <Text style={styles.optionBadge}>FREE</Text>
              </View>
              <View style={styles.optionDescRow}>
                {requesting && <ActivityIndicator size="small" color={color.success.base} />}
                <Text style={styles.optionDesc}>
                  {requesting ? 'Requesting...' : 'Sponsored by Vela for new users'}
                </Text>
              </View>
            </Pressable>

            <Pressable
              style={[styles.optionCard, styles.optionPaid]}
              onPress={() => setStep('self-fund')}
            >
              <View style={styles.optionHeader}>
                <Fuel size={18} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.optionTitle}>Self Activate</Text>
                <Text style={styles.optionAmount}>{activationAmount} {funding.nativeSym}</Text>
              </View>
              <Text style={styles.optionDesc}>
                Send {funding.nativeSym} to activate the gas account
              </Text>
            </Pressable>

            <Pressable style={styles.cancelBtn} onPress={onCancel}>
              <Text style={styles.cancelText}>Cancel</Text>
            </Pressable>
          </>
        )}

        {step === 'self-fund' && !funded && (
          <>
            {denialReason && (
              <View style={styles.denialRow}>
                <Text style={styles.denialText}>{denialText(denialReason)}</Text>
              </View>
            )}

            {/* Amount needed */}
            <VelaCard style={styles.amountCard}>
              <Text style={styles.amountLabel}>Activation Fee</Text>
              <Text style={styles.amountValue}>
                {activationAmount} {funding.nativeSym}
              </Text>
            </VelaCard>

            {/* QR + Address */}
            <View style={styles.qrWrap}>
              <QRCode value={funding.depositAddress} size={110} />
            </View>

            <Pressable style={styles.addressCard} onPress={copyAddress}>
              <View style={styles.addressRow}>
                <Text style={styles.addressLabel}>Gas Account</Text>
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
              style={[styles.btn, styles.btnCheck]}
              onPress={checkBalance}
              disabled={checking}
            >
              <RefreshCw size={16} color={color.accent.base} strokeWidth={2} />
              <Text style={styles.btnCheckText}>
                {checking ? 'Checking...' : 'I\'ve Sent It'}
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

  // Header
  header: {
    alignItems: 'center',
    marginBottom: space.lg,
  },
  iconWrap: {
    width: 44,
    height: 44,
    borderRadius: 22,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  title: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  networkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingHorizontal: space.md,
    paddingVertical: 3,
  },
  networkLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.base,
  },

  // Balance row
  balanceRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: space.lg,
    paddingHorizontal: space.sm,
  },
  balanceLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },
  balanceValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  balanceGreen: {
    color: color.success.base,
  },

  // Option cards (choose step)
  optionCard: {
    borderRadius: radius.lg,
    padding: space.lg,
    marginBottom: space.md,
    borderWidth: 1,
  },
  optionFree: {
    backgroundColor: color.success.soft,
    borderColor: color.success.base + '30',
  },
  optionPaid: {
    backgroundColor: color.bg.sunken,
    borderColor: color.border.base,
  },
  optionHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginBottom: space.xs,
  },
  optionTitle: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
    flex: 1,
  },
  optionBadge: {
    fontSize: text.xs,
    ...inter.bold,
    color: color.success.base,
    backgroundColor: color.success.base + '18',
    paddingHorizontal: space.sm,
    paddingVertical: 2,
    borderRadius: radius.sm,
    overflow: 'hidden',
  },
  optionAmount: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  optionDescRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginLeft: 26,
  },
  optionDesc: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },

  // Denial
  denialRow: {
    backgroundColor: color.warning.soft,
    borderRadius: radius.md,
    padding: space.md,
    marginBottom: space.lg,
    borderWidth: 1,
    borderColor: color.warning.border,
  },
  denialText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.warning.base,
  },

  // Amount card (self-fund step)
  amountCard: {
    padding: space.lg,
    marginBottom: space.lg,
    alignItems: 'center',
  },
  amountLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginBottom: space.xs,
  },
  amountValue: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.accent.base,
    fontFamily: font.mono,
  },

  // QR + Address
  qrWrap: {
    alignItems: 'center',
    padding: space.md,
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
    marginBottom: space.xs,
  },
  addressLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  addressText: {
    fontSize: text.xs,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    lineHeight: 18,
  },

  // Buttons
  btn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.md,
    paddingVertical: space.lg,
    borderRadius: radius.xl,
    marginBottom: space.sm,
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
  btnCheck: {
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  btnCheckText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
  cancelBtn: {
    alignItems: 'center',
    paddingVertical: space.md,
  },
  cancelText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
}));
