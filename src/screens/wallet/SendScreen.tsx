import { QRScanner } from '@/components/QRScanner';
import { TokenLogo } from '@/components/TokenLogo';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenRow } from '@/components/ui/TokenRow';
import { color, text, weight, space, radius, shadow, motion, createStyles } from '@/constants/theme';
import { chainName } from '@/models/network';
import { type APIToken, formatBalance, isNativeToken, tokenBalanceDouble, tokenChainId, tokenLogoURL, tokenUsdValue } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import * as Passkey from '@/modules/passkey';
import { fromHex, toHex } from '@/services/hex';
import { sendERC20, sendNative } from '@/services/safe-transaction';
import { findAccountByCredentialId } from '@/services/storage';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { useLocalSearchParams, useRouter } from 'expo-router';
import React, { useEffect, useState } from 'react';
import { Alert, FlatList, ScrollView, Text, TextInput, TouchableOpacity, View, Pressable } from 'react-native';
import Animated, {
  useAnimatedStyle,
  withSpring,
  useSharedValue,
  FadeInDown,
  Layout,
} from 'react-native-reanimated';

type Step = 'select-token' | 'enter-details' | 'confirm';

function isValidAddress(addr: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(addr);
}

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function amountToWeiHex(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const intPart = parts[0] || '0';
  let fracPart = parts[1] || '';
  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, '0');
  }
  const weiStr = (intPart + fracPart).replace(/^0+/, '') || '0';
  let n = BigInt(weiStr);
  return n.toString(16);
}

