import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { RecipientTypeBadge } from '@/components/contacts/RecipientTypeBadge';
import { MultiRecipientEditor, recipientsAreValid } from '@/components/send/MultiRecipientEditor';
import { TokenLogo } from '@/components/TokenLogo';
import { AmountText } from '@/components/ui/AmountText';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { VelaButton } from '@/components/ui/VelaButton';
import { fadeInDown } from '@/constants/entering';
import { color, space, text } from '@/constants/theme';
import { styles } from './SendScreen.styles';
import { amountFontSize, isValidAddress, sanitizeAmountInput, shortAddr } from './send-utils';
import { chainName, tokenBadgeNetwork } from '@/models/network';
import { isNativeToken, tokenBalanceDouble, tokenChainId, tokenId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { BATCH_MAX_RECIPIENTS } from '@/services/batch-send';
import { ZERO_DECIMAL_CODES } from '@/services/currency';
import { formatTokenAmount, numberSeparators, parseLocaleNumber } from '@/services/locale-format';
import { copyToClipboard } from '@/services/platform';
import { ArrowUpDown, BookUser, Check, Copy, FileUp, Plus, ScanLine } from 'lucide-react-native';
import React from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';
import type { SendController } from './useSendController';

export function EnterDetailsStep({ c }: { c: SendController }) {
  const {
    t,
    params,
    locked,
    amountLocked,
    dc,
    formatUsd,
    setStep,
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
    setPickerTarget,
    multiSelectMode,
    setShowScanner,
    copiedContract,
    setCopiedContract,
    estimatingGas,
    inputInUsd,
    setInputInUsd,
    setShowContactPicker,
    setShowBatchImport,
    amountWarning,
    recipientIdentity,
    recipientRisk,
    amountInputRef,
    enterSplitMode,
    handleRecipientsChange,
    pickedTokens,
    multiTokenSpecs,
    handleContinue,
    handleMaxAmount,
  } = c;

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

          {/* Token hero (single/split) — open row on the page, tap to switch token.
              Multi-select hides it. */}
          {!multiSelectMode && (
          <View style={styles.heroBlock}>
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
            {!isNativeToken(selectedToken) && selectedToken.tokenAddress ? (<>
              <View style={styles.heroDivider} />
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
            </>) : null}
          </View>
          )}

          {/* Single-recipient flow (default). Split / multiSelect replace it below. */}
          {!splitMode && !multiSelectMode && (<>
          {/* Amount — open hero on the page (no box); large display with inline unit */}
          <SectionLabel>{t('send.amountLabel', { defaultValue: 'Amount' })}</SectionLabel>
          <Pressable style={styles.amountWrap} onPress={() => { if (!amountLocked) amountInputRef.current?.focus(); }}>
            <View style={styles.amountTopRow}>
              <View style={styles.amountInputWrap}>
                <TextInput
                  ref={amountInputRef}
                  testID="amount-input"
                  style={[styles.amountInput, { fontSize: amountFontSize(amount) }]}
                  placeholder="0"
                  placeholderTextColor={color.fg.subtle}
                  // Stored canonical (dot); shown with the locale decimal so a
                  // dot_comma user sees "47,28" and can type a comma — every
                  // downstream parseFloat(amount) keeps its canonical input.
                  value={amount.replace('.', numberSeparators().decimal)}
                  editable={!amountLocked}
                  onChangeText={(t) => {
                    const maxDec = inputInUsd ? fiatDecimals : selectedToken.decimals;
                    const sanitized = sanitizeAmountInput(parseLocaleNumber(t), maxDec);
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
                {/* Scan in-flow (one tap) — plain icon, no container. */}
                <Pressable
                  onPress={() => setShowScanner(true)}
                  hitSlop={8}
                  style={styles.addrActionBtn}
                  accessibilityRole="button"
                  accessibilityLabel={t('send.scanAria', { defaultValue: 'Scan a QR code' })}
                >
                  <ScanLine size={22} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
                {/* Address book / recent recipients. */}
                <Pressable
                  onPress={() => { setPickerTarget(null); setShowContactPicker(true); }}
                  hitSlop={8}
                  style={styles.addrActionBtn}
                  accessibilityRole="button"
                  accessibilityLabel={t('send.recipientPickAria', { defaultValue: 'Choose recipient' })}
                >
                  <BookUser size={22} color={color.fg.muted} strokeWidth={2} />
                </Pressable>
              </View>
            )}
          </View>

          {/* Recipient identity — SAME treatment as the confirm row: avatar + name + the trust
              marker on the right (calm colors; the name is ink, not accent-orange). */}
          {recipient.length > 0 && isValidAddress(recipient) && (
            <View style={styles.recipientIdentityRow}>
              <ContactAvatar name={recipientIdentity?.name ?? ''} address={recipient} size={28} />
              <View style={styles.nameRow}>
                <RecipientTrust address={recipient} identity={recipientIdentity} prominent nameOnly />
                <RecipientTypeBadge address={recipient} identity={recipientIdentity} isContract={recipientRisk?.isContract} />
              </View>
            </View>
          )}

          {/* Send this token to several people at once → split mode, or import a
              payroll table (fiat → token) in one go. */}
          {!locked && !params.prefilledRecipient && (
            <View style={styles.splitEntryRow}>
              <Pressable onPress={enterSplitMode} style={styles.addRecipientEntry}>
                <Plus size={16} color={color.accent.base} strokeWidth={2.5} />
                <Text style={styles.addRecipientEntryText}>{t('send.addRecipient', { defaultValue: 'Add recipient' })}</Text>
              </Pressable>
              <Pressable onPress={() => setShowBatchImport(true)} style={styles.addRecipientEntry} testID="send-batch-import">
                <FileUp size={16} color={color.accent.base} strokeWidth={2.5} />
                <Text style={styles.addRecipientEntryText}>{t('send.batchImport', { defaultValue: 'Import list' })}</Text>
              </Pressable>
            </View>
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
              onImport={() => setShowBatchImport(true)}
              maxRecipients={BATCH_MAX_RECIPIENTS}
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
            <View style={styles.multiBlock}>
              <View style={styles.mtSummary}>
                <Text style={styles.mtSummaryTitle}>
                  {t('send.multiSendSummary', { n: pickedTokens.length, chain: chainName(cid) })}
                </Text>
                <Text style={styles.mtSummaryUsd}>{formatUsd(total)}</Text>
              </View>
              {pickedTokens.map((tk) => {
                const amt = amountOf(tk);
                const usd = amt * (tk.priceUsd ?? 0);
                // Trimmed for gas: sent amount is below full balance. True for the native coin
                // AND for any ERC-20 fee asset whose line is trimmed for the gas reserve.
                const reserved = amt < tokenBalanceDouble(tk);
                return (
                  <View key={tokenId(tk)}>
                    <View style={styles.mtSep} />
                    <View style={styles.mtRow}>
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
                  </View>
                );
              })}
            </View>

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
            {recipient.length > 0 && isValidAddress(recipient) && (
              <View style={styles.recipientIdentityRow}>
                <ContactAvatar name={recipientIdentity?.name ?? ''} address={recipient} size={28} />
                <View style={styles.nameRow}>
                  <RecipientTrust address={recipient} identity={recipientIdentity} prominent nameOnly />
                  <RecipientTypeBadge address={recipient} identity={recipientIdentity} isContract={recipientRisk?.isContract} />
                </View>
              </View>
            )}
          </>);
          })()}

          <VelaButton
            title={estimatingGas ? t('send.preparing') : t('send.continueBtn')}
            onPress={handleContinue}
            loading={estimatingGas}
            style={styles.continueBtn}
            disabled={(splitMode ? !recipientsAreValid(recipients) : multiSelectMode ? (!isValidAddress(recipient) || pickedTokens.length === 0) : (!recipient || !amount)) || estimatingGas || (locked && !!amountWarning)}
          />
        </Animated.View>
      </ScrollView>
    );
}
