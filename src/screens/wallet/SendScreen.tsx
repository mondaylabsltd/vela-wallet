import React, { useState, useEffect } from 'react';
import { View, Text, TextInput, FlatList, TouchableOpacity, StyleSheet, Alert, ScrollView } from 'react-native';
import { useRouter, useLocalSearchParams } from 'expo-router';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { QRScanner } from '@/components/QRScanner';
import { VelaColor, VelaFont, VelaRadius, VelaSpacing } from '@/constants/theme';
import { useWallet } from '@/models/wallet-state';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { type APIToken, tokenBalanceDouble, tokenUsdValue, tokenLogoURL, formatBalance, tokenChainId, isNativeToken } from '@/models/types';
import { chainName } from '@/models/network';
import { sendNative, sendERC20 } from '@/services/safe-transaction';
import { findAccountByCredentialId } from '@/services/storage';
import * as Passkey from '@/modules/passkey';
import { derSignatureToRaw } from '@/services/attestation-parser';
import { keccak256 } from '@/services/eth-crypto';
import { fromHex, toHex, stripHexPrefix } from '@/services/hex';

type Step = 'select-token' | 'enter-details' | 'confirm';

function isValidAddress(addr: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(addr);
}

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/** Convert a decimal amount string + decimals to a hex wei string (no 0x prefix).
 *  Uses string math to avoid floating-point precision loss. */
function amountToWeiHex(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const intPart = parts[0] || '0';
  let fracPart = parts[1] || '';
  // Pad or truncate fractional part to `decimals` digits
  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, '0');
  }
  // Combine and remove leading zeros
  const weiStr = (intPart + fracPart).replace(/^0+/, '') || '0';
  // Convert decimal string to hex
  let n = BigInt(weiStr);
  return n.toString(16);
}

