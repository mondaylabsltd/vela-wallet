import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { RecipientTypeBadge } from '@/components/contacts/RecipientTypeBadge';
import { ConfirmAssets, type ConfirmAssetRow } from '@/components/send/ConfirmAssets';
import { FlowArrow } from '@/components/send/FlowArrow';
import { BalanceChangePreview } from '@/components/signing/BalanceChangePreview';
import { GasFeeCard } from '@/components/ui/GasFeeCard';
import { SlideToConfirmButton } from '@/components/ui/SlideToConfirmButton';
import { WalletAvatar } from '@/components/ui/WalletAvatar';
import { fadeInDown } from '@/constants/entering';
import { color, space } from '@/constants/theme';
import { styles } from './SendScreen.styles';
import { shortAddr } from './send-utils';
import { chainName, nativeSymbol, tokenBadgeNetwork } from '@/models/network';
import { formatBalance, isNativeToken, tokenBalanceDouble, tokenChainId, tokenId, tokenLogoURLs, type APIToken } from '@/models/types';
import * as Passkey from '@/modules/passkey';
import { sumSplitBaseUnits } from '@/services/batch-send';
import { fromBaseUnits } from '@/services/eip681';
import { resolveTokenAmount } from '@/services/fiat-convert';
import { AlertCircle, Gift, X } from 'lucide-react-native';
import React, { useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import type { SendController } from './useSendController';

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

export function ConfirmStep({ c }: { c: SendController }) {
  const {
    t,
    activeAccount,
    address,
    dc,
    formatUsd,
    tokens,
    selectedToken,
    recipient,
    amount,
    splitMode,
    recipients,
    multiSelectMode,
    sending,
    setSending,
    feeEstimate,
    setFeeEstimate,
    estimatingGas,
    sendLock,
    sendCancelledRef,
    gasSponsored,
    mountedRef,
    txStatus,
    setTxStatus,
    txError,
    setTxError,
    inputInUsd,
    gasFeeToken,
    setGasFeeToken,
    feeBusy,
    setFeeBusy,
    recipientIdentity,
    recipientRisk,
    sim,
    pickedTokens,
    multiTokenSpecs,
    handleConfirm,
  } = c;

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

    // The asset shown below the recipient in the single-token modes (1→1 and
    // split). One quiet identity pill; the amount already lives on the From/To
    // rows, so ConfirmAssets doesn't repeat it here.
    const singleAsset: ConfirmAssetRow[] = [{
      key: tokenId(selectedToken),
      symbol: selectedToken.symbol,
      logoUrls: logos,
      chain: tokenBadgeNetwork(selectedToken),
      networkText: chain,
    }];

    // Native symbol + price for GasFeeCard's fee display (native fee ≈fiat).
    const sym = nativeSymbol(tokenChainId(selectedToken));
    const nativePrice = isNativeToken(selectedToken)
      ? (selectedToken.priceUsd ?? 0)
      : (tokens.find(t => isNativeToken(t) && tokenChainId(t) === tokenChainId(selectedToken))?.priceUsd ?? 0);

    return (
      <ScrollView style={styles.stepContainer} showsVerticalScrollIndicator={false}>
        <Animated.View entering={fadeInDown(0, 300)}>
          <Text style={styles.stepTitle}>{t('send.confirmTitle')}</Text>

          {/* Transfer review: From → To with token info — open rows on the page */}
          <View style={styles.confirmBlock}>
            {(!splitMode && !multiSelectMode) ? (
              /* Simple 1→1 transfer — a From → To flow (money follows the person:
                 sender −, recipient +). Merges the old separate token row and the
                 "余额变化" section, which for a plain send just restated this amount. */
              <>
                <View style={styles.party}>
                  <WalletAvatar name={activeAccount?.name ?? ''} address={address ?? undefined} size={38} />
                  <View style={styles.partyWho}>
                    <Text style={styles.partyName} numberOfLines={1}>{activeAccount?.name ?? t('send.walletFallbackName')}</Text>
                    <Text style={styles.transferAddr}>{shortAddr(address ?? '')}</Text>
                  </View>
                  <View style={styles.partyAmt}>
                    <Text style={styles.amtOut} numberOfLines={1}>−{formatBalance(amountNum)} {selectedToken.symbol}</Text>
                    {usdAmount > 0 && <Text style={styles.transferTokenSub}>≈ {formatUsd(usdAmount)}</Text>}
                  </View>
                </View>

                <FlowArrow />

                <View style={styles.party}>
                  <ContactAvatar name={recipientIdentity?.name ?? ''} address={recipient} size={38} />
                  <View style={styles.partyWho}>
                    {/* Name (contact › vela › ens); trust marker on the RIGHT. No "first time"
                        tag — this device can't see sends made from the user's other devices. */}
                    <View style={styles.nameRow}>
                      <RecipientTrust address={recipient} identity={recipientIdentity} prominent nameOnly />
                      <RecipientTypeBadge address={recipient} identity={recipientIdentity} isContract={recipientRisk?.isContract} />
                    </View>
                    <Text style={styles.transferAddr}>{shortAddr(recipient)}</Text>
                  </View>
                  <View style={styles.partyAmt}>
                    <Text style={styles.amtIn} numberOfLines={1}>+{formatBalance(amountNum)} {selectedToken.symbol}</Text>
                    {usdAmount > 0 && <Text style={styles.transferTokenSub}>≈ {formatUsd(usdAmount)}</Text>}
                  </View>
                </View>

                <ConfirmAssets rows={singleAsset} />
              </>
            ) : multiSelectMode ? (
            /* ===== N tokens → 1 recipient (sweep): sender → ↓ → recipient, then the
               collapsible asset cluster BELOW the recipient — the same flow as 1→1,
               only the single-asset pill becomes a multi-asset cluster. ===== */
            (() => {
              const specs = multiTokenSpecs(tokenChainId(selectedToken));
              const amountOf = (tk: APIToken) => {
                const addr = isNativeToken(tk) ? null : tk.tokenAddress;
                const spec = specs.find((s) => s.tokenAddress === addr);
                return spec ? parseFloat(spec.amount) : 0;
              };
              const sweepRows: ConfirmAssetRow[] = pickedTokens.map((tk) => {
                const amt = amountOf(tk);
                const usd = amt * (tk.priceUsd ?? 0);
                const reserved = amt < tokenBalanceDouble(tk);
                return {
                  key: tokenId(tk),
                  symbol: tk.symbol,
                  logoUrls: tokenLogoURLs(tk),
                  chain: tokenBadgeNetwork(tk),
                  networkText: `${chainName(tokenChainId(tk))}${reserved ? ` · ${t('send.gasReserved')}` : ''}`,
                  amountText: formatBalance(amt),
                  usdText: usd > 0 ? `≈ ${formatUsd(usd)}` : undefined,
                };
              });
              const totalUsd = pickedTokens.reduce((s, tk) => s + amountOf(tk) * (tk.priceUsd ?? 0), 0);
              return (
                <>
                  <View style={styles.party}>
                    <WalletAvatar name={activeAccount?.name ?? ''} address={address ?? undefined} size={38} />
                    <View style={styles.partyWho}>
                      <Text style={styles.partyName} numberOfLines={1}>{activeAccount?.name ?? t('send.walletFallbackName')}</Text>
                      <Text style={styles.transferAddr}>{shortAddr(address ?? '')}</Text>
                    </View>
                  </View>

                  <FlowArrow />

                  <View style={styles.party}>
                    <ContactAvatar name={recipientIdentity?.name ?? ''} address={recipient} size={38} />
                    <View style={styles.partyWho}>
                      <View style={styles.nameRow}>
                        <RecipientTrust address={recipient} identity={recipientIdentity} prominent nameOnly />
                        <RecipientTypeBadge address={recipient} identity={recipientIdentity} isContract={recipientRisk?.isContract} />
                      </View>
                      <Text style={styles.transferAddr}>{shortAddr(recipient)}</Text>
                    </View>
                  </View>

                  <ConfirmAssets
                    rows={sweepRows}
                    countLabel={t('send.tokenCount', { n: pickedTokens.length })}
                    totalLabel={totalUsd > 0 ? `≈ ${formatUsd(totalUsd)}` : undefined}
                  />
                </>
              );
            })()
            ) : (
            /* ===== 1 token → N recipients (split): party From, then a fixed-height,
               internally-scrolling recipient list (≤5 visible) in the same style. ===== */
            <>
              <View style={styles.party}>
                <WalletAvatar name={activeAccount?.name ?? ''} address={address ?? undefined} size={38} />
                <View style={styles.partyWho}>
                  <Text style={styles.partyName} numberOfLines={1}>{activeAccount?.name ?? t('send.walletFallbackName')}</Text>
                  <Text style={styles.transferAddr}>{shortAddr(address ?? '')}</Text>
                </View>
                <View style={styles.partyAmt}>
                  <Text style={styles.amtOut} numberOfLines={1}>−{formatBalance(amountNum)} {selectedToken.symbol}</Text>
                  {usdAmount > 0 && <Text style={styles.transferTokenSub}>≈ {formatUsd(usdAmount)}</Text>}
                </View>
              </View>

              <FlowArrow />

              <Text style={[styles.transferLabel, styles.recipientListLabel]}>
                {t('send.recipientCount', { count: recipients.length, n: recipients.length })}
              </Text>
              <ScrollView style={styles.recipientList} nestedScrollEnabled showsVerticalScrollIndicator={false}>
                {recipients.map((r, i) => {
                  const n = parseFloat(r.amount || '0');
                  const u = n * (selectedToken.priceUsd ?? 0);
                  return (
                    <View key={r.id} style={[styles.party, styles.recipientRow]}>
                      <Text style={styles.recipientIndex}>{i + 1}</Text>
                      <ContactAvatar name="" address={r.address} size={32} />
                      <View style={styles.partyWho}>
                        <View style={styles.nameRow}>
                          <RecipientTrust address={r.address} prominent nameOnly />
                          <RecipientTypeBadge address={r.address} size={13} />
                        </View>
                        <Text style={styles.transferAddr}>{shortAddr(r.address)}</Text>
                      </View>
                      <View style={styles.partyAmt}>
                        <Text style={styles.amtIn} numberOfLines={1}>+{formatBalance(n)} {selectedToken.symbol}</Text>
                        {u > 0 && <Text style={styles.transferTokenSub}>≈ {formatUsd(u)}</Text>}
                      </View>
                    </View>
                  );
                })}
              </ScrollView>

              <ConfirmAssets rows={singleAsset} />
            </>
            )}
          </View>

          {/* Balance-change simulation — a SAFETY signal only now: the From→To flow
              above already states the intended change, so `heroFlows` + `hideReassurance`
              collapse the corroborated-plain-send case to nothing. LOUD states (predicted
              revert, underfunded, an UNEXPECTED asset movement) still surface. */}
          <BalanceChangePreview
            result={sim}
            chainId={tokenChainId(selectedToken)}
            selfTransfer={!!activeAccount && recipient.toLowerCase() === activeAccount.address.toLowerCase()}
            heroFlows={(multiSelectMode ? pickedTokens : [selectedToken]).map((tk) => ({
              token: isNativeToken(tk) ? undefined : tk.tokenAddress?.toLowerCase(),
              dir: 'out' as const,
            }))}
            hideReassurance
          />

          {/* First-send trust moment: the treasury covered (or is about to
              cover) this wallet's gas float — say so, quietly. */}
          {gasSponsored && (
            <View style={styles.sponsoredRow}>
              <Gift size={13} color={color.fg.subtle} strokeWidth={2} />
              <Text style={styles.sponsoredText}>{t('send.gasSponsoredNote')}</Text>
            </View>
          )}

          {/* Estimated fee — the one gas surface left (tiers + technical rows are
              gone; every send runs at 'fast'). An in-band stablecoin fee renders
              in the token's own units with a matching ≈USD subline; otherwise the
              native formatting stays. Tap to pick the fee asset when this chain
              offers a choice. */}
          {/* Estimated fee + fee-asset selector — the shared GasFeeCard (same
              component the dApp signing sheet uses). It owns the collapsed row,
              the expand, and the per-asset re-quote; this screen just owns the
              gasFeeToken selection + feeEstimate it threads into submit. On Tempo
              / legacy chains it collapses to a read-only fee line (no selector). */}
          {activeAccount && (
            <GasFeeCard
              feeEstimate={feeEstimate}
              estimating={estimatingGas}
              nativeSymbol={sym}
              nativeUsdPrice={nativePrice}
              safeAddress={activeAccount.address}
              chainId={tokenChainId(selectedToken)}
              gasFeeToken={gasFeeToken}
              onFeeTokenChange={setGasFeeToken}
              onFeeUpdate={(fee) => { if (mountedRef.current) setFeeEstimate(fee); }}
              onBusyChange={setFeeBusy}
            />
          )}

          {txStatus === 'idle' && (
            // Every send is a deliberate slide-to-confirm — a stray tap can't fire
            // a payment. A risky destination (never sent here before, or a contract)
            // is signaled by the first-time / contract tags above; the slide itself
            // stays quiet (founder call: never a scary red commit surface).
            <SlideToConfirmButton
              title={estimatingGas || feeBusy ? t('send.checkingGas') : t('send.confirmSendBtn')}
              hint={t('componentsUi.signing.slideToConfirm', { defaultValue: 'Slide to confirm' })}
              onConfirm={handleConfirm}
              loading={sending}
              disabled={estimatingGas || feeBusy}
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
                    <TxCancelButton onCancel={() => {
                      // Signal the in-flight executeTransaction too: during the
                      // Phase-2 grant await there is no passkey prompt to abort
                      // yet — without the ref, the flow would resurrect a
                      // passkey prompt (or a funding sheet) AFTER this cancel.
                      sendCancelledRef.current = true;
                      // Release the re-entry lock so a retry starts (instead of
                      // silently no-op'ing until the cancelled promise settles);
                      // cancel() also invalidates that promise's stale finally so
                      // it won't clear the retry's lock (issue #91).
                      sendLock.cancel();
                      Passkey.cancelSign();
                      setTxStatus('idle');
                      setSending(false);
                    }} />
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
}