// Animated step indicator
function StepIndicator({ current }: { current: Step }) {
  const steps: Step[] = ['select-token', 'enter-details', 'confirm'];
  const currentIndex = steps.indexOf(current);

  return (
    <View style={styles.stepRow}>
      {steps.map((s, i) => {
        const isActive = i <= currentIndex;
        return (
          <View key={s} style={styles.stepDotOuter}>
            <Animated.View
              layout={Layout.springify()}
              style={[
                styles.stepDot,
                isActive && styles.stepDotActive,
                i === currentIndex && styles.stepDotCurrent,
              ]}
            />
          </View>
        );
      })}
    </View>
  );
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
      const stored = await findAccountByCredentialId(activeAccount.id);
      if (!stored?.publicKeyHex) {
        throw new Error('Public key not found for this account');
      }

      const signFn = async (challenge: Uint8Array) => {
        const challengeHex = toHex(challenge);
        const assertion = await Passkey.sign(challengeHex, activeAccount.id);

        const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
        const compat = verifySafeWebAuthn(assertion);
        if (!compat.ok) {
          throw new Error(
            'Your passkey provider is not compatible with Vela Wallet. ' +
            'Please switch to Google Password Manager.\n\n' + compat.reason,
          );
        }

        return {
          signature: fromHex(assertion.signatureHex),
          authenticatorData: fromHex(assertion.authenticatorDataHex),
          clientDataJSON: fromHex(assertion.clientDataJSONHex),
        };
      };

      const weiHex = amountToWeiHex(amount, selectedToken.decimals);

      let result;
      if (isNativeToken(selectedToken)) {
        result = await sendNative(
          activeAccount.address, recipient, weiHex, chainId,
          stored.publicKeyHex, signFn,
        );
      } else {
        result = await sendERC20(
          activeAccount.address, selectedToken.tokenAddress!, recipient, weiHex, chainId,
          stored.publicKeyHex, signFn,
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
        // User cancelled biometric
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

  // Step 1: Select Token
  const renderSelectToken = () => (
    <Animated.View style={styles.stepContainer} entering={FadeInDown.duration(300)}>
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
          renderItem={({ item, index }) => (
            <TokenRow
              symbol={item.symbol}
              chainLabel={chainName(tokenChainId(item))}
              logoUrl={tokenLogoURL(item)}
              balance={formatBalance(tokenBalanceDouble(item))}
              usdValue={tokenUsdValue(item) > 0 ? formatUsd(tokenUsdValue(item)) : undefined}
              onPress={() => handleSelectToken(item)}
              index={index}
            />
          )}
          showsVerticalScrollIndicator={false}
        />
      )}
    </Animated.View>
  );

  // Step 2: Enter Details
  const renderEnterDetails = () => {
    if (!selectedToken) return null;
    const balance = tokenBalanceDouble(selectedToken);
    const logo = tokenLogoURL(selectedToken);

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false}>
        <Animated.View entering={FadeInDown.duration(300)}>
          <Text style={styles.stepTitle}>Send {selectedToken.symbol}</Text>

          {/* Selected token info */}
          <VelaCard style={styles.selectedCard}>
            <View style={styles.selectedRow}>
              <TokenLogo symbol={selectedToken.symbol} logoUrl={logo} size={40} />
              <View style={styles.selectedInfo}>
                <Text style={styles.selectedName}>{selectedToken.name}</Text>
                <Text style={styles.selectedBalance}>
                  {formatBalance(balance)} {selectedToken.symbol}
                </Text>
              </View>
            </View>
          </VelaCard>

          {/* Recipient */}
          <Text style={styles.fieldLabel}>Recipient</Text>
          <View style={styles.recipientRow}>
            <TextInput
              style={[styles.input, styles.recipientInput]}
              placeholder="0x..."
              placeholderTextColor={color.fg.subtle}
              value={recipient}
              onChangeText={setRecipient}
              autoCapitalize="none"
              autoCorrect={false}
            />
            <Pressable
              style={styles.scanButton}
              onPress={() => setShowScanner(true)}
            >
              <Text style={styles.scanText}>Scan</Text>
            </Pressable>
          </View>

          {/* Amount */}
          <Text style={styles.fieldLabel}>Amount</Text>
          <View style={styles.amountRow}>
            <TextInput
              style={[styles.input, styles.amountInput]}
              placeholder="0.00"
              placeholderTextColor={color.fg.subtle}
              value={amount}
              onChangeText={setAmount}
              keyboardType="decimal-pad"
            />
            <Pressable style={styles.maxButton} onPress={handleMaxAmount}>
              <Text style={styles.maxText}>MAX</Text>
            </Pressable>
          </View>

          {/* USD preview */}
          {amount && selectedToken.priceUsd ? (
            <Text style={styles.usdPreview}>
              ≈ {formatUsd(parseFloat(amount || '0') * (selectedToken.priceUsd ?? 0))}
            </Text>
          ) : null}

          <VelaButton
            title="Continue"
            onPress={handleContinue}
            style={styles.continueBtn}
            disabled={!recipient || !amount}
          />
        </Animated.View>
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
        <Animated.View entering={FadeInDown.duration(300)}>
          <Text style={styles.stepTitle}>Confirm</Text>

          <VelaCard elevated style={styles.confirmCard}>
            <ConfirmRow label="From" value={activeAccount?.name ?? 'Wallet'} />
            <View style={styles.confirmSeparator} />
            <ConfirmRow label="To" value={`${recipient.slice(0, 10)}...${recipient.slice(-8)}`} />
            <View style={styles.confirmSeparator} />
            <ConfirmRow
              label="Amount"
              value={`${formatBalance(amountNum)} ${selectedToken.symbol}`}
              highlight
            />
            {usdAmount > 0 && (
              <>
                <View style={styles.confirmSeparator} />
                <ConfirmRow label="Value" value={formatUsd(usdAmount)} />
              </>
            )}
            <View style={styles.confirmSeparator} />
            <ConfirmRow label="Network" value={chainName(tokenChainId(selectedToken))} />
            <View style={styles.confirmSeparator} />
            <ConfirmRow label="Gas" value="Sponsored" />
          </VelaCard>

          <VelaButton
            title="Confirm & Send"
            onPress={handleConfirm}
            variant="accent"
            loading={sending}
            style={styles.confirmBtn}
          />
        </Animated.View>
      </ScrollView>
    );
  };

  return (
    <ScreenContainer>
      {/* Nav bar */}
      <View style={styles.navBar}>
        <Pressable onPress={handleBack} hitSlop={8}>
          <Text style={styles.navBack}>
            {step === 'select-token' ? 'Cancel' : 'Back'}
          </Text>
        </Pressable>
        <StepIndicator current={step} />
        <View style={styles.navSpacer} />
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

function ConfirmRow({ label, value, highlight }: { label: string; value: string; highlight?: boolean }) {
  return (
    <View style={styles.confirmRow}>
      <Text style={styles.confirmLabel}>{label}</Text>
      <Text style={[styles.confirmValue, highlight && styles.confirmValueHighlight]} numberOfLines={1}>
        {value}
      </Text>
    </View>
  );
}

const styles = createStyles(() => ({
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
  },
  navBack: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.accent.base,
    minWidth: 60,
  },
  navSpacer: { minWidth: 60 },

  // Step indicator
  stepRow: {
    flexDirection: 'row',
    gap: space.md,
    alignItems: 'center',
  },
  stepDotOuter: {
    padding: 2,
  },
  stepDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: color.border.base,
  },
  stepDotActive: {
    backgroundColor: color.accent.base,
  },
  stepDotCurrent: {
    width: 20,
    borderRadius: 10,
  },

  // Step content
  stepContainer: {
    flex: 1,
  },
  stepTitle: {
    fontSize: text['3xl'],
    fontWeight: weight.bold,
    color: color.fg.base,
    marginBottom: space['2xl'],
  },
  loadingText: {
    fontSize: text.lg,
    fontWeight: weight.regular,
    color: color.fg.muted,
    textAlign: 'center',
    marginTop: space['5xl'],
  },
  emptyContainer: {
    alignItems: 'center',
    marginTop: space['5xl'],
  },
  emptyText: {
    fontSize: text.xl,
    fontWeight: weight.semibold,
    color: color.fg.muted,
  },

  // Selected token
  selectedCard: {
    padding: space['2xl'],
    marginBottom: space['3xl'],
  },
  selectedRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  selectedInfo: {
    flex: 1,
    gap: 2,
  },
  selectedName: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  selectedBalance: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
  },

  // Form fields
  fieldLabel: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
    marginBottom: space.md,
  },
  recipientRow: {
    flexDirection: 'row',
    alignItems: 'stretch',
    gap: space.md,
    marginBottom: space['2xl'],
  },
  recipientInput: {
    flex: 1,
  },
  scanButton: {
    backgroundColor: color.accent.soft,
    paddingHorizontal: space.xl,
    borderRadius: radius.lg,
    alignItems: 'center',
    justifyContent: 'center',
  },
  scanText: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.accent.base,
  },
  input: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingHorizontal: space.xl,
    paddingVertical: space.xl,
    fontSize: text.lg,
    fontWeight: weight.regular,
    color: color.fg.base,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  amountRow: {
    flexDirection: 'row',
    alignItems: 'stretch',
    gap: space.md,
    marginBottom: space.lg,
  },
  amountInput: {
    flex: 1,
  },
  maxButton: {
    backgroundColor: color.accent.soft,
    paddingHorizontal: space.xl,
    borderRadius: radius.lg,
    alignItems: 'center',
    justifyContent: 'center',
  },
  maxText: {
    fontSize: text.sm,
    fontWeight: weight.bold,
    color: color.accent.base,
    letterSpacing: 0.5,
  },
  usdPreview: {
    fontSize: text.base,
    fontWeight: weight.medium,
    color: color.fg.muted,
    marginBottom: space['2xl'],
    paddingLeft: space.sm,
  },
  continueBtn: {
    marginTop: space.lg,
  },

  // Confirm
  confirmCard: {
    padding: space['2xl'],
    marginBottom: space['3xl'],
  },
  confirmRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  confirmSeparator: {
    height: 1,
    backgroundColor: color.border.base,
  },
  confirmLabel: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
  },
  confirmValue: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
    maxWidth: '60%',
    textAlign: 'right',
  },
  confirmValueHighlight: {
    fontSize: text.lg,
    fontWeight: weight.bold,
    color: color.accent.base,
  },
  confirmBtn: {
    marginTop: space.md,
  },
}));