export default function SendScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{ preselectedSymbol?: string; preselectedNetwork?: string }>();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;

  const [step, setStep] = useState<Step>('select-token');
  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedToken, setSelectedToken] = useState<APIToken | null>(null);
  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [sending, setSending] = useState(false);
  const [showScanner, setShowScanner] = useState(false);

  useEffect(() => {
    if (!address) return;
    setLoading(true);
    fetchTokens(address)
      .then((result) => {
        const nonZero = result.filter((t) => tokenBalanceDouble(t) > 0);
        nonZero.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
        setTokens(nonZero);

        // If a token was preselected via route params, auto-select it
        if (params.preselectedSymbol && params.preselectedNetwork) {
          const match = nonZero.find(
            (t) => t.symbol === params.preselectedSymbol && t.network === params.preselectedNetwork
          );
          if (match) {
            setSelectedToken(match);
            setStep('enter-details');
          }
        }
      })
      .catch(() => Alert.alert('Error', 'Failed to load tokens.'))
      .finally(() => setLoading(false));
  }, [address, params.preselectedSymbol, params.preselectedNetwork]);

  const handleSelectToken = (token: APIToken) => {
    setSelectedToken(token);
    setStep('enter-details');
  };

  const handleContinue = () => {
    if (!isValidAddress(recipient)) {
      Alert.alert('Invalid Address', 'Please enter a valid Ethereum address (0x...).');
      return;
    }
    const amountNum = parseFloat(amount);
    if (isNaN(amountNum) || amountNum <= 0) {
      Alert.alert('Invalid Amount', 'Please enter a valid amount greater than zero.');
      return;
    }
    if (selectedToken && amountNum > tokenBalanceDouble(selectedToken)) {
      Alert.alert('Insufficient Balance', 'Amount exceeds your available balance.');
      return;
    }
    setStep('confirm');
  };

  const handleMaxAmount = () => {
    if (!selectedToken) return;
    setAmount(String(tokenBalanceDouble(selectedToken)));
  };

  const handleConfirm = async () => {
    if (!selectedToken || !activeAccount) return;
    setSending(true);
    try {
      const chainId = tokenChainId(selectedToken);

      // Get public key for this account
      const stored = await findAccountByCredentialId(activeAccount.id);
      if (!stored?.publicKeyHex) {
        throw new Error('Public key not found for this account');
      }

      // Build signFn that calls Passkey.sign
      const signFn = async (challenge: Uint8Array) => {
        const challengeHex = toHex(challenge);
        const assertion = await Passkey.sign(challengeHex, activeAccount.id);
        return {
          signature: fromHex(assertion.signatureHex),
          authenticatorData: fromHex(assertion.authenticatorDataHex),
          clientDataJSON: fromHex(assertion.clientDataJSONHex),
        };
      };

      // Convert amount to wei hex
      const weiHex = amountToWeiHex(amount, selectedToken.decimals);

      let result;
      if (isNativeToken(selectedToken)) {
        result = await sendNative(
          activeAccount.address,
          recipient,
          weiHex,
          chainId,
          stored.publicKeyHex,
          signFn,
        );
      } else {
        result = await sendERC20(
          activeAccount.address,
          selectedToken.tokenAddress!,
          recipient,
          weiHex,
          chainId,
          stored.publicKeyHex,
          signFn,
        );
      }

      clearTokenCache(activeAccount.address);

      Alert.alert(
        'Transaction Confirmed',
        `Transaction hash:\n${result.txHash.slice(0, 16)}...`,
        [{ text: 'OK', onPress: () => router.back() }],
      );
    } catch (error: any) {
      if (error?.code === 'PASSKEY_CANCELLED') {
        // User cancelled biometric — do nothing
      } else {
        Alert.alert('Transaction Failed', error?.message ?? String(error));
      }
    } finally {
      setSending(false);
    }
  };

  const handleBack = () => {
    if (step === 'confirm') {
      setStep('enter-details');
    } else if (step === 'enter-details') {
      setSelectedToken(null);
      setAmount('');
      setRecipient('');
      setStep('select-token');
    } else {
      router.back();
    }
  };

  const renderStepIndicator = () => {
    const steps: Step[] = ['select-token', 'enter-details', 'confirm'];
    const currentIndex = steps.indexOf(step);
    return (
      <View style={styles.stepRow}>
        {steps.map((s, i) => (
          <View
            key={s}
            style={[styles.stepDot, i <= currentIndex && styles.stepDotActive]}
          />
        ))}
      </View>
    );
  };

  // Step 1: Select Token
  const renderSelectToken = () => (
    <View style={styles.stepContainer}>
      <Text style={styles.stepTitle}>Select Token</Text>
      {loading ? (
        <Text style={styles.loadingText}>Loading tokens...</Text>
      ) : tokens.length === 0 ? (
        <View style={styles.emptyContainer}>
          <Text style={styles.emptyText}>No tokens with balance</Text>
        </View>
      ) : (
        <FlatList
          data={tokens}
          keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
          renderItem={({ item }) => {
            const balance = tokenBalanceDouble(item);
            const usd = tokenUsdValue(item);
            const logo = tokenLogoURL(item);
            const chain = chainName(tokenChainId(item));
            return (
              <TouchableOpacity
                style={styles.tokenRow}
                onPress={() => handleSelectToken(item)}
                activeOpacity={0.7}
              >
                <TokenLogo symbol={item.symbol} logoUrl={logo} size={40} />
                <View style={styles.tokenInfo}>
                  <Text style={styles.tokenName} numberOfLines={1}>{item.name || item.symbol}</Text>
                  <Text style={styles.tokenChain}>{chain}</Text>
                </View>
                <View style={styles.tokenValues}>
                  <Text style={styles.tokenBalance}>{formatBalance(balance)} {item.symbol}</Text>
                  {usd > 0 && <Text style={styles.tokenUsd}>{formatUsd(usd)}</Text>}
                </View>
              </TouchableOpacity>
            );
          }}
          showsVerticalScrollIndicator={false}
        />
      )}
    </View>
  );

  // Step 2: Enter Details
  const renderEnterDetails = () => {
    if (!selectedToken) return null;
    const balance = tokenBalanceDouble(selectedToken);
    const logo = tokenLogoURL(selectedToken);

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false}>
        <Text style={styles.stepTitle}>Send {selectedToken.symbol}</Text>

        {/* Selected token info */}
        <VelaCard style={styles.selectedCard}>
          <View style={styles.selectedRow}>
            <TokenLogo symbol={selectedToken.symbol} logoUrl={logo} size={36} />
            <View style={{ flex: 1 }}>
              <Text style={styles.selectedName}>{selectedToken.name}</Text>
              <Text style={styles.selectedBalance}>
                Balance: {formatBalance(balance)} {selectedToken.symbol}
              </Text>
            </View>
          </View>
        </VelaCard>

        {/* Recipient */}
        <Text style={styles.fieldLabel}>Recipient Address</Text>
        <View style={styles.recipientRow}>
          <TextInput
            style={[styles.input, styles.recipientInput]}
            placeholder="0x..."
            placeholderTextColor={VelaColor.textTertiary}
            value={recipient}
            onChangeText={setRecipient}
            autoCapitalize="none"
            autoCorrect={false}
          />
          <TouchableOpacity
            style={styles.scanButton}
            onPress={() => setShowScanner(true)}
            activeOpacity={0.7}
          >
            <Text style={styles.scanText}>Scan QR</Text>
          </TouchableOpacity>
        </View>

        {/* Amount */}
        <Text style={styles.fieldLabel}>Amount</Text>
        <View style={styles.amountRow}>
          <TextInput
            style={[styles.input, styles.amountInput]}
            placeholder="0.00"
            placeholderTextColor={VelaColor.textTertiary}
            value={amount}
            onChangeText={setAmount}
            keyboardType="decimal-pad"
          />
          <TouchableOpacity style={styles.maxButton} onPress={handleMaxAmount} activeOpacity={0.7}>
            <Text style={styles.maxText}>MAX</Text>
          </TouchableOpacity>
        </View>

        {/* Amount in USD */}
        {amount && selectedToken.priceUsd ? (
          <Text style={styles.usdPreview}>
            {formatUsd(parseFloat(amount || '0') * (selectedToken.priceUsd ?? 0))}
          </Text>
        ) : null}

        <VelaButton
          title="Continue"
          onPress={handleContinue}
          style={styles.continueBtn}
          disabled={!recipient || !amount}
        />
      </ScrollView>
    );
  };

  // Step 3: Confirm
  const renderConfirm = () => {
    if (!selectedToken) return null;
    const amountNum = parseFloat(amount || '0');
    const usdAmount = amountNum * (selectedToken.priceUsd ?? 0);

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false}>
        <Text style={styles.stepTitle}>Confirm Transaction</Text>

        <VelaCard style={styles.confirmCard}>
          <ConfirmRow label="From" value={`${activeAccount?.name ?? 'Wallet'}`} />
          <ConfirmRow label="To" value={`${recipient.slice(0, 10)}...${recipient.slice(-8)}`} />
          <ConfirmRow
            label="Amount"
            value={`${formatBalance(amountNum)} ${selectedToken.symbol}`}
          />
          {usdAmount > 0 && <ConfirmRow label="Value" value={formatUsd(usdAmount)} />}
          <ConfirmRow label="Network" value={chainName(tokenChainId(selectedToken))} />
          <ConfirmRow label="Gas" value="Estimated by network" />
        </VelaCard>

        <VelaButton
          title="Confirm & Send"
          onPress={handleConfirm}
          variant="accent"
          loading={sending}
          style={styles.confirmBtn}
        />
      </ScrollView>
    );
  };

  return (
    <ScreenContainer>
      {/* Nav bar */}
      <View style={styles.navBar}>
        <TouchableOpacity onPress={handleBack} activeOpacity={0.7}>
          <Text style={styles.navBack}>
            {step === 'select-token' ? 'Cancel' : 'Back'}
          </Text>
        </TouchableOpacity>
        {renderStepIndicator()}
        <View style={{ width: 60 }} />
      </View>

      {step === 'select-token' && renderSelectToken()}
      {step === 'enter-details' && renderEnterDetails()}
      {step === 'confirm' && renderConfirm()}

      <QRScanner
        visible={showScanner}
        onScan={(address) => {
          setRecipient(address);
          setShowScanner(false);
        }}
        onClose={() => setShowScanner(false)}
      />
    </ScreenContainer>
  );
}

function ConfirmRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.confirmRow}>
      <Text style={styles.confirmLabel}>{label}</Text>
      <Text style={styles.confirmValue} numberOfLines={1}>{value}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: 12,
  },
  navBack: {
    ...VelaFont.title(16),
    color: VelaColor.accent,
    width: 60,
  },
  stepRow: {
    flexDirection: 'row',
    gap: 6,
  },
  stepDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: VelaColor.border,
  },
  stepDotActive: {
    backgroundColor: VelaColor.accent,
  },
  stepContainer: {
    flex: 1,
  },
  stepTitle: {
    ...VelaFont.heading(24),
    color: VelaColor.textPrimary,
    marginBottom: 20,
  },
  loadingText: {
    ...VelaFont.body(15),
    color: VelaColor.textSecondary,
    textAlign: 'center',
    marginTop: 40,
  },
  emptyContainer: {
    alignItems: 'center',
    marginTop: 60,
  },
  emptyText: {
    ...VelaFont.title(17),
    color: VelaColor.textSecondary,
  },
  tokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: VelaSpacing.itemGap,
    gap: 12,
  },
  tokenInfo: {
    flex: 1,
    gap: 2,
  },
  tokenName: {
    ...VelaFont.title(15),
    color: VelaColor.textPrimary,
  },
  tokenChain: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
  },
  tokenValues: {
    alignItems: 'flex-end',
    gap: 2,
  },
  tokenBalance: {
    ...VelaFont.title(15),
    color: VelaColor.textPrimary,
  },
  tokenUsd: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
  },
  selectedCard: {
    padding: VelaSpacing.cardPadding,
    marginBottom: 24,
  },
  selectedRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
  },
  selectedName: {
    ...VelaFont.title(16),
    color: VelaColor.textPrimary,
  },
  selectedBalance: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
    marginTop: 2,
  },
  fieldLabel: {
    ...VelaFont.label(13),
    color: VelaColor.textSecondary,
    marginBottom: 8,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  recipientRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  recipientInput: {
    flex: 1,
  },
  scanButton: {
    backgroundColor: VelaColor.accentSoft,
    paddingHorizontal: 16,
    paddingVertical: 14,
    borderRadius: VelaRadius.cardSmall,
    marginBottom: 20,
  },
  scanText: {
    ...VelaFont.label(14),
    color: VelaColor.accent,
  },
  input: {
    backgroundColor: VelaColor.bgWarm,
    borderRadius: VelaRadius.cardSmall,
    paddingHorizontal: 16,
    paddingVertical: 14,
    ...VelaFont.body(16),
    color: VelaColor.textPrimary,
    marginBottom: 20,
  },
  amountRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  amountInput: {
    flex: 1,
  },
  maxButton: {
    backgroundColor: VelaColor.accentSoft,
    paddingHorizontal: 16,
    paddingVertical: 14,
    borderRadius: VelaRadius.cardSmall,
    marginBottom: 20,
  },
  maxText: {
    ...VelaFont.label(14),
    color: VelaColor.accent,
  },
  usdPreview: {
    ...VelaFont.body(14),
    color: VelaColor.textSecondary,
    marginTop: -12,
    marginBottom: 20,
    paddingLeft: 4,
  },
  continueBtn: {
    marginTop: 12,
  },
  confirmCard: {
    padding: VelaSpacing.cardPadding,
    marginBottom: 24,
    gap: 16,
  },
  confirmRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
  },
  confirmLabel: {
    ...VelaFont.body(14),
    color: VelaColor.textSecondary,
  },
  confirmValue: {
    ...VelaFont.title(14),
    color: VelaColor.textPrimary,
    maxWidth: '60%',
    textAlign: 'right',
  },
  confirmBtn: {
    marginTop: 8,
  },
});
