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
import { sendERC20, sendNative, estimateTransactionFee, formatWeiToEth, prefetchForSend, refreshGasPrice, GAS_TIER_MULTIPLIERS, type TransactionFeeEstimate, type GasTier } from '@/services/safe-transaction';
import { findAccountByCredentialId, saveTransaction, loadTransactions } from '@/services/storage';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { checkBundlerFunding, clearBundlerCache, fetchBundlerAccountInfo, formatWei, type FundingNeeded } from '@/services/bundler-service';
import { AmountText } from '@/components/ui/AmountText';
import { formatTokenAmount } from '@/services/locale-format';
import { BundlerFundingModal } from '@/components/ui/BundlerFundingModal';
import { TransactionReceipt } from '@/components/ui/TransactionReceipt';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { ZERO_DECIMAL_CODES } from '@/services/currency';
import { showAlert } from '@/services/platform';
import { useTranslation } from 'react-i18next';
import React, { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, ScrollView, Text, TextInput, View, Pressable } from 'react-native';
import Animated, {
  useAnimatedStyle,
  withSpring,
  useSharedValue,
  Layout,
} from 'react-native-reanimated';
import { fadeInDown } from '@/constants/entering';
import { ArrowLeft, X, ScanLine, BookUser, AlertCircle, ArrowUpDown, Search, ChevronDown, ChevronUp, RefreshCw, ExternalLink } from 'lucide-react-native';
import { openBrowser } from '@/services/platform';

type Step = 'select-token' | 'enter-details' | 'confirm';
type TxStatus = 'idle' | 'preparing' | 'signing' | 'submitting' | 'confirming' | 'confirmed' | 'error';

function isValidAddress(addr: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(addr);
}

function explorerUserOpUrl(chainId: number): string | null {
  // Most ERC-4337 UserOps can be tracked via jiffyscan
  return `https://jiffyscan.xyz`;
}

/** X button that appears after 3 seconds — gives biometric time to pop up before showing cancel. */
function TxCancelButton({ onCancel }: { onCancel: () => void }) {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const timer = setTimeout(() => setVisible(true), 3000);
    return () => clearTimeout(timer);
  }, []);
  if (!visible) return null;
  return (
    <Pressable onPress={onCancel} hitSlop={12} style={{ padding: space.xs }}>
      <X size={18} color={color.fg.subtle} strokeWidth={2} />
    </Pressable>
  );
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

/** Convert a human-readable balance string (e.g. "0.0113") to BigInt wei. */
function balanceToWei(balance: string, decimals: number): bigint {
  return BigInt('0x' + amountToWeiHex(balance, decimals));
}

/** Resolve the token amount from user input, handling fiat-input mode.
 *  In fiat mode the typed value is in the user's *display currency*, so we divide
 *  by the token's price in that currency (priceUsd × USD→fiat rate), not by the
 *  raw USD price. Truncates to `decimals` to avoid floating-point garbage. */
function resolveTokenAmount(amount: string, inFiat: boolean, priceUsd: number | null | undefined, decimals: number = 18, rate: number = 1): string {
  if (!inFiat || !priceUsd || priceUsd <= 0) return amount;
  const fiat = parseFloat(amount || '0');
  if (fiat <= 0) return '0';
  const fiatPrice = priceUsd * (rate > 0 ? rate : 1);
  return (fiat / fiatPrice).toFixed(decimals).replace(/\.?0+$/, '');
}

/**
 * Font size for the amount the user is typing. Big-tech input pattern (Cash App):
 * the number stays on one line and shrinks *smoothly* as digits are added — no
 * visible step jumps, never abbreviated (you must see exactly what you typed).
 */
function amountFontSize(value: string): number {
  const len = Math.max(value.length, 1);
  const size = Math.round(230 / Math.max(len, 5.75)); // ~5.75 chars at the 40px max
  return Math.max(17, Math.min(40, size));
}

