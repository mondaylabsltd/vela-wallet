import { QRScanner } from '@/components/QRScanner';
import { TokenLogo } from '@/components/TokenLogo';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { SlideToConfirmButton } from '@/components/ui/SlideToConfirmButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenSelector } from '@/components/ui/TokenSelector';
import { color, text, inter, space, radius, font, shadow, motion, createStyles } from '@/constants/theme';
import { chainName, nativeSymbol, networkForChainId, networkId, tokenBadgeNetwork } from '@/models/network';
import { addCustomNetworkByChainId } from '@/services/add-network';
import { parseEIP681, fromBaseUnits, toBaseUnits } from '@/services/eip681';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { type APIToken, formatBalance, isNativeToken, tokenBalanceDouble, tokenChainId, tokenId, tokenLogoURLs, tokenUsdValue } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import * as Passkey from '@/modules/passkey';
import { fromHex, toHex } from '@/services/hex';
import { sendERC20, sendNative, sendBatchCalls, estimateTransactionFee, formatWeiToEth, prefetchForSend, refreshGasPrice, rawBundlerGasCost, type TransactionFeeEstimate, type GasTier } from '@/services/safe-transaction';
import { isTempoChain, TEMPO_DEFAULT_FEE_TOKEN, TEMPO_FEE_TOKEN_DECIMALS } from '@/services/tempo';
import { simulateAssetChanges, type AssetSimResult } from '@/services/tx-simulation';
import { BalanceChangePreview } from '@/components/signing/BalanceChangePreview';
import { findAccountByCredentialId, saveTransactions, updateTransactions } from '@/services/storage';
import { ContactPicker } from '@/components/contacts/ContactPicker';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { saveContact } from '@/services/contacts';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { checkBundlerFunding, clearBundlerCache, fetchBundlerAccountInfo, formatWei, parseBundlerUnderfunded, recommendedFundingWei, type FundingNeeded } from '@/services/bundler-service';
import { AmountText } from '@/components/ui/AmountText';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { MultiRecipientEditor, makeRecipientId, recipientsAreValid, type RecipientDraft } from '@/components/send/MultiRecipientEditor';
import { buildSplitCalls, sumSplitBaseUnits, buildMultiTokenCalls, toMultiTokenSpecs, reserveNativeGas } from '@/services/batch-send';
import { useTokenMultiSelect } from '@/hooks/use-token-multi-select';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { BundlerFundingModal } from '@/components/ui/BundlerFundingModal';
import { TransactionReceipt, type ReceiptTransfer } from '@/components/ui/TransactionReceipt';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { ZERO_DECIMAL_CODES } from '@/services/currency';
import { showAlert, copyToClipboard, openBrowser, hapticSuccess, hapticError } from '@/services/platform';
import { resolveRecipientRisk, type RecipientRisk } from '@/services/recipient-risk';
import { useTranslation } from 'react-i18next';
import React, { useEffect, useRef, useState } from 'react';
import { ActivityIndicator, ScrollView, Text, TextInput, View, Pressable } from 'react-native';
import Animated, {
  useAnimatedStyle,
  withSpring,
  useSharedValue,
  Layout,
} from 'react-native-reanimated';
import { fadeInDown } from '@/constants/entering';
import { ArrowLeft, X, BookUser, AlertCircle, ArrowUpDown, ChevronDown, ChevronUp, RefreshCw, Copy, Check, Globe, Plus } from 'lucide-react-native';

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

/** ERC-20 `transfer(address,uint256)` calldata, for the balance-change pre-check. */
function encErc20Transfer(to: string, amountHex: string): string {
  const a = to.replace(/^0x/, '').toLowerCase().padStart(64, '0');
  const amt = amountHex.replace(/^0x/, '').padStart(64, '0');
  return '0xa9059cbb' + a + amt;
}

/** A zero-balance native token for a chain the user holds nothing on (locked EIP-681 send). */
function synthNativeToken(chainId: number): APIToken {
  const sym = nativeSymbol(chainId);
  return { network: networkId(chainId), chainName: chainName(chainId), symbol: sym, balance: '0', decimals: 18, logo: null, name: sym, tokenAddress: null, priceUsd: null, spam: false };
}

