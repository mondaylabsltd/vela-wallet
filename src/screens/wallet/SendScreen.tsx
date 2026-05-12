import { QRScanner } from '@/components/QRScanner';
import { TokenLogo } from '@/components/TokenLogo';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenRow } from '@/components/ui/TokenRow';
import { color, text, inter, space, radius, font, shadow, motion, createStyles } from '@/constants/theme';
import { chainName, nativeSymbol } from '@/models/network';
import { type APIToken, formatBalance, isNativeToken, tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import * as Passkey from '@/modules/passkey';
import { fromHex, toHex } from '@/services/hex';
import { sendERC20, sendNative, estimateTransactionFee, formatWeiToEth, prefetchForSend } from '@/services/safe-transaction';
import { findAccountByCredentialId, saveTransaction, loadTransactions } from '@/services/storage';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { checkBundlerFunding, clearBundlerCache, fetchBundlerAccountInfo, estimateRecommendedFunding, formatWei, type FundingNeeded } from '@/services/bundler-service';
import { BundlerFundingModal } from '@/components/ui/BundlerFundingModal';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { openBrowser } from '@/services/platform';
import { getAllNetworksSync } from '@/models/network';
import { showAlert } from '@/services/platform';
import React, { useEffect, useRef, useState } from 'react';
import { FlatList, ScrollView, Text, TextInput, View, Pressable } from 'react-native';
import Animated, {
  useAnimatedStyle,
  withSpring,
  useSharedValue,
  Layout,
} from 'react-native-reanimated';
import { fadeInDown } from '@/constants/entering';
import { ArrowLeft, X, ScanLine, BookUser, CheckCircle2, AlertCircle, Loader } from 'lucide-react-native';

type Step = 'select-token' | 'enter-details' | 'confirm';
type TxStatus = 'idle' | 'preparing' | 'signing' | 'submitting' | 'confirming' | 'confirmed' | 'error';

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

function shortAddr(addr: string): string {
  if (addr.length <= 14) return addr;
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
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
  const router = useSafeRouter();
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
  const [estimatedGas, setEstimatedGas] = useState<string | null>(null);
  const [estimatingGas, setEstimatingGas] = useState(false);
  const [fundingNeeded, setFundingNeeded] = useState<FundingNeeded | null>(null);
  const [txStatus, setTxStatus] = useState<TxStatus>('idle');
  const [txHash, setTxHash] = useState<string | null>(null);
  const [txError, setTxError] = useState<string | null>(null);
  const [recentRecipients, setRecentRecipients] = useState<string[]>([]);
  const [showContacts, setShowContacts] = useState(false);

  // Prefetch account credential + webauthn module while user reviews confirm screen
  const prefetchedAccount = useRef<{ publicKeyHex: string } | null>(null);
  const webauthnModuleRef = useRef<typeof import('@/services/webauthn-verify') | null>(null);

  useEffect(() => {
    if (!address) return;
    setLoading(true);
    fetchTokens(address, {
      onProgress: (partial) => {
        const nonZero = partial.filter((t) => tokenBalanceDouble(t) > 0);
        nonZero.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
        setTokens(nonZero);
        setLoading(false); // Show tokens as soon as first chain responds
      },
    })
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
      .catch(() => showAlert('Error', 'Failed to load tokens.'))
      .finally(() => setLoading(false));

    // Load recent recipients from transaction history
    loadTransactions().then(txs => {
      const seen = new Set<string>();
      const recents: string[] = [];
      for (const tx of txs) {
        if (tx.from.toLowerCase() === address.toLowerCase() && !seen.has(tx.to.toLowerCase())) {
          seen.add(tx.to.toLowerCase());
          recents.push(tx.to);
          if (recents.length >= 10) break;
        }
      }
      setRecentRecipients(recents);
    });
  }, [address, params.preselectedSymbol, params.preselectedNetwork]);

  const handleSelectToken = (token: APIToken) => {
    setSelectedToken(token);
    setStep('enter-details');

    // Start prefetching RPC data + bundler info as soon as token is selected.
    // User will spend several seconds filling in recipient + amount — plenty of
    // time for these to complete and warm the caches.
    if (activeAccount) {
      const chainId = tokenChainId(token);
      prefetchForSend(activeAccount.address, chainId);
      fetchBundlerAccountInfo(chainId, activeAccount.address).catch(() => {});
      findAccountByCredentialId(activeAccount.id).then(s => { prefetchedAccount.current = s ?? null; });
      import('@/services/webauthn-verify').then(m => { webauthnModuleRef.current = m; });
    }
  };

  const handleContinue = async () => {
    if (!isValidAddress(recipient)) {
      showAlert('Invalid Address', 'Please enter a valid Ethereum address (0x...).');
      return;
    }
    const amountNum = parseFloat(amount);
    if (isNaN(amountNum) || amountNum <= 0) {
      showAlert('Invalid Amount', 'Please enter a valid amount greater than zero.');
      return;
    }
    if (selectedToken && amountNum > tokenBalanceDouble(selectedToken)) {
      showAlert('Insufficient Balance', 'Amount exceeds your available balance.');
      return;
    }

    // Jump to confirm screen immediately — load gas estimate in background
    if (selectedToken && activeAccount) {
      const chainId = tokenChainId(selectedToken);

      // Ensure prefetch is running (may already be cached from token selection)
      prefetchForSend(activeAccount.address, chainId);
      if (!prefetchedAccount.current) {
        findAccountByCredentialId(activeAccount.id).then(s => { prefetchedAccount.current = s ?? null; });
      }
      if (!webauthnModuleRef.current) {
        import('@/services/webauthn-verify').then(m => { webauthnModuleRef.current = m; });
      }

      // Show confirm screen right away
      setEstimatingGas(true);
      setEstimatedGas(null);
      setStep('confirm');

      // Load gas + check bundler funding in background
      (async () => {
        try {
          const [feeResult] = await Promise.allSettled([
            estimateTransactionFee(activeAccount.address, chainId),
            fetchBundlerAccountInfo(chainId, activeAccount.address),
          ]);

          const fee = feeResult.status === 'fulfilled' ? feeResult.value : null;
          setEstimatedGas(fee ? formatWeiToEth(fee.totalWei) : null);

          const funding = await checkBundlerFunding(chainId, activeAccount.address, fee?.totalWei);
          if (funding) {
            setFundingNeeded(funding);
            setStep('enter-details'); // Go back — can't proceed without funding
          }
        } catch { /* proceed */ }
        setEstimatingGas(false);
      })();
    } else {
      setStep('confirm');
    }
  };

  const handleMaxAmount = () => {
    if (!selectedToken) return;
    // Use the raw balance string directly to avoid floating-point precision loss.
    // tokenBalanceDouble() converts through parseFloat which loses precision for
    // large balances (e.g., 1234567890.123456789 → scientific notation).
    setAmount(selectedToken.balance || '0');
  };

  const handleConfirm = async () => {
    if (!selectedToken || !activeAccount) return;

    // Funding was already checked before entering the confirm screen.
    // Proceed directly to transaction execution.
    await executeTransaction();
  };

  /** Called when user taps "Send Transaction" in the funding modal after funding. */
  const handleFundingComplete = () => {
    if (selectedToken && activeAccount) {
      clearBundlerCache(tokenChainId(selectedToken), activeAccount.address);
    }
    setFundingNeeded(null);
    // Re-run handleContinue which will re-check funding → pass → enter confirm screen
    handleContinue();
  };

  const executeTransaction = async () => {
    if (!selectedToken || !activeAccount) return;
    setSending(true);
    setTxStatus('preparing');
    setTxHash(null);
    setTxError(null);
    try {
      const chainId = tokenChainId(selectedToken);
      // Use prefetched account if available, otherwise fetch now
      const stored = prefetchedAccount.current ?? await findAccountByCredentialId(activeAccount.id);
      if (!stored?.publicKeyHex) {
        throw new Error('Public key not found for this account');
      }

      const signFn = async (challenge: Uint8Array) => {
        setTxStatus('signing');
        const challengeHex = toHex(challenge);
        const assertion = await Passkey.sign(challengeHex, activeAccount.id);

        // Use prefetched module if available, otherwise dynamic import
        const webauthnMod = webauthnModuleRef.current ?? await import('@/services/webauthn-verify');
        const compat = webauthnMod.verifySafeWebAuthn(assertion);
        if (!compat.ok) {
          throw new Error(
            'Your device\'s identity provider is not compatible with Vela Wallet. ' +
            'Please switch to Google Password Manager.\n\n' + compat.reason,
          );
        }

        return {
          signature: fromHex(assertion.signatureHex),
          authenticatorData: fromHex(assertion.authenticatorDataHex),
          clientDataJSON: fromHex(assertion.clientDataJSONHex),
        };
      };

      setTxStatus('submitting');
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

      // UserOp submitted — show confirming status immediately
      setTxStatus('confirming');
      setSending(false);

      // Wait for on-chain confirmation in background
      result.waitForTxHash().then(async (hash) => {
        setTxHash(hash);
        setTxStatus('confirmed');
        clearTokenCache(activeAccount.address);

        await saveTransaction({
          id: result.userOpHash,
          userOpHash: result.userOpHash,
          txHash: hash,
          from: activeAccount.address,
          to: recipient,
          value: amount,
          symbol: selectedToken!.symbol,
          decimals: selectedToken!.decimals,
          chainId,
          timestamp: Math.floor(Date.now() / 1000),
          status: 'confirmed',
        });
      }).catch(() => {
        // Receipt polling timed out — still submitted, just can't confirm yet
        setTxError('Transaction submitted but confirmation timed out. Check history later.');
        setTxStatus('error');
      });

    } catch (error: any) {
      if (error?.code === 'PASSKEY_CANCELLED') {
        setTxStatus('idle');
      } else if (error?.message?.includes('Insufficient balance on dedicated bundler EOA')) {
        // Bundler explicitly says EOA balance is insufficient — always show funding modal.
        // Fetch account info just to get the deposit address and current balance for display.
        setTxStatus('idle');
        try {
          const chainId = tokenChainId(selectedToken!);
          clearBundlerCache(chainId, activeAccount!.address);
          const info = await fetchBundlerAccountInfo(chainId, activeAccount!.address);
          if (info) {
            const recommendedWei = await estimateRecommendedFunding(chainId);
            setFundingNeeded({
              depositAddress: info.depositAddress,
              safeAddress: activeAccount!.address,
              chainId,
              nativeSym: info.nativeSym,
              recommendedWei,
              currentBalance: info.spendableBalance,
              recommendedFormatted: formatWei(recommendedWei),
              currentFormatted: formatWei(info.spendableBalance),
            });
            return;
          }
        } catch { /* fall through */ }
        setTxError('Bundler account needs more gas. Please fund it in Settings.');
        setTxStatus('error');
      } else {
        setTxError(error?.message ?? String(error));
        setTxStatus('error');
      }
    } finally {
      setSending(false);
    }
  };

  const handleBack = () => {
    if (step === 'confirm') {
      // Don't go back while transaction is in progress
      if (txStatus !== 'idle' && txStatus !== 'confirmed' && txStatus !== 'error') return;
      setTxStatus('idle');
      setTxHash(null);
      setTxError(null);
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
    <Animated.View style={styles.stepContainer} entering={fadeInDown(0, 300)}>
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
              logoUrls={tokenLogoURLs(item)}
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
    const logos = tokenLogoURLs(selectedToken);
    const chain = chainName(tokenChainId(selectedToken));

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false} keyboardShouldPersistTaps="handled">
        <Animated.View entering={fadeInDown(0, 300)}>
          <Text style={styles.stepTitle}>Send {selectedToken.symbol}</Text>

          {/* Hero card — matches token detail design */}
          <VelaCard elevated style={styles.heroCard}>
            <View style={styles.heroRow}>
              <TokenLogo symbol={selectedToken.symbol} logoUrls={logos} size={44} />
              <View style={styles.heroIdentity}>
                <Text style={styles.heroSymbol}>{selectedToken.symbol}</Text>
                <Text style={styles.heroChain}>{chain}</Text>
              </View>
              <View style={styles.heroBalance}>
                <Text style={styles.heroAmount} adjustsFontSizeToFit numberOfLines={1}>
                  {formatBalance(balance)}
                </Text>
                {tokenUsdValue(selectedToken) > 0 && (
                  <Text style={styles.heroUsd}>
                    {formatUsd(tokenUsdValue(selectedToken))}
                  </Text>
                )}
              </View>
            </View>
          </VelaCard>

          {/* Recipient */}
          <View style={styles.fieldLabelRow}>
            <Text style={styles.fieldLabel}>Recipient</Text>
            <Pressable onPress={() => setShowScanner(true)} hitSlop={8} style={styles.fieldLabelAction}>
              <ScanLine size={15} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          </View>
          <View style={styles.inputWrap}>
            <TextInput
              style={styles.input}
              placeholder="0x... address"
              placeholderTextColor={color.fg.subtle}
              value={recipient}
              onChangeText={(t) => { setRecipient(t); setShowContacts(false); }}
              autoCapitalize="none"
              autoCorrect={false}
              multiline
            />
            {recentRecipients.length > 0 && (
              <View style={styles.inputIcons}>
                <Pressable onPress={() => setShowContacts(!showContacts)} hitSlop={6}>
                  <BookUser size={18} color={color.fg.subtle} strokeWidth={2} />
                </Pressable>
              </View>
            )}
          </View>

          {/* Recent recipients dropdown */}
          {showContacts && recentRecipients.length > 0 && (
            <VelaCard style={styles.contactsCard}>
              {recentRecipients.map((addr, i) => (
                <React.Fragment key={addr}>
                  {i > 0 && <View style={styles.contactSep} />}
                  <Pressable
                    style={styles.contactRow}
                    onPress={() => {
                      setRecipient(addr);
                      setShowContacts(false);
                    }}
                  >
                    <Text style={styles.contactAddr}>{shortAddr(addr)}</Text>
                  </Pressable>
                </React.Fragment>
              ))}
            </VelaCard>
          )}

          {/* Amount — full width, USD inside */}
          <Text style={styles.fieldLabel}>Amount</Text>
          <View style={styles.amountWrap}>
            <View style={styles.amountTopRow}>
              <TextInput
                style={styles.amountInput}
                placeholder="0"
                placeholderTextColor={color.fg.subtle}
                value={amount}
                onChangeText={setAmount}
                keyboardType="decimal-pad"
              />
              <Pressable onPress={handleMaxAmount} hitSlop={6}>
                <Text style={styles.goText}>{selectedToken.symbol}</Text>
              </Pressable>
            </View>
            {amount && selectedToken.priceUsd ? (
              <Text style={styles.amountUsd}>
                ≈ {formatUsd(parseFloat(amount || '0') * (selectedToken.priceUsd ?? 0))}
              </Text>
            ) : null}
          </View>

          {/* Available balance — right aligned */}
          <View style={styles.balanceHint}>
            <Text style={styles.balanceHintText}>
              {formatBalance(balance)} {selectedToken.symbol} available
            </Text>
            <Pressable onPress={handleMaxAmount} hitSlop={6}>
              <Text style={styles.balanceHintMax}>Max</Text>
            </Pressable>
          </View>
          {selectedToken.priceUsd != null && selectedToken.priceUsd > 0 && (
            <Text style={styles.balanceHintUsd}>
              ≈ {formatUsd(balance * selectedToken.priceUsd)}
            </Text>
          )}

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
        <Animated.View entering={fadeInDown(0, 300)}>
          <Text style={styles.stepTitle}>Confirm</Text>

          <VelaCard elevated style={styles.confirmCard}>
            <ConfirmRow label="From" value={activeAccount?.name ?? 'Wallet'} />
            <View style={styles.confirmSeparator} />
            <ConfirmRow label="To" value={shortAddr(recipient)} />
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
            <ConfirmRow
              label="Est. Fee"
              value={estimatingGas ? 'Estimating...' : estimatedGas ? `~${estimatedGas} ${nativeSymbol(tokenChainId(selectedToken))}` : 'Unable to estimate'}
            />
          </VelaCard>

          {txStatus === 'idle' && (
            <VelaButton
              title="Confirm & Send"
              onPress={handleConfirm}
              variant="accent"
              loading={sending}
              style={styles.confirmBtn}
            />
          )}

          {txStatus !== 'idle' && (
            <Animated.View entering={fadeInDown(0, 200)} style={styles.txStatusWrap}>
              {(txStatus === 'preparing' || txStatus === 'signing' || txStatus === 'submitting' || txStatus === 'confirming') && (
                <View style={styles.txStatusRow}>
                  <Animated.View style={styles.txSpinner}>
                    <Loader size={20} color={color.accent.base} strokeWidth={2.5} />
                  </Animated.View>
                  <Text style={styles.txStatusText}>
                    {txStatus === 'preparing' ? 'Preparing transaction...' :
                     txStatus === 'signing' ? 'Waiting for biometric...' :
                     txStatus === 'submitting' ? 'Submitting transaction...' :
                     'Confirming on-chain...'}
                  </Text>
                </View>
              )}
              {txStatus === 'confirmed' && (
                <View style={styles.txStatusRow}>
                  <CheckCircle2 size={20} color={color.success.base} strokeWidth={2.5} />
                  <View style={{ flex: 1 }}>
                    <Text style={styles.txStatusSuccess}>Transaction Confirmed</Text>
                    {txHash && selectedToken && (
                      <Pressable onPress={() => {
                        const chainId = tokenChainId(selectedToken);
                        const network = getAllNetworksSync().find(n => n.chainId === chainId);
                        const base = network?.explorerURL ?? 'https://etherscan.io';
                        openBrowser(`${base}/tx/${txHash}`);
                      }}>
                        <Text style={styles.txStatusHash} numberOfLines={1} ellipsizeMode="middle">
                          {txHash} ↗
                        </Text>
                      </Pressable>
                    )}
                  </View>
                </View>
              )}
              {txStatus === 'error' && (
                <View style={styles.txStatusRow}>
                  <AlertCircle size={20} color={color.error.base} strokeWidth={2.5} />
                  <Text style={styles.txStatusError}>{txError}</Text>
                </View>
              )}
              {(txStatus === 'confirmed' || txStatus === 'error') && (
                <View style={styles.txStatusActions}>
                  {txStatus === 'confirmed' && (
                    <Pressable style={styles.txDoneBtn} onPress={() => router.back()}>
                      <Text style={styles.txDoneBtnText}>Done</Text>
                    </Pressable>
                  )}
                  {txStatus === 'error' && (
                    <Pressable
                      style={styles.txRetryBtn}
                      onPress={() => { setTxStatus('idle'); setTxError(null); }}
                    >
                      <Text style={styles.txRetryBtnText}>Try Again</Text>
                    </Pressable>
                  )}
                </View>
              )}
            </Animated.View>
          )}
        </Animated.View>
      </ScrollView>
    );
  };

  return (
    <ScreenContainer>
      {/* Nav bar */}
      <View style={styles.navBar}>
        <Pressable onPress={handleBack} hitSlop={8} style={styles.navBtn}>
          {step === 'select-token'
            ? <X size={22} color={color.fg.base} strokeWidth={2} />
            : <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          }
        </Pressable>
        <StepIndicator current={step} />
        <View style={styles.navSpacer} />
      </View>

      {step === 'select-token' && renderSelectToken()}
      {step === 'enter-details' && renderEnterDetails()}
      {step === 'confirm' && renderConfirm()}

      <QRScanner
        visible={showScanner}
        onScan={(addr) => {
          setRecipient(addr);
          setShowScanner(false);
        }}
        onClose={() => setShowScanner(false)}
      />

      {fundingNeeded && (
        <BundlerFundingModal
          visible={!!fundingNeeded}
          funding={fundingNeeded}
          onFunded={handleFundingComplete}
          onCancel={() => setFundingNeeded(null)}
        />
      )}
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
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
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
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space['2xl'],
  },
  loadingText: {
    fontSize: text.lg,
    ...inter.regular,
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
    ...inter.semibold,
    color: color.fg.muted,
  },

  // Hero card (matches TokenDetailScreen)
  heroCard: {
    padding: space['2xl'],
    marginBottom: space['3xl'],
  },
  heroRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  heroIdentity: {
    flex: 1,
    gap: 2,
  },
  heroSymbol: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
  },
  heroChain: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  heroBalance: {
    alignItems: 'flex-end',
    gap: 2,
  },
  heroAmount: {
    fontSize: text.xl,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  heroUsd: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },

  // Form fields — full width, icons inside
  fieldLabelRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: space.md,
  },
  fieldLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },
  fieldLabelAction: {
    padding: space.xs,
  },
  inputWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    marginBottom: space.sm,
  },
  input: {
    flex: 1,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    maxHeight: 100,
  },
  inputIcons: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingRight: space.lg,
  },
  goText: {
    fontSize: text.sm,
    ...inter.bold,
    color: color.accent.base,
    letterSpacing: 0.3,
  },

  // Balance hint
  balanceHint: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'flex-end',
    gap: space.sm,
    paddingRight: space.xs,
    marginBottom: space.xs,
  },
  balanceHintText: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },
  balanceHintMax: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },
  balanceHintUsd: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'right',
    paddingRight: space.xs,
    marginBottom: space['2xl'],
  },

  // Recent contacts
  contactsCard: {
    marginBottom: space.lg,
    padding: space.sm,
  },
  contactRow: {
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
  },
  contactAddr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  contactSep: {
    height: 1,
    backgroundColor: color.border.base,
    marginHorizontal: space.lg,
  },

  amountWrap: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    marginBottom: space.sm,
  },
  amountTopRow: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  amountInput: {
    flex: 1,
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    padding: 0,
  },
  amountUsd: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
    marginTop: space.xs,
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
    ...inter.regular,
    color: color.fg.muted,
  },
  confirmValue: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
    maxWidth: '60%',
    textAlign: 'right',
  },
  confirmValueHighlight: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.accent.base,
  },
  confirmBtn: {
    marginTop: space.md,
  },

  // Inline tx status
  txStatusWrap: {
    marginTop: space.xl,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    padding: space.xl,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  txStatusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  txSpinner: {
    width: 20,
    height: 20,
  },
  txStatusText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.muted,
    flex: 1,
  },
  txStatusSuccess: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
  txStatusHash: {
    fontSize: text.xs,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.accent.base,
    marginTop: 2,
    textDecorationLine: 'underline',
  },
  txStatusError: {
    fontSize: text.base,
    ...inter.medium,
    color: color.error.base,
    flex: 1,
  },
  txStatusActions: {
    marginTop: space.xl,
  },
  txDoneBtn: {
    backgroundColor: color.accent.base,
    borderRadius: radius.xl,
    paddingVertical: space.lg,
    alignItems: 'center',
  },
  txDoneBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.inverse,
  },
  txRetryBtn: {
    backgroundColor: color.bg.base,
    borderRadius: radius.xl,
    paddingVertical: space.lg,
    alignItems: 'center',
    borderWidth: 1,
    borderColor: color.border.base,
  },
  txRetryBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
}));
