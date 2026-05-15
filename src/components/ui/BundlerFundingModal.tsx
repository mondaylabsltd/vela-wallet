/**
 * Modal prompting the user to fund their bundler EOA.
 *
 * Shown before a transaction when the built-in bundler's dedicated EOA
 * has insufficient balance to relay. Displays deposit address + QR code
 * and auto-refreshes until funding is detected.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { copyToClipboard, hapticSuccess, hapticLight } from '@/services/platform';
import { Check, Copy, RefreshCw } from 'lucide-react-native';

import { AppModal } from './AppModal';
import { chainName } from '@/models/network';
import { QRCode } from '@/components/QRCode';
import { VelaCard } from './VelaCard';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { fetchBundlerAccountInfo, clearBundlerCache, type FundingNeeded } from '@/services/bundler-service';

interface Props {
  visible: boolean;
  funding: FundingNeeded;
  onFunded: () => void;
  onCancel: () => void;
}

const POLL_INTERVAL = 10_000; // 10s

export function BundlerFundingModal({ visible, funding, onFunded, onCancel }: Props) {
  const [copied, setCopied] = useState(false);
  const [checking, setChecking] = useState(false);
  const [currentBalance, setCurrentBalance] = useState(funding.currentFormatted);
  const [funded, setFunded] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Use the actual threshold from the funding check — not a hardcoded constant.
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

  // Auto-poll for balance changes
  useEffect(() => {
    if (!visible) return;
    pollRef.current = setInterval(checkBalance, POLL_INTERVAL);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [visible, checkBalance]);

  const copyAddress = async () => {
    await copyToClipboard(funding.depositAddress);
    setCopied(true);
    hapticLight();
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <AppModal visible={visible} onClose={onCancel}>
      <View style={styles.container}>
        {/* Header */}
        <View style={styles.header}>
          <Text style={styles.title}>Activate Gas Relayer</Text>
          <Text style={styles.networkLabel}>{chainName(funding.chainId)}</Text>
        </View>

        {/* QR Code */}
        <View style={styles.qrWrap}>
          <QRCode value={funding.depositAddress} size={140} />
        </View>

        {/* Deposit address */}
        <Pressable style={styles.addressCard} onPress={copyAddress}>
          <View style={styles.addressRow}>
            <Text style={styles.addressLabel}>Deposit Address</Text>
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

        {/* Amount info */}
        <VelaCard style={styles.infoCard}>
          <View style={styles.infoRow}>
            <Text style={styles.infoLabel}>Deposit</Text>
            <Text style={styles.infoValue}>
              {funding.recommendedFormatted} {funding.nativeSym}
            </Text>
          </View>
          <View style={styles.divider} />
          <View style={styles.infoRow}>
            <Text style={styles.infoLabel}>Balance</Text>
            <Text style={[styles.infoValue, funded && styles.infoValueGreen]}>
              {currentBalance} {funding.nativeSym}
            </Text>
          </View>
        </VelaCard>

        {/* Note */}
        <Text style={styles.note}>
          Non-refundable deposit to activate the gas relayer. Relayer address may change with service upgrades.
        </Text>

        {/* Actions */}
        <View style={styles.actions}>
          {funded ? (
            <Pressable style={[styles.btn, styles.btnPrimary]} onPress={onFunded}>
              <Check size={18} color={color.fg.inverse} strokeWidth={2.5} />
              <Text style={styles.btnPrimaryText}>Send Transaction</Text>
            </Pressable>
          ) : (
            <>
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
        </View>
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
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.xs,
  },
  networkLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },

  qrWrap: {
    alignItems: 'center',
    padding: space.lg,
    backgroundColor: '#FFFFFF',
    borderRadius: radius.xl,
    alignSelf: 'center',
    marginBottom: space.lg,
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

  infoCard: {
    padding: space.lg,
    marginBottom: space.md,
  },
  infoRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.sm,
  },
  infoLabel: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
  },
  infoValue: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
  infoValueGreen: {
    color: color.success.base,
  },
  divider: {
    height: 1,
    backgroundColor: color.border.base,
    marginVertical: space.xs,
  },

  note: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
    lineHeight: 20,
    marginBottom: space.lg,
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
  cancelBtn: {
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  cancelText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.subtle,
  },
}));