/** Validate and constrain amount input: max `maxDecimals` decimal places, valid number chars only. */
function sanitizeAmountInput(text: string, maxDecimals: number): string | null {
  // Allow only digits and a single dot
  const cleaned = text.replace(/[^0-9.]/g, '');
  // Reject multiple dots
  if ((cleaned.match(/\./g) || []).length > 1) return null;

  const parts = cleaned.split('.');
  if (parts.length === 2 && parts[1].length > maxDecimals) {
    // Truncate excess decimals
    return parts[0] + '.' + parts[1].slice(0, maxDecimals);
  }
  return cleaned;
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
  const { t } = useTranslation();
  const router = useSafeRouter();
  const params = useLocalSearchParams<{ preselectedSymbol?: string; preselectedNetwork?: string; prefilledRecipient?: string }>();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const dc = useDisplayCurrency();
  const formatUsd = dc.fmt;

  const hasPreselection = !!(params.prefilledRecipient || (params.preselectedSymbol && params.preselectedNetwork));
  const [step, setStep] = useState<Step>(hasPreselection ? 'enter-details' : 'select-token');
  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedToken, setSelectedToken] = useState<APIToken | null>(null);
  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [sending, setSending] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const [tokenSearch, setTokenSearch] = useState('');
  const [feeEstimate, setFeeEstimate] = useState<TransactionFeeEstimate | null>(null);
  const [estimatingGas, setEstimatingGas] = useState(false);
  const [fundingNeeded, setFundingNeeded] = useState<FundingNeeded | null>(null);
  const fundingRetryCount = useRef(0);
  const [txStatus, setTxStatus] = useState<TxStatus>('idle');
  const [txHash, setTxHash] = useState<string | null>(null);
  const [txError, setTxError] = useState<string | null>(null);
  const [inputInUsd, setInputInUsd] = useState(false);
  const [gasExpanded, setGasExpanded] = useState(false);
  const [gasTier, setGasTier] = useState<GasTier>('standard');
  const [refreshingGas, setRefreshingGas] = useState(false);
  const [recentRecipients, setRecentRecipients] = useState<string[]>([]);
  const [showContacts, setShowContacts] = useState(false);
  const [amountWarning, setAmountWarning] = useState<string | null>(null);
  const [recipientIdentity, setRecipientIdentity] = useState<RecipientIdentity | null>(null);

  // Prefetch account credential + webauthn module while user reviews confirm screen
  const amountInputRef = useRef<TextInput>(null);
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
        } else if (params.prefilledRecipient && nonZero.length > 0) {
          // Quick-send from scan: auto-select highest-value token, prefill recipient
          setSelectedToken(nonZero[0]);
          setRecipient(params.prefilledRecipient);
          setStep('enter-details');
        }
      })
      .catch(() => showAlert(t('common.error'), t('send.alertLoadTokensError')))
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

  // Compute real-time amount warnings
  useEffect(() => {
    if (!selectedToken || !amount) {
      setAmountWarning(null);
      return;
    }

    const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
    const amountNum = parseFloat(tokenAmount || '0');
    if (isNaN(amountNum) || amountNum <= 0) {
      setAmountWarning(null);
      return;
    }

    const chainId = tokenChainId(selectedToken);
    const sym = nativeSymbol(chainId);

    if (isNativeToken(selectedToken)) {
      // Native token: check amount + gas > balance
      const balanceWei = balanceToWei(selectedToken.balance, selectedToken.decimals);
      const amountWei = BigInt('0x' + amountToWeiHex(tokenAmount, selectedToken.decimals));
      if (amountWei > balanceWei) {
        setAmountWarning(t('send.warnNotEnoughToken', { symbol: selectedToken.symbol }));
        return;
      }
      // Also check if gas can be covered (use cached estimate if available)
      if (feeEstimate) {
        const reserveWei = feeEstimate.totalWei * 3n;
        if (amountWei + reserveWei > balanceWei) {
          setAmountWarning(t('send.warnInsufficientForGas', { sym }));
          return;
        }
      }
    } else {
      // ERC-20: check token balance
      const tokenBal = tokenBalanceDouble(selectedToken);
      if (amountNum > tokenBal) {
        setAmountWarning(t('send.warnNotEnoughToken', { symbol: selectedToken.symbol }));
        return;
      }
      // Check native token balance for gas
      const nativeToken = tokens.find(t => isNativeToken(t) && tokenChainId(t) === chainId);
      if (feeEstimate) {
        const nativeBalWei = nativeToken
          ? balanceToWei(nativeToken.balance, nativeToken.decimals)
          : 0n;
        if (nativeBalWei < feeEstimate.totalWei) {
          setAmountWarning(t('send.warnInsufficientGas', { sym }));
          return;
        }
      } else if (!nativeToken || tokenBalanceDouble(nativeToken) === 0) {
        setAmountWarning(t('send.warnNeedGas', { sym }));
        return;
      }
    }

    setAmountWarning(null);
  }, [amount, inputInUsd, selectedToken, tokens, feeEstimate, dc.rate]);

  // Resolve recipient identity (passkey index → ENS) when a valid address is entered
  useEffect(() => {
    setRecipientIdentity(null);
    if (!/^0x[0-9a-fA-F]{40}$/.test(recipient)) return;

    let cancelled = false;
    resolveRecipientIdentity(recipient)
      .then((id) => { if (!cancelled) setRecipientIdentity(id); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [recipient]);

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
      showAlert(t('send.alertInvalidAddressTitle'), t('send.alertInvalidAddressBody'));
      return;
    }
    const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken!.priceUsd, selectedToken!.decimals, dc.rate);
    const amountNum = parseFloat(tokenAmount);
    if (isNaN(amountNum) || amountNum <= 0) {
      showAlert(t('send.alertInvalidAmountTitle'), t('send.alertInvalidAmountBody'));
      return;
    }
    if (amountWarning) {
      showAlert(t('send.alertInsufficientBalanceTitle'), amountWarning);
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

      // Estimate gas + check/sponsor bundler funding BEFORE advancing to confirm.
      // This ensures the user never sees a flash-back from confirm to enter-details.
      setEstimatingGas(true);
      setFeeEstimate(null);

      try {
        // Race with a timeout — pool init for new chains can stall on slow RPCs.
        // If it takes too long, skip the pre-check and let the bundler reject at submit time.
        const preCheck = async () => {
          const [feeResult] = await Promise.allSettled([
            estimateTransactionFee(activeAccount!.address, chainId, gasTier),
            fetchBundlerAccountInfo(chainId, activeAccount!.address),
          ]);

          const fee = feeResult.status === 'fulfilled' ? feeResult.value : null;
          setFeeEstimate(fee);

          // Divide out tier markup: fee.totalWei uses userOpMaxFee (gasPrice × tier),
          // but the bundler's balance check uses raw chain gasPrice.
          const m = GAS_TIER_MULTIPLIERS[gasTier];
          const bundlerCost = fee ? (fee.totalWei * m.den) / m.num : undefined;
          return checkBundlerFunding(chainId, activeAccount!.address, bundlerCost);
        };
        const timeout = new Promise<null>(r => setTimeout(() => r(null), 15_000));
        const funding = await Promise.race([preCheck(), timeout]);
        if (funding) {
          setFundingNeeded(funding);
          setEstimatingGas(false);
          return;
        }
      } catch { /* proceed */ }

      setEstimatingGas(false);
      setStep('confirm');
    } else {
      setStep('confirm');
    }
  };

  const handleMaxAmount = async () => {
    if (!selectedToken) return;
    // Max always fills in token amount (not USD)
    if (inputInUsd) setInputInUsd(false);

    // For native tokens (ETH, BNB, etc.), reserve gas for the EntryPoint prefund.
    // The Safe must hold: transferAmount + prefund, so max = balance - prefund.
    // We add a 50% gas margin to avoid tx failure from gas price volatility.
    if (isNativeToken(selectedToken) && activeAccount) {
      try {
        const chainId = tokenChainId(selectedToken);
        const fee = feeEstimate ?? await estimateTransactionFee(activeAccount.address, chainId, gasTier);
        // Use string-based conversion to avoid floating-point precision loss
        const balanceWei = balanceToWei(selectedToken.balance, selectedToken.decimals);
        // Reserve 3x estimated gas (200% margin for gas price volatility)
        const reserveWei = fee.totalWei * 3n;
        if (balanceWei > reserveWei) {
          const maxWei = balanceWei - reserveWei;
          const maxEth = Number(maxWei) / 1e18;
          // Show enough decimals without trailing zeros
          setAmount(maxEth.toFixed(18).replace(/\.?0+$/, ''));
          return;
        }
        // Balance too low to cover gas — set to 0
        setAmount('0');
        return;
      } catch {
        // Estimation failed — fall through to full balance (tx may fail but user sees the error)
      }
    }

    // ERC-20 tokens: gas is paid in native token, so full balance is sendable
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
    // Go straight to confirm — the modal already verified balance >= threshold.
    // Re-running handleContinue would re-estimate gas (potentially getting a higher
    // price), causing the modal to reappear in a loop.
    setStep('confirm');
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
        throw new Error(t('send.txErrorPublicKey'));
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
      const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
      const weiHex = amountToWeiHex(tokenAmount, selectedToken.decimals);

      const maxFee = feeEstimate?.maxFeePerGas;
      let result;
      if (isNativeToken(selectedToken)) {
        result = await sendNative(
          activeAccount.address, recipient, weiHex, chainId,
          stored.publicKeyHex, signFn, maxFee,
        );
      } else {
        result = await sendERC20(
          activeAccount.address, selectedToken.tokenAddress!, recipient, weiHex, chainId,
          stored.publicKeyHex, signFn, maxFee,
        );
      }

      // Bundler accepted the UserOp — treat the payment as sent right now (we
      // have the userOpHash). The on-chain tx hash resolves in the background to
      // light up the explorer link; a slow/failed receipt poll must NOT turn a
      // submitted payment into an error.
      setTxStatus('confirmed');
      setSending(false);
      clearTokenCache(activeAccount.address);

      // Persist the USD value (at send time) so the activity feed can show the
      // fiat amount — without it, non-stablecoin sends (e.g. BNB) render with no
      // fiat, since there's no price to recover later. Persisted exactly once
      // (saveTransaction appends; no upsert) when the hash settles.
      const sentUsd = parseFloat(tokenAmount) * (selectedToken.priceUsd ?? 0);
      const persistSend = (hash: string) => saveTransaction({
        id: result.userOpHash,
        userOpHash: result.userOpHash,
        txHash: hash,
        from: activeAccount.address,
        to: recipient,
        toName: recipientIdentity?.name,
        value: tokenAmount,
        symbol: selectedToken!.symbol,
        decimals: selectedToken!.decimals,
        chainId,
        timestamp: Math.floor(Date.now() / 1000),
        status: 'confirmed',
        type: 'send',
        usd: sentUsd > 0 ? '$' + sentUsd.toFixed(2) : undefined,
      }).catch(() => {});

      // Resolve the on-chain hash in the background; persist with it if it
      // arrives, otherwise persist hash-less so it still lands in history.
      result.waitForTxHash()
        .then((hash) => { setTxHash(hash); persistSend(hash); })
        .catch(() => { persistSend(''); });

    } catch (error: any) {
      if (error?.code === 'PASSKEY_CANCELLED') {
        setTxStatus('idle');
      } else if (error?.message?.includes('Insufficient balance on dedicated bundler EOA')) {
        // Bundler explicitly says EOA balance is insufficient — show funding modal.
        // Parse the server's actual required balance from the error message to avoid
        // threshold mismatch (client estimate vs server's gas price) causing an infinite loop.
        fundingRetryCount.current += 1;
        if (fundingRetryCount.current > 3) {
          // Break infinite loop — show error instead of modal
          fundingRetryCount.current = 0;
          setTxError(t('send.txErrorBundlerLoop'));
          setTxStatus('error');
        } else {
          setTxStatus('idle');
          try {
            const chainId = tokenChainId(selectedToken!);
            clearBundlerCache(chainId, activeAccount!.address);
            const info = await fetchBundlerAccountInfo(chainId, activeAccount!.address);
            if (info) {
              // Try to parse server's required balance: "...required: 123456..."
              const requiredMatch = error.message.match(/required:\s*(\d+)/);
              let thresholdWei: bigint;
              if (requiredMatch) {
                thresholdWei = BigInt(requiredMatch[1]);
              } else {
                const fee = feeEstimate ?? await estimateTransactionFee(activeAccount!.address, chainId, gasTier);
                // Divide out tier markup to match server's raw gasPrice calculation
                const tm = GAS_TIER_MULTIPLIERS[gasTier];
                thresholdWei = (fee.totalWei * tm.den) / tm.num;
              }
              const deficit = thresholdWei - info.spendableBalance;
              const base = deficit > 0n ? deficit : thresholdWei;
              const recommendedWei = (base * 12n) / 10n;
              setFundingNeeded({
                reason: 'deposit_needed',
                sponsorshipAvailable: true,
                depositAddress: info.depositAddress,
                safeAddress: activeAccount!.address,
                chainId,
                nativeSym: info.nativeSym,
                thresholdWei,
                recommendedWei,
                currentBalance: info.spendableBalance,
                recommendedFormatted: formatWei(recommendedWei),
                currentFormatted: formatWei(info.spendableBalance),
              });
              return;
            }
          } catch { /* fall through */ }
          setTxError(t('send.txErrorBundlerFund'));
          setTxStatus('error');
        }
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
  const filteredTokens = tokenSearch
    ? tokens.filter((t) => {
        const q = tokenSearch.toLowerCase();
        return t.symbol.toLowerCase().includes(q) || t.name.toLowerCase().includes(q) || t.network.toLowerCase().includes(q);
      })
    : tokens;

  const renderSelectToken = () => (
    <Animated.View style={styles.stepContainer} entering={fadeInDown(0, 300)}>
      <Text style={styles.stepTitle}>{t('send.selectTokenTitle')}</Text>
      <View style={styles.searchWrap}>
        <Search size={16} color={color.fg.subtle} strokeWidth={2} />
        <TextInput
          style={styles.searchInput}
          placeholder={t('send.searchPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          value={tokenSearch}
          onChangeText={setTokenSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
      </View>
      {loading ? (
        <Text style={styles.loadingText}>{t('send.loadingTokens')}</Text>
      ) : filteredTokens.length === 0 ? (
        <View style={styles.emptyContainer}>
          <Text style={styles.emptyText}>{tokenSearch ? t('send.noMatchingTokens') : t('send.noTokensWithBalance')}</Text>
        </View>
      ) : (
        <FlatList
          data={filteredTokens}
          keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
          renderItem={({ item, index }) => (
            <TokenRow
              symbol={item.symbol}
              chainLabel={chainName(tokenChainId(item))}
              logoUrls={tokenLogoURLs(item)}
              balance={formatTokenAmount(tokenBalanceDouble(item), { compact: true })}
              usdValue={tokenUsdValue(item) > 0 ? formatUsd(tokenUsdValue(item)) : undefined}
              onPress={() => { handleSelectToken(item); setTokenSearch(''); }}
              index={index}
            />
          )}
          initialNumToRender={10}
          windowSize={5}
          showsVerticalScrollIndicator={false}
          keyboardShouldPersistTaps="handled"
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
    // Fiat-input mode is denominated in the user's display currency, not USD.
    const fiatPrice = (selectedToken.priceUsd ?? 0) * dc.rate; // 1 token in display currency
    const fiatDecimals = ZERO_DECIMAL_CODES.has(dc.code) ? 0 : 2;

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false} keyboardShouldPersistTaps="handled">
        <Animated.View entering={fadeInDown(0, 300)}>
          <Text style={styles.stepTitle}>{t('send.sendTitle', { symbol: selectedToken.symbol })}</Text>

          {/* Hero card — tap to switch token */}
          <Pressable onPress={() => { setStep('select-token'); setSelectedToken(null); setAmount(''); setInputInUsd(false); }}>
          <VelaCard style={styles.heroCard}>
            <View style={styles.heroRow}>
              <TokenLogo symbol={selectedToken.symbol} logoUrls={logos} size={44} />
              <View style={styles.heroIdentity}>
                <Text style={styles.heroSymbol}>{selectedToken.symbol}</Text>
                <Text style={styles.heroChain}>{chain}</Text>
              </View>
              <View style={styles.heroBalance}>
                <AmountText
                  text={formatTokenAmount(balance, { compact: true })}
                  size={text.xl}
                  minScale={0.7}
                  style={styles.heroAmount}
                  containerStyle={styles.heroAmountBox}
                />
                {tokenUsdValue(selectedToken) > 0 && (
                  <Text style={styles.heroUsd}>
                    {formatUsd(tokenUsdValue(selectedToken))}
                  </Text>
                )}
              </View>
            </View>
          </VelaCard>
          </Pressable>

          {/* Amount — large display with inline unit */}
          <Pressable style={styles.amountWrap} onPress={() => amountInputRef.current?.focus()}>
            <View style={styles.amountTopRow}>
              <View style={styles.amountInputWrap}>
                <TextInput
                  ref={amountInputRef}
                  testID="amount-input"
                  style={[styles.amountInput, { fontSize: amountFontSize(amount) }]}
                  placeholder="0"
                  placeholderTextColor={color.fg.subtle}
                  value={amount}
                  onChangeText={(t) => {
                    const maxDec = inputInUsd ? fiatDecimals : selectedToken.decimals;
                    const sanitized = sanitizeAmountInput(t, maxDec);
                    if (sanitized !== null) setAmount(sanitized);
                  }}
                  keyboardType="decimal-pad"
                  selectionColor={color.fg.muted}
                />
              </View>
              {amount ? (
                <Text style={[styles.unitLabel, { fontSize: Math.max(amountFontSize(amount) * 0.7, 16) }]}>
                  {inputInUsd ? dc.code : selectedToken.symbol}
                </Text>
              ) : (
                <Pressable onPress={handleMaxAmount} hitSlop={8} style={styles.maxBtn}>
                  <Text style={styles.maxBtnText}>{t('send.maxBtn')}</Text>
                </Pressable>
              )}
            </View>
            {/* Conversion toggle row — below the input, like ↕ 0.0113 ETH */}
            {selectedToken.priceUsd != null && selectedToken.priceUsd > 0 ? (
              <Pressable
                onPress={() => {
                  const val = parseFloat(amount || '0');
                  if (val > 0 && fiatPrice > 0) {
                    if (inputInUsd) {
                      setAmount((val / fiatPrice).toFixed(selectedToken.decimals).replace(/\.?0+$/, ''));
                    } else {
                      setAmount((val * fiatPrice).toFixed(fiatDecimals));
                    }
                  }
                  setInputInUsd(!inputInUsd);
                }}
                hitSlop={8}
                style={styles.conversionRow}
              >
                <ArrowUpDown size={14} color={color.fg.muted} strokeWidth={2.5} />
                <Text style={styles.conversionText}>
                  {amount
                    ? inputInUsd
                      ? `${(parseFloat(amount || '0') / (fiatPrice || 1)).toFixed(Math.min(selectedToken.decimals, 8)).replace(/\.?0+$/, '')} ${selectedToken.symbol}`
                      : formatUsd(parseFloat(amount || '0') * (selectedToken.priceUsd ?? 0))
                    : inputInUsd
                      ? `0 ${selectedToken.symbol}`
                      : formatUsd(0)}
                </Text>
              </Pressable>
            ) : null}
          </Pressable>
          {amountWarning ? (
            <Text style={styles.amountWarning}>{amountWarning}</Text>
          ) : null}

          {/* Recipient */}
          <View style={styles.fieldLabelRow}>
            <Text style={styles.fieldLabel}>{t('send.recipientLabel')}</Text>
            <Pressable onPress={() => setShowScanner(true)} hitSlop={8} style={styles.fieldLabelAction}>
              <ScanLine size={15} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          </View>
          <View style={styles.inputWrap}>
            <TextInput
              style={styles.input}
              placeholder={t('send.recipientPlaceholder')}
              placeholderTextColor={color.fg.subtle}
              value={recipient}
              onChangeText={(t) => { setRecipient(t); setShowContacts(false); }}
              autoCapitalize="none"
              autoCorrect={false}
              editable={!params.prefilledRecipient}
              multiline
              blurOnSubmit
              returnKeyType="done"
            />
            {recentRecipients.length > 0 && (
              <View style={styles.inputIcons}>
                <Pressable onPress={() => setShowContacts(!showContacts)} hitSlop={6}>
                  <BookUser size={18} color={color.fg.subtle} strokeWidth={2} />
                </Pressable>
              </View>
            )}
          </View>

          {/* Recipient identity */}
          {recipientIdentity && (
            <View style={styles.identityRow}>
              <Text style={styles.identityName}>
                {recipientIdentity.source === 'passkey' ? '👤 ' : ''}
                {recipientIdentity.name}
              </Text>
              <Text style={styles.identitySource}>
                {recipientIdentity.source === 'passkey' ? t('send.velaUser') : recipientIdentity.source}
              </Text>
            </View>
          )}

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

          {fundingNeeded?.reason === 'wallet_balance_too_low' && (
            <View style={styles.lowBalanceWarning}>
              <Text style={styles.lowBalanceText}>
                {t('send.lowBalanceWarning', { nativeSym: fundingNeeded.nativeSym })}
              </Text>
            </View>
          )}

          <VelaButton
            title={estimatingGas ? t('send.preparing') : t('send.continueBtn')}
            onPress={handleContinue}
            loading={estimatingGas}
            style={styles.continueBtn}
            disabled={!recipient || !amount || estimatingGas || fundingNeeded?.reason === 'wallet_balance_too_low'}
          />
        </Animated.View>
      </ScrollView>
    );
  };

  // Step 3: Confirm
  const renderConfirm = () => {
    if (!selectedToken) return null;
    const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
    const amountNum = parseFloat(tokenAmount || '0');
    const usdAmount = amountNum * (selectedToken.priceUsd ?? 0);
    const logos = tokenLogoURLs(selectedToken);
    const chain = chainName(tokenChainId(selectedToken));

    // Fee calculations
    const sym = nativeSymbol(tokenChainId(selectedToken));
    const nativePrice = isNativeToken(selectedToken)
      ? (selectedToken.priceUsd ?? 0)
      : (tokens.find(t => isNativeToken(t) && tokenChainId(t) === tokenChainId(selectedToken))?.priceUsd ?? 0);
    const feeNative = feeEstimate ? Number(feeEstimate.totalWei) / 1e18 : 0;
    const feeUsd = feeNative * nativePrice;

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false}>
        <Animated.View entering={fadeInDown(0, 300)}>
          <Text style={styles.stepTitle}>{t('send.confirmTitle')}</Text>

          {/* Transfer card: From → To with token info */}
          <VelaCard style={styles.confirmCard}>
            {/* From */}
            <View style={styles.transferEndpoint}>
              <Text style={styles.transferLabel}>{t('send.fromLabel')}</Text>
              <Text style={styles.transferName}>{activeAccount?.name ?? t('send.walletFallbackName')}</Text>
              <Text style={styles.transferAddr}>{shortAddr(address ?? '')}</Text>
            </View>

            {/* Line + Token */}
            <View style={styles.transferMiddle}>
              <View style={styles.transferLineCol}>
                <View style={styles.transferLine} />
              </View>
              <View style={styles.transferToken}>
                <TokenLogo symbol={selectedToken.symbol} logoUrls={logos} size={36} />
                <View style={styles.transferTokenIdentity}>
                  <Text style={styles.transferTokenSymbol}>{selectedToken.symbol}</Text>
                  <Text style={styles.transferTokenChain}>{chain}</Text>
                </View>
                <View style={styles.transferTokenValues}>
                  <Text style={styles.transferTokenAmount}>{formatBalance(amountNum)}</Text>
                  {usdAmount > 0 && (
                    <Text style={styles.transferTokenSub}>≈ {formatUsd(usdAmount)}</Text>
                  )}
                </View>
              </View>
            </View>

            {/* To */}
            <View style={styles.transferEndpoint}>
              <Text style={styles.transferLabel}>{t('send.toLabel')}</Text>
              {recipientIdentity && (
                <Text style={styles.transferName}>{recipientIdentity.name}</Text>
              )}
              <Text style={styles.transferAddr}>{shortAddr(recipient)}</Text>
            </View>
          </VelaCard>

          {/* Gas Details — collapsed by default, fee shown in toggle row */}
          <Pressable onPress={() => setGasExpanded(!gasExpanded)} style={styles.gasToggleRow}>
            <Text style={styles.gasToggleLabel}>{t('send.estFeeLabel')}</Text>
            <View style={styles.gasToggleRight}>
              <View style={styles.gasToggleValues}>
                <Text style={styles.gasToggleValue}>
                  {estimatingGas ? t('send.estimatingFee') : feeEstimate ? `~${formatWeiToEth(feeEstimate.totalWei)} ${sym}` : '—'}
                </Text>
                {!estimatingGas && feeUsd > 0.001 && (
                  <Text style={styles.gasToggleSub}>≈ {formatUsd(feeUsd)}</Text>
                )}
              </View>
              {feeEstimate && !estimatingGas && (
                gasExpanded
                  ? <ChevronUp size={16} color={color.fg.subtle} strokeWidth={2} />
                  : <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />
              )}
            </View>
          </Pressable>
          {gasExpanded && feeEstimate && (() => {
            const bundlerGwei = Number(feeEstimate.bundlerGasPrice) / 1e9;
            const userOpGwei = Number(feeEstimate.maxFeePerGas) / 1e9;

            const handleRefreshGas = async () => {
              if (!activeAccount || !selectedToken || refreshingGas) return;
              setRefreshingGas(true);
              try {
                await refreshGasPrice(tokenChainId(selectedToken));
                const fee = await estimateTransactionFee(activeAccount.address, tokenChainId(selectedToken), gasTier);
                setFeeEstimate(fee);
              } catch { /* ignore */ }
              setRefreshingGas(false);
            };

            const handleTierChange = async (tier: GasTier) => {
              setGasTier(tier);
              if (!activeAccount || !selectedToken) return;
              try {
                const fee = await estimateTransactionFee(activeAccount.address, tokenChainId(selectedToken), tier);
                setFeeEstimate(fee);
              } catch { /* ignore */ }
            };

            return (
              <VelaCard style={styles.gasCard}>
                {/* Tier selector */}
                <View style={styles.tierRow}>
                  {(['slow', 'standard', 'rapid', 'fast'] as GasTier[]).map((t) => (
                    <Pressable
                      key={t}
                      style={[styles.tierBtn, gasTier === t && styles.tierBtnActive]}
                      onPress={() => handleTierChange(t)}
                    >
                      <Text style={[styles.tierBtnText, gasTier === t && styles.tierBtnTextActive]}>
                        {GAS_TIER_MULTIPLIERS[t].label}
                      </Text>
                    </Pressable>
                  ))}
                  <Pressable onPress={handleRefreshGas} hitSlop={8} style={styles.tierRefresh}>
                    {refreshingGas ? (
                      <ActivityIndicator size={14} color={color.fg.muted} />
                    ) : (
                      <RefreshCw size={14} color={color.fg.muted} strokeWidth={2} />
                    )}
                  </Pressable>
                </View>
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.gasPriceLabel')} value={`${bundlerGwei.toFixed(4)} Gwei`} />
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.gasPriceUserOpLabel')} value={`${userOpGwei.toFixed(4)} Gwei`} />
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.gasLimitLabel')} value={feeEstimate.totalGas.toLocaleString()} />
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.walletDeployedLabel')} value={feeEstimate.deployed ? t('send.walletDeployedYes') : t('send.walletDeployedNo')} />
              </VelaCard>
            );
          })()}

          {txStatus === 'idle' && (
            <VelaButton
              title={estimatingGas ? t('send.checkingGas') : t('send.confirmSendBtn')}
              onPress={handleConfirm}
              variant="accent"
              loading={sending}
              disabled={estimatingGas}
              style={styles.confirmBtn}
            />
          )}

          {txStatus !== 'idle' && (
            <Animated.View entering={fadeInDown(0, 200)} style={styles.txStatusWrap}>
              {(txStatus === 'preparing' || txStatus === 'signing' || txStatus === 'submitting') && (
                <View style={styles.txStatusRow}>
                  <Animated.View style={styles.txSpinner}>
                    <ActivityIndicator size="small" color={color.accent.base} />
                  </Animated.View>
                  <Text style={[styles.txStatusText, { flex: 1 }]}>
                    {txStatus === 'preparing' ? t('send.txPreparing') :
                     txStatus === 'signing' ? t('send.txSigning') :
                     t('send.txSubmitting')}
                  </Text>
                  {(txStatus === 'preparing' || txStatus === 'signing') && (
                    <TxCancelButton onCancel={() => { Passkey.cancelSign(); setTxStatus('idle'); setSending(false); }} />
                  )}
                </View>
              )}
              {txStatus === 'error' && (
                <View style={styles.txStatusRow}>
                  <AlertCircle size={20} color={color.error.base} strokeWidth={2.5} />
                  <Text style={styles.txStatusError}>{txError}</Text>
                </View>
              )}
              {txStatus === 'error' && (
                <View style={styles.txStatusActions}>
                  <Pressable
                    style={styles.txRetryBtn}
                    onPress={() => { setTxStatus('idle'); setTxError(null); }}
                  >
                    <Text style={styles.txRetryBtnText}>{t('send.txRetryBtn')}</Text>
                  </Pressable>
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

      {/* Transaction confirmed — full-screen receipt replaces everything */}
      {txStatus === 'confirmed' && selectedToken ? (
        <TransactionReceipt
          from={activeAccount?.address ?? ''}
          fromName={activeAccount?.name}
          to={recipient}
          toName={recipientIdentity?.name}
          amount={resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate)}
          symbol={selectedToken.symbol}
          chainId={tokenChainId(selectedToken)}
          txHash={txHash ?? ''}
          logoUrls={tokenLogoURLs(selectedToken)}
          usdValue={parseFloat(resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate)) * (selectedToken.priceUsd ?? 0)}
          rate={dc.rate}
          currencyCode={dc.code}
          currencySymbol={dc.symbol}
          timestamp={new Date()}
          recipientIdentity={recipientIdentity}
          onDone={() => router.back()}
        />
      ) : (
        <>
          {step === 'select-token' && renderSelectToken()}
          {step === 'enter-details' && renderEnterDetails()}
          {step === 'confirm' && renderConfirm()}
        </>
      )}

      <QRScanner
        visible={showScanner}
        onScan={(addr) => {
          setRecipient(addr);
          setShowScanner(false);
        }}
        onClose={() => setShowScanner(false)}
      />

      {fundingNeeded && fundingNeeded.reason !== 'wallet_balance_too_low' && (
        <BundlerFundingModal
          visible={!!fundingNeeded}
          funding={fundingNeeded}
          onFunded={handleFundingComplete}
          onCancel={() => { setFundingNeeded(null); fundingRetryCount.current = 0; }}
        />
      )}
    </ScreenContainer>
  );
}

function ConfirmRow({ label, value, sub, highlight }: { label: string; value: string; sub?: string; highlight?: boolean }) {
  return (
    <View style={styles.confirmRow}>
      <Text style={styles.confirmLabel}>{label}</Text>
      <View style={styles.confirmValueWrap}>
        <Text style={[styles.confirmValue, highlight && styles.confirmValueHighlight]}>
          {value}
        </Text>
        {sub ? <Text style={styles.confirmSub}>{sub}</Text> : null}
      </View>
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

  // Search
  searchWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    marginBottom: space.xl,
  },
  searchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,

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
    flexShrink: 1,
    maxWidth: '58%', // keep the token symbol readable; huge balances shrink
  },
  heroAmountBox: {
    alignSelf: 'stretch',
  },
  heroAmount: {
    fontSize: text.xl,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    textAlign: 'right',
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
    outlineStyle: 'none',
  } as any,
  inputIcons: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingRight: space.lg,
  },
  // Recipient identity
  identityRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    marginBottom: space.xs,
  },
  identityName: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },
  identitySource: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
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
    marginBottom: space.lg,
  },
  amountTopRow: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  amountInputWrap: {
    flex: 1,
    overflow: 'hidden',
  },
  amountInput: {
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,
  unitLabel: {
    ...inter.medium,
    color: color.fg.subtle,
    marginLeft: space.sm,
    flexShrink: 0,
  },
  maxBtn: {
    paddingVertical: space.xs,
    paddingHorizontal: space.lg,
    backgroundColor: color.bg.raised,
    borderRadius: radius.full,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  maxBtnText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  conversionRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.md,
  },
  conversionText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  amountWarning: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.error.base,
    marginTop: space.sm,
    marginBottom: space.sm,
    paddingHorizontal: space.xs,
  },
  continueBtn: {
    marginTop: space.lg,
  },
  lowBalanceWarning: {
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    padding: space.lg,
    marginTop: space.lg,
  },
  lowBalanceText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.warning.base,
    lineHeight: 20,
    textAlign: 'center' as const,
  },

  // Confirm — transfer flow card
  confirmCard: {
    padding: space['2xl'],
    marginBottom: space.lg,
  },
  transferEndpoint: {
    gap: 2,
  },
  transferLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.5,
  },
  transferName: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  transferAddr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
  },
  transferMiddle: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginVertical: space['2xl'],
  },
  transferLineCol: {
    alignSelf: 'stretch',
    alignItems: 'center',
    width: space.lg,
  },
  transferLine: {
    width: 1,
    flex: 1,
    backgroundColor: color.border.base,
  },
  transferToken: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
  },
  transferTokenIdentity: {
    flex: 1,
    gap: 1,
  },
  transferTokenSymbol: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  transferTokenChain: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  transferTokenValues: {
    alignItems: 'flex-end' as const,
    gap: 1,
  },
  transferTokenAmount: {
    fontSize: text.base,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  transferTokenSub: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  // Gas toggle row
  gasToggleRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
    paddingHorizontal: space.sm,
    marginBottom: space.sm,
  },
  gasToggleLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  gasToggleRight: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  gasToggleValues: {
    alignItems: 'flex-end' as const,
  },
  gasToggleValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },
  gasToggleSub: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  gasCard: {
    padding: space.xl,
    marginBottom: space.lg,
  },
  tierRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingBottom: space.lg,
  },
  tierBtn: {
    flex: 1,
    paddingVertical: space.sm,
    borderRadius: radius.md,
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
  },
  tierBtnActive: {
    backgroundColor: color.fg.base,
  },
  tierBtnText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
  },
  tierBtnTextActive: {
    color: color.fg.inverse,
  },
  tierRefresh: {
    padding: space.sm,
  },
  // Kept for gas detail rows (ConfirmRow)
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
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    flexShrink: 0,
    marginRight: space.lg,
  },
  confirmValueWrap: {
    alignItems: 'flex-end' as const,
    flexShrink: 1,
  },
  confirmValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    textAlign: 'right' as const,
  },
  confirmSub: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: 2,
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
  txConfirmTime: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.accent.base,
    marginTop: 2,
  },
  txProgressTrack: {
    height: 4,
    backgroundColor: color.border.base,
    borderRadius: 2,
    marginTop: space.lg,
    overflow: 'hidden' as const,
  },
  txProgressFill: {
    height: 4,
    backgroundColor: color.accent.base,
    borderRadius: 2,
  },
  txProgressFillSlow: {
    backgroundColor: color.warning?.base ?? '#F59E0B',
  },
  txConfirmHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.sm,
  },
}));