/** A zero-balance ERC-20 placeholder built from resolved metadata (locked EIP-681 send). */
function synthErc20Token(chainId: number, address: string, symbol: string, decimals: number): APIToken {
  return { network: networkId(chainId), chainName: chainName(chainId), symbol, balance: '0', decimals, logo: null, name: symbol, tokenAddress: address, priceUsd: null, spam: false };
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
  useLocalePrefs(); // re-render when the number format changes
  const router = useSafeRouter();
  const params = useLocalSearchParams<{
    preselectedSymbol?: string;
    preselectedNetwork?: string;
    prefilledRecipient?: string;
    // EIP-681 scan: lock the whole request (recipient + chain + token + amount).
    prefilledChainId?: string;
    prefilledTokenAddress?: string;
    prefilledAmountBase?: string;
    locked?: string;
    // Multi-token hand-off from the Home assets sheet: comma-joined tokenId()s.
    preselectedMulti?: string;
  }>();
  const locked = params.locked === '1';
  // The amount is only fixed when the request actually specified one; an
  // "open" request (token but no amount) still lets the sender choose.
  const amountLocked = locked && !!params.prefilledAmountBase;
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const dc = useDisplayCurrency();
  const formatUsd = dc.fmt;

  const hasPreselection = !!(params.prefilledRecipient || params.preselectedMulti || (params.preselectedSymbol && params.preselectedNetwork));
  const [step, setStep] = useState<Step>(hasPreselection ? 'enter-details' : 'select-token');

  // EIP-681 locked-request resolution + the exceptions it can hit.
  type LockError =
    | { kind: 'network'; chainId: number }
    | { kind: 'token' }
    | null;
  const [lockError, setLockError] = useState<LockError>(null);
  const [lockRetry, setLockRetry] = useState(0);
  const [resolvingLock, setResolvingLock] = useState(locked);
  const [addingNetwork, setAddingNetwork] = useState(false);
  const [addNetworkMsg, setAddNetworkMsg] = useState<string | null>(null);
  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedToken, setSelectedToken] = useState<APIToken | null>(null);
  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  // ① split mode (一币多人): one token → many recipients, each its own amount,
  // settled in one UserOp via sendBatchCalls. Off by default — single sends keep
  // their exact existing flow. `pickerTarget` = the row id the contact picker fills
  // (null ⇒ the single-mode recipient field).
  const [splitMode, setSplitMode] = useState(false);
  const [recipients, setRecipients] = useState<RecipientDraft[]>([]);
  const [pickerTarget, setPickerTarget] = useState<string | null>(null);
  // ② multiSelect (多币一人 / 清空): many tokens on ONE chain → one recipient, full
  // balance each, in a single MultiSend UserOp. Selection state lives in the
  // shared hook (also used by the Home assets sheet). `multiSelectMode` = we're in the
  // multiSelect enter-details/confirm flow (set when a multi-selection is confirmed).
  const [multiSelectMode, setMultiSelectMode] = useState(false);
  const multiSelect = useTokenMultiSelect();
  const [sending, setSending] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const [copiedContract, setCopiedContract] = useState(false);
  const [feeEstimate, setFeeEstimate] = useState<TransactionFeeEstimate | null>(null);
  const [estimatingGas, setEstimatingGas] = useState(false);
  const [fundingNeeded, setFundingNeeded] = useState<FundingNeeded | null>(null);
  const fundingRetryCount = useRef(0);
  // Guards UI state updates that run after an `await` in the submit flow, so a
  // user who navigates away mid-send doesn't trigger updates on an unmounted
  // screen. Persistence (DB writes) still runs regardless — only UI is gated.
  const mountedRef = useRef(true);
  useEffect(() => () => { mountedRef.current = false; }, []);
  const [txStatus, setTxStatus] = useState<TxStatus>('idle');
  const [txHash, setTxHash] = useState<string | null>(null);
  // The submitted UserOp hash — passed to the receipt so it can self-poll the
  // bundler and converge its status even if the parent's waitForTxHash times out.
  const [userOpHash, setUserOpHash] = useState<string | null>(null);
  const [txError, setTxError] = useState<string | null>(null);
  // Batch-send receipt: the per-line breakdown shown on the confirmed receipt for
  // split (1 token → N recipients) / multiSelect (N tokens → 1 recipient). null for
  // a plain single send (which uses the scalar amount/symbol props instead).
  const [receiptTransfers, setReceiptTransfers] = useState<ReceiptTransfer[] | null>(null);
  const [receiptKind, setReceiptKind] = useState<'split' | 'multiSelect' | null>(null);
  // Set when the background on-chain poll reports a definitive failure, so the
  // receipt shows a clear "Failed" stamp instead of staying "Submitted" forever.
  const [receiptFailed, setReceiptFailed] = useState(false);
  const [inputInUsd, setInputInUsd] = useState(false);
  const [gasExpanded, setGasExpanded] = useState(false);
  const [gasTier, setGasTier] = useState<GasTier>('standard');
  const [refreshingGas, setRefreshingGas] = useState(false);
  const [showContactPicker, setShowContactPicker] = useState(false);
  const [amountWarning, setAmountWarning] = useState<string | null>(null);
  const [recipientIdentity, setRecipientIdentity] = useState<RecipientIdentity | null>(null);
  // Recipient-risk signals for the confirm step — "first time" (address-poisoning
  // defense) + contract-vs-EOA. Best-effort, never a false alarm. Same signals the
  // dApp signing sheet shows; plain transfers deserve the same protection.
  const [recipientRisk, setRecipientRisk] = useState<RecipientRisk | null>(null);
  // Balance-change simulation for the confirm step (null = unknown / not run).
  const [sim, setSim] = useState<AssetSimResult | null>(null);

  // Prefetch account credential + webauthn module while user reviews confirm screen
  const amountInputRef = useRef<TextInput>(null);
  const prefetchedAccount = useRef<{ publicKeyHex: string } | null>(null);
  const webauthnModuleRef = useRef<typeof import('@/services/webauthn-verify') | null>(null);

  // Resolve a locked EIP-681 request against the loaded token list. Sets the
  // exact token (held, or a synthetic zero-balance placeholder), recipient and
  // amount — or surfaces an unsupported-network / unknown-token exception.
  const resolveLockedRequest = async (allTokens: APIToken[]) => {
    setResolvingLock(true);
    try {
      const chainId = parseInt(params.prefilledChainId ?? '', 10);
      if (!Number.isFinite(chainId)) { setLockError(null); return; }
      if (!networkForChainId(chainId)) { setLockError({ kind: 'network', chainId }); return; }

      const wantAddr = params.prefilledTokenAddress?.toLowerCase();
      let tok: APIToken | null = allTokens.find((tk) =>
        tokenChainId(tk) === chainId &&
        (wantAddr ? (!isNativeToken(tk) && tk.tokenAddress?.toLowerCase() === wantAddr) : isNativeToken(tk))
      ) ?? null;

      if (!tok) {
        if (!wantAddr) {
          tok = synthNativeToken(chainId);
        } else {
          const meta = await resolveTokenMetadata(chainId, [wantAddr]);
          const m = meta.get(wantAddr);
          if (!m) { setLockError({ kind: 'token' }); return; }
          tok = synthErc20Token(chainId, params.prefilledTokenAddress!, m.symbol, m.decimals);
        }
      }

      setLockError(null);
      setSelectedToken(tok);
      setRecipient(params.prefilledRecipient ?? '');
      if (params.prefilledAmountBase) {
        try { setAmount(fromBaseUnits(BigInt(params.prefilledAmountBase), tok.decimals)); } catch {}
      }
      setStep('enter-details');
    } finally {
      setResolvingLock(false);
    }
  };

  // "Add this network" recovery when a scanned request names an unsupported chain.
  const handleAddNetwork = async (chainId: number) => {
    setAddingNetwork(true);
    setAddNetworkMsg(null);
    try {
      const result = await addCustomNetworkByChainId(chainId);
      if (result.ok) {
        setLockError(null);
        setLockRetry((n) => n + 1); // re-run resolution now that the chain exists
      } else {
        setAddNetworkMsg(result.reason === 'not-found' ? t('send.lock.netNotFound') : (result.error || t('send.lock.netNotCompatible')));
      }
    } catch {
      setAddNetworkMsg(t('send.lock.netAddError'));
    } finally {
      setAddingNetwork(false);
    }
  };

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

        if (locked) {
          // Match against the full list (incl. zero-balance known tokens) for
          // the exact requested token; fall back to a synthetic placeholder.
          resolveLockedRequest(result);
          return;
        }

        // Multi-token hand-off from the Home assets sheet → land in multiSelect mode.
        if (params.preselectedMulti) {
          const wanted = new Set(params.preselectedMulti.split(','));
          const picked = nonZero.filter((tk) => wanted.has(tokenId(tk)));
          if (picked.length > 0) {
            multiSelect.selectTokens(picked);
            setMultiSelectMode(true);
            setSelectedToken(picked[0]);
            setStep('enter-details');
            if (activeAccount) {
              const chainId = tokenChainId(picked[0]);
              prefetchForSend(activeAccount.address, chainId);
              fetchBundlerAccountInfo(chainId, activeAccount.address).catch(() => {});
              findAccountByCredentialId(activeAccount.id).then((s) => { prefetchedAccount.current = s ?? null; });
              import('@/services/webauthn-verify').then((m) => { webauthnModuleRef.current = m; });
              estimateTransactionFee(activeAccount.address, chainId, gasTier)
                .then((f) => { if (mountedRef.current) setFeeEstimate(f); })
                .catch(() => {});
            }
          }
          return;
        }

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
  }, [address, params.preselectedSymbol, params.preselectedNetwork, params.preselectedMulti, lockRetry]);

  // Re-pull balances after the user adds/removes a custom token in the sheet,
  // so it shows up (or disappears) without a manual page refresh.
  const refreshTokens = () => {
    if (!address) return;
    clearTokenCache(address);
    fetchTokens(address)
      .then((result) => {
        const nonZero = result.filter((tk) => tokenBalanceDouble(tk) > 0);
        nonZero.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
        setTokens(nonZero);
      })
      .catch(() => {});
  };

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
      if (isTempoChain(chainId)) {
        // Tempo has no native coin: gas is paid in pathUSD (the canonical fee token).
        // The sent-token amount is checked above; here ensure pathUSD covers the fee.
        if (feeEstimate) {
          // feeEstimate.totalWei is attodollars (USD×1e-18) → pathUSD units (6 dec).
          const feePathUsd = feeEstimate.totalWei / 10n ** BigInt(18 - TEMPO_FEE_TOKEN_DECIMALS);
          const sendingPathUsd =
            selectedToken.tokenAddress?.toLowerCase() === TEMPO_DEFAULT_FEE_TOKEN.toLowerCase();
          if (sendingPathUsd) {
            const balWei = balanceToWei(selectedToken.balance, selectedToken.decimals);
            const amountWei = BigInt('0x' + amountToWeiHex(tokenAmount, selectedToken.decimals));
            if (amountWei + feePathUsd > balWei) {
              setAmountWarning(t('send.warnInsufficientForGas', { sym: 'pathUSD' }));
              return;
            }
          } else {
            const pathUsd = tokens.find(
              tk => tk.tokenAddress?.toLowerCase() === TEMPO_DEFAULT_FEE_TOKEN.toLowerCase() &&
                tokenChainId(tk) === chainId,
            );
            const pathBalWei = pathUsd ? balanceToWei(pathUsd.balance, pathUsd.decimals) : 0n;
            if (pathBalWei < feePathUsd) {
              setAmountWarning(t('send.warnNeedGas', { sym: 'pathUSD' }));
              return;
            }
          }
        }
        setAmountWarning(null);
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

  // Simulate the send (revert pre-check + net balance changes) once the user
  // reaches the confirm step — same surface the dApp signing sheet shows.
  // Best-effort: any failure leaves `sim` null and confirm shows nothing extra.
  useEffect(() => {
    const okSingle = !splitMode && !multiSelectMode && isValidAddress(recipient);
    const okSplit = splitMode && recipientsAreValid(recipients);
    const okMulti = multiSelectMode && isValidAddress(recipient) && pickedTokens.length > 0;
    if (step !== 'confirm' || !selectedToken || !activeAccount || (!okSingle && !okSplit && !okMulti)) {
      setSim(null);
      return;
    }
    let cancelled = false;
    setSim(null);
    try {
      const chainId = tokenChainId(selectedToken);
      // One call (single) or N calls (split/multiSelect) — the sim sums them into one
      // net-balance preview, the same surface a batch UserOp produces on-chain.
      let calls: { to: string; value?: string; data?: string }[];
      if (multiSelectMode) {
        calls = buildMultiTokenCalls(recipient.trim(), multiTokenSpecs(chainId));
      } else if (splitMode) {
        calls = buildSplitCalls(
          { tokenAddress: isNativeToken(selectedToken) ? null : selectedToken.tokenAddress, decimals: selectedToken.decimals },
          recipients.map((r) => ({ address: r.address.trim(), amount: r.amount })),
        );
      } else {
        const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
        const weiHex = amountToWeiHex(tokenAmount, selectedToken.decimals);
        calls = [isNativeToken(selectedToken)
          ? { to: recipient, value: '0x' + weiHex }
          : { to: selectedToken.tokenAddress!, data: encErc20Transfer(recipient, weiHex) }];
      }
      simulateAssetChanges(activeAccount.address, calls, chainId)
        .then((r) => { if (!cancelled) setSim(r); })
        .catch(() => { if (!cancelled) setSim(null); });
    } catch {
      /* malformed amount → no sim */
    }
    return () => { cancelled = true; };
  }, [step, selectedToken, recipient, amount, inputInUsd, activeAccount, dc.rate, splitMode, recipients, multiSelectMode, multiSelect.selectedIds, feeEstimate]);

  // Recipient-risk on the confirm step: "first time" (address-poisoning defense)
  // + contract-vs-EOA. Drives the first-time/contract tags by the To row and
  // whether the confirm CTA upgrades to a deliberate hold-to-confirm. Best-effort.
  useEffect(() => {
    setRecipientRisk(null);
    if (step !== 'confirm' || !selectedToken || !isValidAddress(recipient)) return;
    let cancelled = false;
    resolveRecipientRisk(tokenChainId(selectedToken), recipient)
      .then((r) => { if (!cancelled) setRecipientRisk(r); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [step, selectedToken, recipient]);

  // ── ① split-mode (一币多人) helpers ─────────────────────────────────────────
  // Enter split mode seeded with the current single recipient (amount in token
  // units) + one empty row; a converted amount keeps continuity from the hero.
  const enterSplitMode = () => {
    if (!selectedToken) return;
    const tokenAmt = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
    setRecipients([
      { id: makeRecipientId(), address: recipient, amount: amount ? tokenAmt : '' },
      { id: makeRecipientId(), address: '', amount: '' },
    ]);
    setInputInUsd(false);
    setSplitMode(true);
  };

  // Removing the last extra recipient drops back to the familiar single-send UI,
  // carrying the remaining row's address/amount with it.
  const handleRecipientsChange = (next: RecipientDraft[]) => {
    if (next.length <= 1) {
      setRecipient(next[0]?.address ?? '');
      setAmount(next[0]?.amount ?? '');
      setSplitMode(false);
      setRecipients([]);
      return;
    }
    setRecipients(next);
  };

  // Route a picked/scanned address to the split row that opened the picker, or to
  // the single-mode recipient field when none is targeted.
  const applyPickedAddress = (addr: string) => {
    if (pickerTarget) {
      setRecipients((prev) => prev.map((r) => (r.id === pickerTarget ? { ...r, address: addr } : r)));
    } else {
      setRecipient(addr);
    }
  };

  // ── ② multiSelect (多币一人 / 清空) ─────────────────────────────────────────────────
  // Selection logic lives in the shared `multiSelect` hook; here we just react to a
  // confirmed selection. Multi-select is gated on a chosen network (TokenSelector
  // only shows checkboxes once one is picked), so selection is always one chain.
  const pickedTokens = multiSelect.selectedTokens(tokens);

  // The exact per-token amounts a multiSelect submits: full balance for ERC-20s, and
  // the native coin minus a gas reserve so the EntryPoint prefund can be paid
  // (non-Tempo, no paymaster). Used by BOTH the simulation and the submit so the
  // preview matches what gets signed. The native line drops out if it can't even
  // cover the reserve.
  const multiTokenSpecs = (chainId: number) =>
    reserveNativeGas(
      toMultiTokenSpecs(pickedTokens),
      !isTempoChain(chainId) && feeEstimate ? feeEstimate.totalWei * 3n : 0n,
    );

  // Confirmed selection → advance. ONE token is a normal amount-send (not a
  // full-balance multiSelect); TWO+ is a multiSelect. The first token carries chain/gas context.
  const confirmSelection = () => {
    const selected = multiSelect.selectedTokens(tokens);
    if (selected.length === 0) return;
    if (selected.length === 1) {
      handleSelectToken(selected[0]);
      return;
    }
    setMultiSelectMode(true);
    setSelectedToken(selected[0]);
    setStep('enter-details');
    if (activeAccount) {
      const chainId = tokenChainId(selected[0]);
      prefetchForSend(activeAccount.address, chainId);
      fetchBundlerAccountInfo(chainId, activeAccount.address).catch(() => {});
      findAccountByCredentialId(activeAccount.id).then((s) => { prefetchedAccount.current = s ?? null; });
      import('@/services/webauthn-verify').then((m) => { webauthnModuleRef.current = m; });
      // Warm a gas estimate so the detail list can show the native line net of
      // its reserve right away (not just at confirm).
      estimateTransactionFee(activeAccount.address, chainId, gasTier)
        .then((f) => { if (mountedRef.current) setFeeEstimate(f); })
        .catch(() => {});
    }
  };


  const handleSelectToken = (token: APIToken) => {
    setMultiSelectMode(false); // single-token path — normal amount-send, not a multiSelect
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
    if (multiSelectMode) {
      if (!isValidAddress(recipient)) {
        showAlert(t('send.alertInvalidAddressTitle'), t('send.alertInvalidAddressBody'));
        return;
      }
      if (pickedTokens.length === 0) return;
    } else if (splitMode) {
      if (!recipientsAreValid(recipients)) {
        showAlert(t('send.alertInvalidAddressTitle'), t('send.alertInvalidAddressBody'));
        return;
      }
      if (selectedToken) {
        const totalBase = sumSplitBaseUnits(recipients, selectedToken.decimals);
        const balBase = toBaseUnits(selectedToken.balance || '0', selectedToken.decimals);
        if (totalBase > balBase) {
          showAlert(t('send.alertInsufficientBalanceTitle'), t('send.alertInsufficientBalanceBody', { defaultValue: 'The total exceeds your balance.' }));
          return;
        }
      }
    } else {
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

          // Compare against the bundler's raw gas cost (tier markup removed).
          const bundlerCost = fee ? rawBundlerGasCost(fee) : undefined;
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
    // Tap haptic fires on the one-tap VelaButton; the hold-to-confirm path
    // provides its own (Medium on press + Success on completion).

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
    setUserOpHash(null);
    setTxError(null);
    setReceiptFailed(false);
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
      const maxFee = feeEstimate?.maxFeePerGas;

      // One send line per output (single = 1, split = N recipients, multiSelect = N
      // tokens). split/multiSelect submit as a single Safe MultiSend UserOp — one
      // signature, one gas. Each line carries its own token so multiSelect's mixed-token
      // activity records (symbol/decimals/usd) are correct per line.
      let result;
      let lines: { to: string; toName?: string; amount: string; symbol: string; decimals: number; priceUsd: number; logoUrls?: string[] }[];
      if (multiSelectMode) {
        // Reserved specs = the exact amounts sent (native minus gas). Activity
        // lines are derived from them so each record shows what actually moved.
        const specs = multiTokenSpecs(chainId);
        if (specs.length === 0) {
          throw new Error(t('send.multiSendNoFundsAfterGas', { defaultValue: 'Not enough to cover gas after the reserve.' }));
        }
        const calls = buildMultiTokenCalls(recipient.trim(), specs);
        result = await sendBatchCalls(activeAccount.address, calls, chainId, stored.publicKeyHex, signFn);
        lines = specs.map((spec) => {
          const tk = pickedTokens.find((t) => (isNativeToken(t) ? null : t.tokenAddress) === spec.tokenAddress)!;
          return { to: recipient.trim(), toName: recipientIdentity?.name, amount: spec.amount, symbol: tk.symbol, decimals: tk.decimals, priceUsd: tk.priceUsd ?? 0, logoUrls: tokenLogoURLs(tk) };
        });
      } else if (splitMode) {
        const calls = buildSplitCalls(
          { tokenAddress: isNativeToken(selectedToken) ? null : selectedToken.tokenAddress, decimals: selectedToken.decimals },
          recipients.map((r) => ({ address: r.address.trim(), amount: r.amount })),
        );
        result = await sendBatchCalls(activeAccount.address, calls, chainId, stored.publicKeyHex, signFn);
        lines = recipients.map((r) => ({ to: r.address.trim(), amount: r.amount, symbol: selectedToken!.symbol, decimals: selectedToken!.decimals, priceUsd: selectedToken!.priceUsd ?? 0, logoUrls: tokenLogoURLs(selectedToken!) }));
      } else {
        const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
        const weiHex = amountToWeiHex(tokenAmount, selectedToken.decimals);
        if (isNativeToken(selectedToken)) {
          result = await sendNative(activeAccount.address, recipient, weiHex, chainId, stored.publicKeyHex, signFn, maxFee);
        } else {
          result = await sendERC20(activeAccount.address, selectedToken.tokenAddress!, recipient, weiHex, chainId, stored.publicKeyHex, signFn, maxFee);
        }
        lines = [{ to: recipient, toName: recipientIdentity?.name, amount: tokenAmount, symbol: selectedToken.symbol, decimals: selectedToken.decimals, priceUsd: selectedToken.priceUsd ?? 0, logoUrls: tokenLogoURLs(selectedToken) }];
      }

      // Feed the receipt the per-line breakdown for batch sends so it renders
      // "30 USDC → 3 recipients" / "3 assets → Bob" instead of a single (NaN) amount.
      // A plain single send stays null and uses the scalar amount/symbol props.
      if (multiSelectMode || splitMode) {
        setReceiptTransfers(lines.map((ln) => ({
          to: ln.to,
          toName: ln.toName,
          amount: ln.amount,
          symbol: ln.symbol,
          logoUrls: ln.logoUrls ?? [],
          usdValue: (parseFloat(ln.amount || '0') || 0) * ln.priceUsd,
        })));
        setReceiptKind(multiSelectMode ? 'multiSelect' : 'split');
      } else {
        setReceiptTransfers(null);
        setReceiptKind(null);
      }

      // Bundler accepted the UserOp — treat the payment as sent right now (we
      // have the userOpHash). The on-chain tx hash resolves in the background to
      // light up the explorer link; a slow/failed receipt poll must NOT turn a
      // submitted payment into an error.
      if (mountedRef.current) setUserOpHash(result.userOpHash);
      setTxStatus('confirmed');
      hapticSuccess(); // payment accepted by the bundler — distinct success buzz
      setSending(false);
      clearTokenCache(activeAccount.address);

      // One activity record per recipient. In a batch they share the userOpHash,
      // so each gets a distinct id (`<hash>-<i>`) to show as its own history line
      // and be patched independently when the on-chain hash lands. USD is captured
      // now so non-stablecoin sends (e.g. BNB) still render a fiat amount later.
      const ts = Math.floor(Date.now() / 1000);
      const records = lines.map((ln, i) => {
        const usd = parseFloat(ln.amount || '0') * ln.priceUsd;
        return {
          id: lines.length > 1 ? `${result.userOpHash}-${i}` : result.userOpHash,
          userOpHash: result.userOpHash,
          txHash: '',
          from: activeAccount!.address,
          to: ln.to,
          toName: ln.toName,
          value: ln.amount,
          symbol: ln.symbol,
          decimals: ln.decimals,
          logoUrls: ln.logoUrls,
          chainId,
          timestamp: ts,
          status: 'pending' as const,
          type: 'send' as const,
          usd: usd > 0 ? '$' + usd.toFixed(2) : undefined,
        };
      });
      // Persist ALL siblings in one atomic write. A per-record Promise.all would
      // race the read-modify-write and silently drop every sibling but one — which
      // collapsed a batch send to a single line in Activity.
      const recordIds = records.map((rec) => rec.id);
      const pendingWrites = saveTransactions(records).catch(() => {});

      // Resolve the on-chain hash in the background and flip every record to
      // 'confirmed' (awaiting the pending writes first so the patches find them).
      // A definitive drop/revert flips them to 'failed' so the receipt stamp and
      // the feed both show the real outcome; a transient/timeout stays 'pending'.
      result.waitForTxHash()
        .then(async (hash) => {
          if (mountedRef.current) setTxHash(hash);
          await pendingWrites;
          await updateTransactions(recordIds, { txHash: hash, status: 'confirmed' }).catch(() => {});
        })
        .catch(async (err) => {
          // Definitive failure (op dropped / reverted) vs. a slow/unreachable poll.
          // Only the former is a real failure; the latter stays pending (reconciled later).
          if (!/dropped from the network|reverted|failed/i.test(err?.message ?? '')) return;
          if (mountedRef.current) setReceiptFailed(true);
          await pendingWrites;
          await updateTransactions(recordIds, { status: 'failed' }).catch(() => {});
        });

    } catch (error: any) {
      // Wording-tolerant detection — the bundler has reworded this error before
      // (legacy "...bundler EOA" → current "...bundler gas account ... Deposit to:").
      const underfunded = parseBundlerUnderfunded(error?.message);
      if (error?.code === 'PASSKEY_CANCELLED') {
        setTxStatus('idle');
      } else if (underfunded) {
        // Bundler says the gas account balance is insufficient — show funding modal.
        // Prefer the server's actual required balance to avoid a threshold mismatch
        // (client estimate vs server's gas price) causing an infinite loop.
        fundingRetryCount.current += 1;
        if (fundingRetryCount.current > 3) {
          // Break infinite loop — show error instead of modal
          fundingRetryCount.current = 0;
          setTxError(t('send.txErrorBundlerLoop'));
          setTxStatus('error'); hapticError();
        } else {
          setTxStatus('idle');
          try {
            const chainId = tokenChainId(selectedToken!);
            clearBundlerCache(chainId, activeAccount!.address);
            const info = await fetchBundlerAccountInfo(chainId, activeAccount!.address);
            // User navigated away during the account lookup — don't touch UI state.
            if (!mountedRef.current) return;
            // Prefer live account info; fall back to values parsed from the error.
            const depositAddress = info?.depositAddress || underfunded.depositAddress;
            if (depositAddress) {
              const currentBalance = info?.spendableBalance ?? underfunded.spendableWei ?? 0n;
              let thresholdWei: bigint;
              if (underfunded.requiredWei != null) {
                thresholdWei = underfunded.requiredWei;
              } else {
                const fee = feeEstimate ?? await estimateTransactionFee(activeAccount!.address, chainId, gasTier);
                // Match the server's raw gasPrice calculation (tier markup removed)
                thresholdWei = rawBundlerGasCost(fee);
              }
              const recommendedWei = recommendedFundingWei(thresholdWei, currentBalance);
              const nativeSym = info?.nativeSym ?? (underfunded.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(chainId));
              setFundingNeeded({
                reason: 'deposit_needed',
                sponsorshipAvailable: true,
                depositAddress,
                safeAddress: activeAccount!.address,
                chainId,
                nativeSym,
                thresholdWei,
                recommendedWei,
                currentBalance,
                recommendedFormatted: formatWei(recommendedWei),
                currentFormatted: formatWei(currentBalance),
              });
              return;
            }
          } catch { /* fall through */ }
          setTxError(t('send.txErrorBundlerFund'));
          setTxStatus('error'); hapticError();
        }
      } else {
        // Never surface a raw RPC/library exception on the money-flow confirm
        // screen — it's unlocalized and jargon-filled. Log it for diagnostics and
        // show a calm, actionable, localized message instead.
        console.warn('[send] unhandled tx error:', error?.message ?? String(error));
        setTxError(t('send.txErrorGeneric'));
        setTxStatus('error'); hapticError();
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
      if (multiSelectMode) {
        // Back to the multi-select picker, preserving the multiSelect selection.
        setStep('select-token');
      } else {
        setSelectedToken(null);
        setAmount('');
        setRecipient('');
        setSplitMode(false);
        setRecipients([]);
        setStep('select-token');
      }
    } else {
      router.back();
    }
  };

  // Step 1: Select Token — delegated to the shared TokenSelector.
  // Multi-select is built-in now: filter to a specific network and the picker
  // shows checkboxes (one token = amount-send, two+ = multiSelect). No mode toggle.
  const tokenMultiSelect = {
    selectedIds: multiSelect.selectedIds,
    onToggle: multiSelect.toggle,
    onToggleAll: multiSelect.toggleAll,
    isAllSelected: multiSelect.isAllSelected,
    onNetworkChange: multiSelect.onNetworkChange,
    onConfirm: confirmSelection,
    confirmLabel: multiSelect.count === 1
      ? t('send.continueBtn')
      : t('send.multiSendContinue', { n: multiSelect.count, chain: multiSelect.chainId != null ? chainName(multiSelect.chainId) : '' }),
    selectAllLabel: t('send.selectAllValuable', { defaultValue: 'Select all valuable' }),
  };

  const renderSelectToken = () => (
    <Animated.View style={styles.stepContainer} entering={fadeInDown(0, 300)}>
      <Text style={styles.stepTitle}>{t('send.selectTokenTitle')}</Text>
      <TokenSelector
        tokens={tokens}
        loading={loading}
        onSelect={handleSelectToken}
        onAddChanged={refreshTokens}
        defaultCategory="stable"
        initialChainId={multiSelect.chainId}
        multiSelect={tokenMultiSelect}
      />
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
          <Text style={styles.stepTitle}>{multiSelectMode ? t('send.multiSendTitle') : t('send.sendTitle', { symbol: selectedToken.symbol })}</Text>

          {/* Token hero (single/split) — tap to switch token. Multi-select hides it. */}
          {!multiSelectMode && (
          <VelaCard style={styles.heroCard}>
            <Pressable style={styles.heroRow} disabled={locked} onPress={() => { setStep('select-token'); setSelectedToken(null); setAmount(''); setInputInUsd(false); setSplitMode(false); setRecipients([]); }}>
              <TokenLogo symbol={selectedToken.symbol} logoUrls={logos} chain={tokenBadgeNetwork(selectedToken)} size={44} />
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
            </Pressable>
            {!isNativeToken(selectedToken) && selectedToken.tokenAddress ? (
              <Pressable
                style={styles.contractRow}
                onPress={() => {
                  copyToClipboard(selectedToken.tokenAddress!);
                  setCopiedContract(true);
                  setTimeout(() => setCopiedContract(false), 1500);
                }}
                hitSlop={6}
              >
                <Text style={styles.contractLabel}>{t('addToken.tokenAddressLabel')}</Text>
                <Text style={styles.contractAddr} numberOfLines={1}>{shortAddr(selectedToken.tokenAddress)}</Text>
                {copiedContract
                  ? <Check size={14} color={color.success.base} strokeWidth={2.5} />
                  : <Copy size={14} color={color.fg.subtle} strokeWidth={2} />}
              </Pressable>
            ) : null}
          </VelaCard>
          )}

          {/* Single-recipient flow (default). Split / multiSelect replace it below. */}
          {!splitMode && !multiSelectMode && (<>
          {/* Amount — large display with inline unit */}
          <Pressable style={styles.amountWrap} onPress={() => { if (!amountLocked) amountInputRef.current?.focus(); }}>
            <View style={styles.amountTopRow}>
              <View style={styles.amountInputWrap}>
                <TextInput
                  ref={amountInputRef}
                  testID="amount-input"
                  style={[styles.amountInput, { fontSize: amountFontSize(amount) }]}
                  placeholder="0"
                  placeholderTextColor={color.fg.subtle}
                  value={amount}
                  editable={!amountLocked}
                  onChangeText={(t) => {
                    const maxDec = inputInUsd ? fiatDecimals : selectedToken.decimals;
                    const sanitized = sanitizeAmountInput(t, maxDec);
                    if (sanitized !== null) setAmount(sanitized);
                  }}
                  keyboardType="decimal-pad"
                  selectionColor={color.fg.muted}
                />
              </View>
              {amount || amountLocked ? (
                <Text style={[styles.unitLabel, { fontSize: Math.max(amountFontSize(amount || '0') * 0.7, 16) }]}>
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
          </View>
          <View style={styles.inputWrap}>
            <AutoGrowTextInput
              style={styles.input}
              minHeight={48}
              maxHeight={100}
              placeholder={t('send.recipientPlaceholder')}
              placeholderTextColor={color.fg.subtle}
              value={recipient}
              onChangeText={(t) => setRecipient(t)}
              autoCapitalize="none"
              autoCorrect={false}
              editable={!params.prefilledRecipient}
              blurOnSubmit
              returnKeyType="done"
            />
            {!params.prefilledRecipient && (
              <View style={styles.inputIcons}>
                {/* Address book — also the way in to the QR scanner (shown at the
                    top of the picker sheet). Big, comfortable tap target. */}
                <Pressable
                  onPress={() => { setPickerTarget(null); setShowContactPicker(true); }}
                  hitSlop={8}
                  style={styles.addrActionBtn}
                  accessibilityLabel={t('send.recipientPickAria', { defaultValue: 'Choose recipient or scan' })}
                >
                  <BookUser size={22} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
              </View>
            )}
          </View>

          {/* Recipient identity — one line, one name: a green check for a starred
              contact, else a neutral person icon for any resolved identity. */}
          <View style={{ marginTop: space.sm }}>
            <RecipientTrust address={recipient} identity={recipientIdentity} />
          </View>

          {/* Send this token to several people at once → enters split mode. */}
          {!locked && !params.prefilledRecipient && (
            <Pressable onPress={enterSplitMode} style={styles.addRecipientEntry}>
              <Plus size={16} color={color.accent.base} strokeWidth={2.5} />
              <Text style={styles.addRecipientEntryText}>{t('send.addRecipient', { defaultValue: 'Add recipient' })}</Text>
            </Pressable>
          )}
          </>)}

          {splitMode && selectedToken && (
            <MultiRecipientEditor
              recipients={recipients}
              onChange={handleRecipientsChange}
              tokenSymbol={selectedToken.symbol}
              decimals={selectedToken.decimals}
              priceUsd={selectedToken.priceUsd}
              balance={selectedToken.balance}
              formatUsd={formatUsd}
              onPickContact={(id) => { setPickerTarget(id); setShowContactPicker(true); }}
            />
          )}

          {/* ② multi-token send — exact amounts sent (native net of its gas reserve). */}
          {multiSelectMode && (() => {
            const cid = tokenChainId(selectedToken);
            const specs = multiTokenSpecs(cid);
            // Amount actually sent per token: ERC-20 = full balance; native = balance
            // minus its gas reserve (0 if the native line was dropped).
            const amountOf = (tk: APIToken) => {
              const addr = isNativeToken(tk) ? null : tk.tokenAddress;
              const spec = specs.find((s) => s.tokenAddress === addr);
              return spec ? parseFloat(spec.amount) : 0;
            };
            const total = pickedTokens.reduce((s, tk) => s + amountOf(tk) * (tk.priceUsd ?? 0), 0);
            return (<>
            <VelaCard style={styles.multiCard}>
              <View style={styles.mtSummary}>
                <Text style={styles.mtSummaryTitle}>
                  {t('send.multiSendSummary', { n: pickedTokens.length, chain: chainName(cid) })}
                </Text>
                <Text style={styles.mtSummaryUsd}>{formatUsd(total)}</Text>
              </View>
              {pickedTokens.map((tk) => {
                const amt = amountOf(tk);
                const usd = amt * (tk.priceUsd ?? 0);
                const reserved = isNativeToken(tk) && amt < tokenBalanceDouble(tk);
                return (
                  <View key={tokenId(tk)} style={[styles.mtRow, styles.mtRowBorder]}>
                    <TokenLogo symbol={tk.symbol} logoUrls={tokenLogoURLs(tk)} chain={tokenBadgeNetwork(tk)} size={32} />
                    <View style={styles.mtInfo}>
                      <Text style={styles.mtSym}>{tk.symbol}</Text>
                      <Text style={styles.mtChain}>
                        {chainName(tokenChainId(tk))}{reserved ? ` · ${t('send.gasReserved')}` : ''}
                      </Text>
                    </View>
                    <View style={styles.mtVals}>
                      <Text style={styles.mtBal}>{formatTokenAmount(amt, { compact: true })}</Text>
                      {usd > 0 && <Text style={styles.mtUsd}>{formatUsd(usd)}</Text>}
                    </View>
                  </View>
                );
              })}
            </VelaCard>

            <View style={[styles.fieldLabelRow, { marginTop: space.xl }]}>
              <Text style={styles.fieldLabel}>{t('send.recipientLabel')}</Text>
            </View>
            <View style={styles.inputWrap}>
              <AutoGrowTextInput
                style={styles.input}
                minHeight={48}
                maxHeight={100}
                placeholder={t('send.recipientPlaceholder')}
                placeholderTextColor={color.fg.subtle}
                value={recipient}
                onChangeText={(t) => setRecipient(t)}
                autoCapitalize="none"
                autoCorrect={false}
                blurOnSubmit
                returnKeyType="done"
              />
              <View style={styles.inputIcons}>
                <Pressable onPress={() => { setPickerTarget(null); setShowContactPicker(true); }} hitSlop={8} style={styles.addrActionBtn}>
                  <BookUser size={22} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
              </View>
            </View>
            <View style={{ marginTop: space.sm }}>
              <RecipientTrust address={recipient} identity={recipientIdentity} />
            </View>
          </>);
          })()}

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
            disabled={(splitMode ? !recipientsAreValid(recipients) : multiSelectMode ? (!isValidAddress(recipient) || pickedTokens.length === 0) : (!recipient || !amount)) || estimatingGas || fundingNeeded?.reason === 'wallet_balance_too_low' || (locked && !!amountWarning)}
          />
        </Animated.View>
      </ScrollView>
    );
  };

  // Step 3: Confirm
  const renderConfirm = () => {
    if (!selectedToken) return null;
    const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
    const singleAmountNum = parseFloat(tokenAmount || '0');
    // In split mode the headline amount is the sum across all recipients.
    const splitTotalNum = splitMode
      ? parseFloat(fromBaseUnits(sumSplitBaseUnits(recipients, selectedToken.decimals), selectedToken.decimals))
      : 0;
    const amountNum = splitMode ? splitTotalNum : singleAmountNum;
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

            {/* Line + Token(s) — one token, or every swept token in multiSelect mode */}
            <View style={styles.transferMiddle}>
              <View style={styles.transferLineCol}>
                <View style={styles.transferLine} />
              </View>
              {!multiSelectMode ? (
                <View style={styles.transferToken}>
                  <TokenLogo symbol={selectedToken.symbol} logoUrls={logos} chain={tokenBadgeNetwork(selectedToken)} size={36} />
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
              ) : (
                (() => {
                  const specs = multiTokenSpecs(tokenChainId(selectedToken));
                  const amountOf = (tk: APIToken) => {
                    const addr = isNativeToken(tk) ? null : tk.tokenAddress;
                    const spec = specs.find((s) => s.tokenAddress === addr);
                    return spec ? parseFloat(spec.amount) : 0;
                  };
                  return (
                    <View style={styles.mtConfirmList}>
                      {pickedTokens.map((tk) => {
                        const amt = amountOf(tk);
                        const usd = amt * (tk.priceUsd ?? 0);
                        const reserved = isNativeToken(tk) && amt < tokenBalanceDouble(tk);
                        return (
                          <View key={tokenId(tk)} style={styles.transferToken}>
                            <TokenLogo symbol={tk.symbol} logoUrls={tokenLogoURLs(tk)} chain={tokenBadgeNetwork(tk)} size={36} />
                            <View style={styles.transferTokenIdentity}>
                              <Text style={styles.transferTokenSymbol}>{tk.symbol}</Text>
                              <Text style={styles.transferTokenChain}>
                                {chainName(tokenChainId(tk))}{reserved ? ` · ${t('send.gasReserved')}` : ''}
                              </Text>
                            </View>
                            <View style={styles.transferTokenValues}>
                              <Text style={styles.transferTokenAmount}>{formatBalance(amt)}</Text>
                              {usd > 0 && (
                                <Text style={styles.transferTokenSub}>≈ {formatUsd(usd)}</Text>
                              )}
                            </View>
                          </View>
                        );
                      })}
                    </View>
                  );
                })()
              )}
            </View>

            {/* To — one recipient, or the full list in split mode */}
            {!splitMode ? (
              <View style={styles.transferEndpoint}>
                <Text style={styles.transferLabel}>{t('send.toLabel')}</Text>
                <View style={{ marginBottom: 2 }}>
                  <RecipientTrust address={recipient} identity={recipientIdentity} prominent />
                </View>
                <Text style={styles.transferAddr}>{shortAddr(recipient)}</Text>
                {(recipientRisk?.firstInteraction || recipientRisk?.isContract === true) && (
                  <View style={styles.riskTagRow}>
                    {recipientRisk?.firstInteraction && (
                      <Text style={[styles.riskTag, styles.riskTagWarn]}>{t('componentsUi.signing.firstTimeTag')}</Text>
                    )}
                    {recipientRisk?.isContract === true && (
                      <Text style={styles.riskTag}>{t('componentsUi.signing.contractTag')}</Text>
                    )}
                  </View>
                )}
              </View>
            ) : (
              <View style={styles.transferEndpoint}>
                <Text style={styles.transferLabel}>
                  {t('send.toLabel')} · {t('send.recipientCount', { n: recipients.length, defaultValue: `${recipients.length} recipients` })}
                </Text>
                {recipients.map((r) => {
                  const n = parseFloat(r.amount || '0');
                  const u = n * (selectedToken.priceUsd ?? 0);
                  return (
                    <View key={r.id} style={styles.confirmRecipientRow}>
                      <View style={styles.confirmRecipientLeft}>
                        <Text style={styles.transferAddr}>{shortAddr(r.address)}</Text>
                        <RecipientTrust address={r.address} compact />
                      </View>
                      <View style={styles.confirmRecipientAmt}>
                        <Text style={styles.transferTokenAmount}>{formatBalance(n)} {selectedToken.symbol}</Text>
                        {u > 0 && <Text style={styles.transferTokenSub}>≈ {formatUsd(u)}</Text>}
                      </View>
                    </View>
                  );
                })}
              </View>
            )}
          </VelaCard>

          {/* Simulation — revert pre-check + net balance changes (shared render
              path with the dApp signing sheet). */}
          <BalanceChangePreview
            result={sim}
            chainId={tokenChainId(selectedToken)}
            selfTransfer={!!activeAccount && recipient.toLowerCase() === activeAccount.address.toLowerCase()}
          />

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
            const tempo = isTempoChain(tokenChainId(selectedToken));
            // Tempo: the "gas price" is attodollars/gas (USD-denominated, protocol-fixed),
            // not Gwei of a native coin. Show a USD rate; hide the meaningless speed tiers.
            const tempoGasUsdPer1M = Number(feeEstimate.bundlerGasPrice) / 1e12;

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
                {/* Speed tiers — hidden on Tempo: gas price is a fixed protocol rate with
                    no priority-fee market, so the tiers do nothing. */}
                {!tempo && (
                  <>
                    <View style={styles.tierRow}>
                      {(['slow', 'standard', 'rapid', 'fast'] as GasTier[]).map((tier) => (
                        <Pressable
                          key={tier}
                          style={[styles.tierBtn, gasTier === tier && styles.tierBtnActive]}
                          onPress={() => handleTierChange(tier)}
                          accessibilityRole="button"
                          accessibilityState={{ selected: gasTier === tier }}
                          accessibilityLabel={t(`send.gasTier.${tier}`)}
                        >
                          <Text style={[styles.tierBtnText, gasTier === tier && styles.tierBtnTextActive]}>
                            {t(`send.gasTier.${tier}`)}
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
                  </>
                )}
                {tempo ? (
                  <ConfirmRow label={t('send.gasPriceLabel')} value={`$${tempoGasUsdPer1M.toFixed(2)} / 1M gas`} />
                ) : (
                  <>
                    <ConfirmRow label={t('send.gasPriceLabel')} value={`${bundlerGwei.toFixed(4)} Gwei`} />
                    <View style={styles.confirmSeparator} />
                    <ConfirmRow label={t('send.gasPriceUserOpLabel')} value={`${userOpGwei.toFixed(4)} Gwei`} />
                  </>
                )}
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.gasLimitLabel')} value={feeEstimate.totalGas.toLocaleString()} />
                <View style={styles.confirmSeparator} />
                <ConfirmRow label={t('send.walletDeployedLabel')} value={feeEstimate.deployed ? t('send.walletDeployedYes') : t('send.walletDeployedNo')} />
              </VelaCard>
            );
          })()}

          {txStatus === 'idle' && (
            // Every send is a deliberate slide-to-confirm — a stray tap can't fire
            // a payment. A risky destination (never sent here before, or a contract)
            // turns the slide red and shows the first-time / contract tags above.
            <SlideToConfirmButton
              title={estimatingGas ? t('send.checkingGas') : t('send.confirmSendBtn')}
              hint={t('componentsUi.signing.slideToConfirm', { defaultValue: 'Slide to confirm' })}
              onConfirm={handleConfirm}
              loading={sending}
              disabled={estimatingGas}
              tone={(recipientRisk?.firstInteraction || recipientRisk?.isContract === true) ? 'danger' : 'accent'}
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

  // Exception screen for a scanned EIP-681 request Vela can't fulfil as-is.
  const renderLockError = () => {
    if (!lockError) return null;
    if (lockError.kind === 'network') {
      return (
        <View style={styles.lockErrorWrap}>
          <View style={styles.lockErrorIcon}><Globe size={30} color={color.accent.base} strokeWidth={2} /></View>
          <Text style={styles.lockErrorTitle}>{t('send.lock.netTitle')}</Text>
          <Text style={styles.lockErrorBody}>{t('send.lock.netBody', { chainId: lockError.chainId })}</Text>
          {addNetworkMsg ? <Text style={styles.lockErrorMsg}>{addNetworkMsg}</Text> : null}
          <VelaButton
            title={t('send.lock.addNetwork')}
            onPress={() => handleAddNetwork(lockError.chainId)}
            loading={addingNetwork}
            style={styles.lockErrorBtn}
          />
          <Pressable onPress={() => router.back()} hitSlop={8}>
            <Text style={styles.lockErrorCancel}>{t('common.cancel')}</Text>
          </Pressable>
        </View>
      );
    }
    return (
      <View style={styles.lockErrorWrap}>
        <View style={styles.lockErrorIcon}><AlertCircle size={30} color={color.accent.base} strokeWidth={2} /></View>
        <Text style={styles.lockErrorTitle}>{t('send.lock.tokenTitle')}</Text>
        <Text style={styles.lockErrorBody}>{t('send.lock.tokenBody')}</Text>
        <Pressable onPress={() => router.back()} hitSlop={8}>
          <Text style={styles.lockErrorCancel}>{t('common.cancel')}</Text>
        </Pressable>
      </View>
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
      {lockError ? (
        renderLockError()
      ) : (locked && resolvingLock && !selectedToken) ? (
        <View style={styles.lockLoading}><ActivityIndicator color={color.accent.base} /></View>
      ) : txStatus === 'confirmed' && selectedToken ? (
        <TransactionReceipt
          from={activeAccount?.address ?? ''}
          fromName={activeAccount?.name}
          to={recipient}
          toName={recipientIdentity?.name}
          amount={resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate)}
          symbol={selectedToken.symbol}
          chainId={tokenChainId(selectedToken)}
          txHash={txHash ?? ''}
          userOpHash={userOpHash ?? undefined}
          logoUrls={tokenLogoURLs(selectedToken)}
          usdValue={parseFloat(resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate)) * (selectedToken.priceUsd ?? 0)}
          rate={dc.rate}
          currencyCode={dc.code}
          currencySymbol={dc.symbol}
          timestamp={new Date()}
          recipientIdentity={recipientIdentity}
          transfers={receiptTransfers ?? undefined}
          batchKind={receiptKind ?? undefined}
          status={receiptFailed ? 'failed' : (txHash ? 'confirmed' : 'submitted')}
          onDone={() => router.back()}
          onSaveContact={receiptKind === 'split' ? undefined : () => saveContact({ address: recipient, name: recipientIdentity?.name, resolvedName: recipientIdentity?.name })}
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
        onScan={(data) => {
          setShowScanner(false);
          const req = parseEIP681(data);
          // Per-row scan in split mode — just take the address; a full-request
          // re-lock would blow away the other recipients.
          if (pickerTarget) {
            applyPickedAddress(req?.recipient ?? data);
            return;
          }
          // A full EIP-681 request re-opens Send locked; otherwise just take the address.
          if (req && req.chainId != null) {
            const p: Record<string, string> = {
              prefilledRecipient: req.recipient,
              prefilledChainId: String(req.chainId),
              locked: '1',
            };
            if (req.tokenAddress) p.prefilledTokenAddress = req.tokenAddress;
            if (req.amountBaseUnits != null) p.prefilledAmountBase = req.amountBaseUnits.toString();
            router.replace({ pathname: '/send', params: p });
            return;
          }
          setRecipient(req?.recipient ?? data);
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

      <ContactPicker
        visible={showContactPicker}
        onClose={() => setShowContactPicker(false)}
        onSelect={(addr) => applyPickedAddress(addr)}
        onScan={locked ? undefined : () => setShowScanner(true)}
        myAddress={address}
      />
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
    marginBottom: space.md,
  },
  searchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,

  // Category + network filters
  filterRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    marginBottom: space.xl,
  },
  chipScroll: {
    flex: 1,
  },
  chipRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingRight: space.sm,
  },
  chip: {
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  chipActive: {
    backgroundColor: color.accent.soft,
    borderColor: color.accent.base,
  },
  chipText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  chipTextActive: {
    color: color.accent.base,
  },

  // Count + total summary above the list
  summaryRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space.sm,
    marginBottom: space.md,
  },
  summaryCount: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  summaryTotal: {
    fontSize: text.sm,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },

  // Add-token affordance (footer + empty state)
  addTokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    alignSelf: 'stretch', // full width in both the list footer and the centered empty state
    gap: space.sm,
    paddingVertical: space.xl,
    marginTop: space.lg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    borderStyle: 'dashed',
    backgroundColor: color.bg.raised,
  },
  addTokenText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
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
  // ERC-20 contract address (tap to copy)
  contractRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.lg,
    paddingTop: space.lg,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
  },
  contractLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  contractAddr: {
    flex: 1,
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
    textAlign: 'right',
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
  addrActionBtn: {
    width: 40,
    height: 40,
    borderRadius: radius.md,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: color.bg.base,
  },
  // "+ 添加收款人" entry → split mode
  addRecipientEntry: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    marginTop: space.lg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    borderStyle: 'dashed',
  },
  addRecipientEntryText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
  // Split-mode confirm: one row per recipient
  confirmRecipientRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: space.md,
    paddingVertical: space.sm,
  },
  confirmRecipientLeft: { flex: 1, gap: 2 },
  confirmRecipientAmt: { alignItems: 'flex-end' },
  // ② multi-token send (multi-select → one recipient). Card owns the horizontal
  // padding (VelaCard has none); inner rows are flush so dividers align cleanly.
  multiCard: { marginBottom: space.lg, paddingHorizontal: space.xl, paddingVertical: space.xs },
  mtSummary: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingTop: space.md,
    paddingBottom: space.md,
  },
  mtSummaryTitle: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted, flex: 1 },
  mtSummaryUsd: { fontSize: text.lg, ...inter.bold, fontFamily: font.numeric, color: color.fg.base },
  mtRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.lg,
  },
  mtRowBorder: { borderTopWidth: 1, borderTopColor: color.border.base },
  mtInfo: { flex: 1, gap: 1 },
  mtSym: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  mtChain: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  mtVals: { alignItems: 'flex-end' },
  mtBal: { fontSize: text.base, ...inter.semibold, fontFamily: font.numeric, color: color.fg.base },
  mtUsd: { fontSize: text.sm, ...inter.regular, fontFamily: font.numeric, color: color.fg.muted },
  mtConfirmList: { flex: 1, gap: space.md },
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
  // Recipient-risk tags (first-time / contract) — mirrors the dApp signing sheet.
  riskTagRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: space.sm,
    marginTop: space.sm,
  },
  riskTag: {
    fontSize: 9,
    ...inter.semibold,
    color: color.fg.subtle,
    backgroundColor: color.bg.sunken,
    overflow: 'hidden',
    paddingHorizontal: 5,
    paddingVertical: 1,
    borderRadius: radius.sm,
    textTransform: 'uppercase',
    letterSpacing: 0.3,
  },
  riskTagWarn: {
    color: color.warning.base,
    backgroundColor: color.warning.soft,
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
    backgroundColor: color.warning.base,
  },
  txConfirmHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.sm,
  },

  // EIP-681 lock states
  lockLoading: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
  lockErrorWrap: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: space['3xl'],
    gap: space.lg,
  },
  lockErrorIcon: {
    width: 64,
    height: 64,
    borderRadius: 32,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  lockErrorTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
  },
  lockErrorBody: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 22,
  },
  lockErrorMsg: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.error.base,
    textAlign: 'center',
  },
  lockErrorBtn: {
    alignSelf: 'stretch',
    marginTop: space.md,
  },
  lockErrorCancel: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
    padding: space.md,
  },
}));
