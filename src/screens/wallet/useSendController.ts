import { makeRecipientId, recipientsAreValid, type RecipientDraft } from '@/components/send/MultiRecipientEditor';
import { type ReceiptTransfer } from '@/components/ui/TransactionReceipt';
import { amountToWeiHex, balanceToWei, encErc20Transfer, isValidAddress, synthErc20Token, synthNativeToken } from './send-utils';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useTokenMultiSelect } from '@/hooks/use-token-multi-select';
import { chainName, nativeSymbol, networkForChainId } from '@/models/network';
import { isNativeToken, tokenBalanceDouble, tokenChainId, tokenId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import * as Passkey from '@/modules/passkey';
import { addCustomNetworkByChainId } from '@/services/add-network';
import { buildMultiTokenCalls, buildSplitCalls, maxNativeSendable, reserveNativeGas, reserveTempoFeeToken, sumSplitBaseUnits, toMultiTokenSpecs } from '@/services/batch-send';
import { probeTreasury, parseBundlerUnderfunded, type TreasuryStatus } from '@/services/bundler-service';
import { fromBaseUnits, toBaseUnits } from '@/services/eip681';
import { resolveTokenAmount } from '@/services/fiat-convert';
import { fromHex, toHex } from '@/services/hex';
import { useLocalePrefs } from '@/services/locale-format';
import { hapticError, hapticSuccess, showAlert } from '@/services/platform';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { resolveRecipientRisk, type RecipientRisk } from '@/services/recipient-risk';
import { createReentryLock } from '@/services/reentry-lock';
import { estimateTransactionFee, prefetchForSend, sendBatchCalls, sendERC20, sendNative, type TransactionFeeEstimate } from '@/services/safe-transaction';
import { findAccountByCredentialId, saveTransactions, updateTransactions } from '@/services/storage';
import { isTempoChain, isTempoFeeToken, TEMPO_DEFAULT_FEE_TOKEN, TEMPO_FEE_TOKEN_DECIMALS } from '@/services/tempo';
import { resolveTokenMetadata } from '@/services/token-metadata';
import { simulateAssetChanges, type AssetSimResult } from '@/services/tx-simulation';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { useLocalSearchParams } from 'expo-router';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { TextInput } from 'react-native';

type Step = 'select-token' | 'enter-details' | 'confirm';
type TxStatus = 'idle' | 'preparing' | 'signing' | 'submitting' | 'confirming' | 'confirmed' | 'error';

/**
 * All Send-flow state, refs, effects, and handlers. Extracted verbatim from
 * SendScreen so the screen file holds only view wiring. Returns everything the
 * step views and the screen shell consume.
 */
export function useSendController() {
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
    // Multi-token hand-off: comma-joined tokenId()s land Send in multiSelect
    // mode. (No in-app producer since the Home assets sheet was retired; kept
    // as a param entry into the sweep flow.)
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
  // Synchronous mirror of `step` for guards inside long-running async flows
  // (the Phase-2 sponsorship grant can take ~20s — the user may back out of
  // the confirm screen meanwhile, and a passkey prompt must NOT resurrect).
  const stepRef = useRef(step);
  useEffect(() => { stepRef.current = step; }, [step]);

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
  // shared hook. `multiSelectMode` = we're in the
  // multiSelect enter-details/confirm flow (set when a multi-selection is confirmed).
  const [multiSelectMode, setMultiSelectMode] = useState(false);
  const multiSelect = useTokenMultiSelect();
  const [sending, setSending] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const [copiedContract, setCopiedContract] = useState(false);
  const [feeEstimate, setFeeEstimate] = useState<TransactionFeeEstimate | null>(null);
  const [estimatingGas, setEstimatingGas] = useState(false);
  // Single-flight re-entry lock with a generation token. A cancelled send
  // releases it immediately (so a retry isn't a silent no-op) while the cancelled
  // promise's stale `end()` must not clear a newer send's lock (issue #91).
  const sendLock = useRef(createReentryLock()).current;
  // Set by the confirm screen's cancel button; checked after every pre-sign
  // await in executeTransaction (mirrors dapp-connection's signCancelledRef).
  const sendCancelledRef = useRef(false);
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
  // Speed tiers are gone — every estimate/submit runs at 'fast'. What the user
  // CAN choose (on in-band chains with a DEX) is the fee ASSET: null = native,
  // else a whitelisted stablecoin contract. Options load when confirm opens;
  // null options = no selector (legacy chain, or Tempo where pathUSD is fixed).
  const [gasFeeToken, setGasFeeToken] = useState<string | null>(null);
  // Treasury bootstrap sheet (relayer float depleted on this network) — shown
  // instead of the generic error/funding surface when the treasury reports
  // bootstrapNeeded. See maybeShowTreasuryBootstrap.
  const [treasuryBootstrap, setTreasuryBootstrap] = useState<TreasuryStatus | null>(null);
  // GasFeeCard fires this while it re-quotes internally (fee-asset switch / refresh),
  // so the confirm slide stays disabled until the displayed quote is settled.
  const [feeBusy, setFeeBusy] = useState(false);
  const [showContactPicker, setShowContactPicker] = useState(false);
  const [showBatchImport, setShowBatchImport] = useState(false);
  const [amountWarning, setAmountWarning] = useState<string | null>(null);
  // Every chain settles gas in-band (native coin or a whitelisted stablecoin selected on
  // the confirm screen), so the amount step must never require a separate native-gas balance.
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

        // Multi-token hand-off via params → land in multiSelect mode.
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
              findAccountByCredentialId(activeAccount.id).then((s) => {
                prefetchedAccount.current = s ?? null;
                return estimateTransactionFee(
                  activeAccount.address, chainId, 'fast', undefined, undefined, gasFeeToken, s?.publicKeyHex,
                );
              })
                .then((f) => { if (mountedRef.current) setFeeEstimate(f); })
                .catch(() => {});
              import('@/services/webauthn-verify').then((m) => { webauthnModuleRef.current = m; });
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
      // Generic in-band chain: the gas asset is
      // chosen on the confirm screen's fee-token selector — the native coin OR a whitelisted
      // stablecoin the user holds — so the native-coin "insufficient / need <native>" warnings
      // don't apply here. The selector enforces the chosen fee asset's sufficiency; only the
      // "not enough of the token being sent" check above still gates this step.
      setAmountWarning(null);
      return;
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

  // Leaving confirm resets the fee-asset choice (next entry re-quotes in native) and clears a
  // stale erc20 estimate (totalWei=0n) so the gas-reserve/warning math downstream never reads 0.
  useEffect(() => {
    if (step !== 'confirm') {
      setGasFeeToken(null);
      setFeeEstimate((fe) => (fe?.feeAsset?.kind === 'erc20' ? null : fe));
    }
  }, [step]);

  // Resolve the bootstrap state without changing the UI. The send preflight uses
  // this so the relayer-funding sheet is presented at the same "before sending"
  // point as the personal gas-account funding sheet — including on in-band
  // chains, which otherwise skip the latter gate entirely.
  const getTreasuryBootstrap = async (chainId: number): Promise<TreasuryStatus | null> => {
    try {
      if (!activeAccount) return null;
      // The bundler's treasury endpoint is the authority (works for ANY chain the bundler serves,
      // incl. custom / local nets — no isInBandChain gate that mislabels them). A low-float
      // treasury returns its status; a legacy/uncovered chain 404s (no relayer treasury) → fall
      // through so the normal self-fund deposit path is preserved. Transient errors never route.
      const probe = await probeTreasury(chainId);
      if (probe.kind === 'low-float') return probe.status;
    } catch { /* fall back to the caller's default surface */ }
    return null;
  };

  // Relayer float depleted → offer the community bootstrap sheet instead of a
  // dead-end error/funding surface. Returns true when the sheet was shown.
  const maybeShowTreasuryBootstrap = async (chainId: number): Promise<boolean> => {
    const status = await getTreasuryBootstrap(chainId);
    if (status && mountedRef.current) {
      setTreasuryBootstrap(status);
      return true;
    }
    return false;
  };

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

  // A batch import (payroll table) or a whole-group pick seeds split mode directly
  // with the resolved recipient rows — same submission path as a hand-built split.
  const seedSplitRecipients = (rows: RecipientDraft[]) => {
    if (rows.length === 0) return;
    setInputInUsd(false);
    setRecipients(rows);
    setSplitMode(true);
    setShowBatchImport(false);
    setShowContactPicker(false);
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
  const multiTokenSpecs = (chainId: number) => {
    const specs = toMultiTokenSpecs(pickedTokens);
    if (isTempoChain(chainId)) {
      // Tempo pays gas from the pathUSD balance being swept, so trim that line — reserveNativeGas
      // can't (pathUSD is a non-null-address TIP-20). Reserve 2× the quoted fee: the sweep batches
      // more sub-calls than the 2-sub-call quote and may also deploy the Safe, so the live
      // reimbursement runs higher; over-reserving a little pathUSD is harmless.
      const feeUnits = feeEstimate ? feeEstimate.totalWei / 10n ** BigInt(18 - TEMPO_FEE_TOKEN_DECIMALS) : 0n;
      return reserveTempoFeeToken(specs, TEMPO_DEFAULT_FEE_TOKEN, feeUnits * 2n);
    }
    return reserveNativeGas(specs, feeEstimate ? feeEstimate.totalWei * 3n : 0n);
  };

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
      findAccountByCredentialId(activeAccount.id).then((s) => {
        prefetchedAccount.current = s ?? null;
        return estimateTransactionFee(
          activeAccount.address, chainId, 'fast', undefined, undefined, gasFeeToken, s?.publicKeyHex,
        );
      })
        .then((f) => { if (mountedRef.current) setFeeEstimate(f); })
        .catch(() => {});
      import('@/services/webauthn-verify').then((m) => { webauthnModuleRef.current = m; });
      // Warm a gas estimate so the detail list can show the native line net of
      // its reserve right away (not just at confirm).
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
      let storedForEstimate: { publicKeyHex: string } | null;
      try {
        storedForEstimate = prefetchedAccount.current
          ?? await findAccountByCredentialId(activeAccount.id)
          ?? null;
      } catch {
        showAlert(
          t('send.alertEstimateFailedTitle', { defaultValue: 'Could not prepare transaction' }),
          t('send.alertAccountUnavailableBody', { defaultValue: 'Could not load the account key required to prepare this transaction. Please try again.' }),
        );
        return;
      }
      prefetchedAccount.current = storedForEstimate ?? null;
      if (!storedForEstimate?.publicKeyHex) {
        showAlert(
          t('send.alertEstimateFailedTitle', { defaultValue: 'Could not prepare transaction' }),
          t('send.alertAccountUnavailableBody', { defaultValue: 'Could not load the account key required to prepare this transaction. Please try again.' }),
        );
        return;
      }
      if (!webauthnModuleRef.current) {
        import('@/services/webauthn-verify').then(m => { webauthnModuleRef.current = m; });
      }

      // Estimate gas + check the relayer treasury BEFORE advancing to confirm.
      // A depleted relayer opens TreasuryBootstrapSheet here, replacing the
      // personal gas-account funding sheet entirely.
      setEstimatingGas(true);
      setFeeEstimate(null);

      try {
        // The account context and estimate are mandatory. A timeout is surfaced
        // as an error; never continue with a fabricated UserOperation preview.
        // The REAL call for the charge basis: in-band displayed = signed, so this
        // estimate must price the actual send, not the padded rough model (which
        // over-charged ~8× on Arbitrum). Build the ACTUAL send/batch shape so in-band pricing
        // (estimateInBandBasisGas) sees the real calldata; the fee-reserve amounts don't affect
        // the gas SHAPE, so batch modes use the raw transfer legs (no circular fee dependency).
        let estTx: { to: string; value?: string; data?: string } | undefined;
        let estBatch: { to: string; value?: string; data?: string }[] | undefined;
        try {
          if (multiSelectMode) {
            estBatch = buildMultiTokenCalls(recipient.trim(), toMultiTokenSpecs(pickedTokens));
          } else if (splitMode) {
            estBatch = buildSplitCalls(
              { tokenAddress: isNativeToken(selectedToken!) ? null : selectedToken!.tokenAddress, decimals: selectedToken!.decimals },
              recipients.map((r) => ({ address: r.address.trim(), amount: r.amount })),
            );
          } else if (selectedToken && amount && isValidAddress(recipient)) {
            const tokenAmt = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
            const weiHex = amountToWeiHex(tokenAmt, selectedToken.decimals);
            estTx = isNativeToken(selectedToken)
              ? { to: recipient.trim(), value: weiHex }
              : { to: selectedToken.tokenAddress!, data: encErc20Transfer(recipient.trim(), weiHex) };
          }
        } catch {
          // A half-typed amount/recipient → fall back to the rough basis for this estimate.
          estTx = undefined;
          estBatch = undefined;
        }
        const preCheck = async (): Promise<TreasuryStatus | null> => {
          const [fee, bootstrapStatus] = await Promise.all([
            estimateTransactionFee(
              activeAccount!.address, chainId, 'fast', estTx, estBatch, gasFeeToken,
              storedForEstimate.publicKeyHex,
            ),
            // Do not inspect the user's personal gas account. The sole send
            // gate is the relayer treasury, and a low float replaces the old
            // "发送前，还差一步" sheet at this exact point in the flow.
            getTreasuryBootstrap(chainId),
          ]);
          setFeeEstimate(fee);
          return bootstrapStatus;
        };
        const timeout = new Promise<never>((_, reject) => setTimeout(
          () => reject(new Error('Could not estimate gas in time. Please try again.')), 15_000,
        ));
        const bootstrapStatus = await Promise.race([preCheck(), timeout]);
        if (bootstrapStatus && mountedRef.current) {
          setTreasuryBootstrap(bootstrapStatus);
          setEstimatingGas(false);
          return;
        }
      } catch (err) {
        setEstimatingGas(false);
        showAlert(
          t('send.alertEstimateFailedTitle', { defaultValue: 'Could not prepare transaction' }),
          err instanceof Error ? err.message : t('send.alertEstimateFailedBody', { defaultValue: 'Could not build a valid transaction estimate. Please try again.' }),
        );
        return;
      }

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
        const fee = feeEstimate ?? await estimateTransactionFee(
          activeAccount.address, chainId, 'fast', undefined, undefined, gasFeeToken,
          prefetchedAccount.current?.publicKeyHex,
        );
        // Use string-based conversion to avoid floating-point precision loss
        const balanceWei = balanceToWei(selectedToken.balance, selectedToken.decimals);
        // Reserve 3x estimated gas (200% margin for gas price volatility)
        const reserveWei = fee.totalWei * 3n;
        // String-exact `balance − reserve` (no float precision loss). Matches the
        // Tempo fee-token branch below and reserveNativeGas in batch-send.ts, so
        // amountWei + reserveWei === balanceWei exactly and the "insufficient for
        // gas" pre-check no longer trips on its own Max fill. Returns '0' when the
        // balance can't cover the gas reserve.
        setAmount(maxNativeSendable(balanceWei, reserveWei, selectedToken.decimals));
        return;
      } catch {
        // Estimation failed — fall through to full balance (tx may fail but user sees the error)
      }
    }

    // Tempo fee token (pathUSD): unlike a normal ERC-20, gas is paid FROM this balance via
    // the reimbursement transfer batched into the UserOp — so Max must leave enough to cover
    // it, exactly like a native coin. Gated on isTempoFeeToken: a NON-fee TIP-20 pays gas from
    // the SEPARATE pathUSD balance, so its Max stays full balance (falls through below).
    if (isTempoFeeToken(tokenChainId(selectedToken), selectedToken.tokenAddress) && activeAccount) {
      try {
        const chainId = tokenChainId(selectedToken);
        const fee = feeEstimate ?? await estimateTransactionFee(
          activeAccount.address, chainId, 'fast', undefined, undefined, undefined,
          prefetchedAccount.current?.publicKeyHex,
        );
        // fee.totalWei is attodollars (USD×1e-18); recover the pathUSD reimbursement (6 dec).
        const feeUnits = fee.totalWei / 10n ** BigInt(18 - TEMPO_FEE_TOKEN_DECIMALS);
        // Reserve 1.5× the fee: +50% for gas-price/estimate variance (the send-time
        // reimbursement is re-priced off the bundler's live estimate and may exceed this quote,
        // especially when this send also deploys the Safe). Leaves a margin so the pre-check clears.
        const reserveUnits = (feeUnits * 3n) / 2n;
        const balUnits = balanceToWei(selectedToken.balance, selectedToken.decimals);
        if (balUnits > reserveUnits) {
          setAmount(fromBaseUnits(balUnits - reserveUnits, selectedToken.decimals));
          return;
        }
        setAmount('0');
        return;
      } catch {
        // Estimation failed — fall through to full balance (the pre-check still warns).
      }
    }

    // ERC-20 Max when the in-band gas fee is paid in the SAME token being swept: the batched
    // fee leg (stable.transfer(treasury, fee)) needs balance too, so Max must leave the fee
    // behind — otherwise the op sweeps everything and can't cover its own gas. Only when the
    // selected fee token matches this token; otherwise gas is paid in native / a separate
    // balance and full balance is sendable.
    if (
      activeAccount && gasFeeToken && selectedToken.tokenAddress &&
      gasFeeToken.toLowerCase() === selectedToken.tokenAddress.toLowerCase()
    ) {
      try {
        const chainId = tokenChainId(selectedToken);
        const fee = feeEstimate ?? await estimateTransactionFee(
          activeAccount.address, chainId, 'fast', undefined, undefined, gasFeeToken,
          prefetchedAccount.current?.publicKeyHex,
        );
        if (fee.feeAsset?.kind === 'erc20' && fee.feeAsset.token.toLowerCase() === gasFeeToken.toLowerCase()) {
          // Reserve 1.5× the quoted fee (+50% for the send-time re-quote drift the 2× gate absorbs).
          const reserve = (fee.feeAsset.amount * 3n) / 2n;
          const balUnits = balanceToWei(selectedToken.balance, selectedToken.decimals);
          setAmount(balUnits > reserve ? fromBaseUnits(balUnits - reserve, selectedToken.decimals) : '0');
          return;
        }
      } catch {
        // Estimation failed — fall through to full balance (the pre-check still warns).
      }
    }

    // ERC-20 tokens: gas is paid in native token (or a separate pathUSD balance on Tempo),
    // so full balance is sendable.
    setAmount(selectedToken.balance || '0');
  };

  const handleConfirm = async () => {
    if (!selectedToken || !activeAccount) return;
    // Tap haptic fires on the one-tap VelaButton; the hold-to-confirm path
    // provides its own (Medium on press + Success on completion).

    // The relayer treasury was checked before entering the confirm screen.
    // Proceed directly to transaction execution.
    await executeTransaction();
  };

  const executeTransaction = async () => {
    if (!selectedToken || !activeAccount) return;
    // Synchronous re-entry lock: `sending` is async React state, so a rapid
    // second slide in the same tick would start a concurrent submit. The
    // Phase-2 grant await widens that window to ~20s — guard on a ref.
    const sendGen = sendLock.begin();
    if (sendGen === null) return; // a send is already in flight
    sendCancelledRef.current = false;
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

      // Recheck immediately before signing to cover the rare race where the
      // relayer float falls below its floor after the send-page preflight.
      if (await maybeShowTreasuryBootstrap(chainId)) {
        setSending(false);
        setTxStatus('idle');
        return;
      }

      setTxStatus('submitting');
      const maxFee = feeEstimate?.maxFeePerGas;
      // In-band: sign EXACTLY the fee the confirm slide displayed (amount + recipient).
      // The bundler's 2×-real-cost gate rejects a stale quote loudly; we then re-quote
      // and the user re-confirms a NEW number — never a silent display/charge mismatch.
      const quotedFee = feeEstimate?.inBand && feeEstimate.feeRecipient
        ? {
            amount: feeEstimate.feeAsset?.kind === 'erc20' ? feeEstimate.feeAsset.amount : feeEstimate.totalWei,
            recipient: feeEstimate.feeRecipient,
          }
        : undefined;

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
        result = await sendBatchCalls(activeAccount.address, calls, chainId, stored.publicKeyHex, signFn, maxFee, gasFeeToken, quotedFee);
        lines = specs.map((spec) => {
          const tk = pickedTokens.find((t) => (isNativeToken(t) ? null : t.tokenAddress) === spec.tokenAddress)!;
          return { to: recipient.trim(), toName: recipientIdentity?.name, amount: spec.amount, symbol: tk.symbol, decimals: tk.decimals, priceUsd: tk.priceUsd ?? 0, logoUrls: tokenLogoURLs(tk) };
        });
      } else if (splitMode) {
        const calls = buildSplitCalls(
          { tokenAddress: isNativeToken(selectedToken) ? null : selectedToken.tokenAddress, decimals: selectedToken.decimals },
          recipients.map((r) => ({ address: r.address.trim(), amount: r.amount })),
        );
        result = await sendBatchCalls(activeAccount.address, calls, chainId, stored.publicKeyHex, signFn, maxFee, gasFeeToken, quotedFee);
        lines = recipients.map((r) => ({ to: r.address.trim(), toName: r.name?.trim() || undefined, amount: r.amount, symbol: selectedToken!.symbol, decimals: selectedToken!.decimals, priceUsd: selectedToken!.priceUsd ?? 0, logoUrls: tokenLogoURLs(selectedToken!) }));
      } else {
        const tokenAmount = resolveTokenAmount(amount, inputInUsd, selectedToken.priceUsd, selectedToken.decimals, dc.rate);
        const weiHex = amountToWeiHex(tokenAmount, selectedToken.decimals);
        if (isNativeToken(selectedToken)) {
          result = await sendNative(activeAccount.address, recipient, weiHex, chainId, stored.publicKeyHex, signFn, maxFee, gasFeeToken, quotedFee);
        } else {
          result = await sendERC20(activeAccount.address, selectedToken.tokenAddress!, recipient, weiHex, chainId, stored.publicKeyHex, signFn, maxFee, gasFeeToken, quotedFee);
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
      } else if (/gas relayer is unavailable/i.test(error?.message ?? '')
          && await maybeShowTreasuryBootstrap(tokenChainId(selectedToken!))) {
        // The in-band path found no usable relayer float AND the treasury says
        // it needs a bootstrap: the community bootstrap sheet is the honest ask —
        // a generic "try again" would loop forever. A transient relayer blip
        // (no bootstrapNeeded) falls through to the generic error below.
        setTxStatus('idle');
      } else if (underfunded) {
        // Never open the personal gas-account top-up sheet from a reactive
        // bundler error. Recheck only the relayer treasury: if it is depleted,
        // show the bootstrap sheet; otherwise leave the request as an ordinary
        // failed send rather than asking the user to fund their own gas bucket.
        const chainId = tokenChainId(selectedToken!);
        if (await maybeShowTreasuryBootstrap(chainId)) {
          setTxStatus('idle');
        } else {
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
      // Release only if this is still the current send. A cancelled send already
      // released the lock (and bumped the generation), so this stale finally must
      // not clear a newer in-flight send's lock or its spinner (issue #91).
      if (sendLock.end(sendGen)) setSending(false);
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

  return {
    t,
    router,
    params,
    locked,
    amountLocked,
    activeAccount,
    state,
    address,
    dc,
    formatUsd,
    hasPreselection,
    step,
    setStep,
    stepRef,
    lockError,
    setLockError,
    lockRetry,
    setLockRetry,
    resolvingLock,
    setResolvingLock,
    addingNetwork,
    setAddingNetwork,
    addNetworkMsg,
    setAddNetworkMsg,
    tokens,
    setTokens,
    loading,
    setLoading,
    selectedToken,
    setSelectedToken,
    recipient,
    setRecipient,
    amount,
    setAmount,
    splitMode,
    setSplitMode,
    recipients,
    setRecipients,
    pickerTarget,
    setPickerTarget,
    multiSelectMode,
    setMultiSelectMode,
    multiSelect,
    sending,
    setSending,
    showScanner,
    setShowScanner,
    copiedContract,
    setCopiedContract,
    feeEstimate,
    setFeeEstimate,
    estimatingGas,
    setEstimatingGas,
    sendLock,
    sendCancelledRef,
    mountedRef,
    txStatus,
    setTxStatus,
    txHash,
    setTxHash,
    userOpHash,
    setUserOpHash,
    txError,
    setTxError,
    receiptTransfers,
    setReceiptTransfers,
    receiptKind,
    setReceiptKind,
    receiptFailed,
    setReceiptFailed,
    inputInUsd,
    setInputInUsd,
    gasFeeToken,
    setGasFeeToken,
    treasuryBootstrap,
    setTreasuryBootstrap,
    feeBusy,
    setFeeBusy,
    showContactPicker,
    setShowContactPicker,
    showBatchImport,
    setShowBatchImport,
    amountWarning,
    setAmountWarning,
    recipientIdentity,
    setRecipientIdentity,
    recipientRisk,
    setRecipientRisk,
    sim,
    setSim,
    amountInputRef,
    prefetchedAccount,
    webauthnModuleRef,
    resolveLockedRequest,
    handleAddNetwork,
    refreshTokens,
    maybeShowTreasuryBootstrap,
    enterSplitMode,
    seedSplitRecipients,
    handleRecipientsChange,
    applyPickedAddress,
    pickedTokens,
    multiTokenSpecs,
    confirmSelection,
    handleSelectToken,
    handleContinue,
    handleMaxAmount,
    handleConfirm,
    executeTransaction,
    handleBack,
    tokenMultiSelect,
  };
}

export type SendController = ReturnType<typeof useSendController>;
