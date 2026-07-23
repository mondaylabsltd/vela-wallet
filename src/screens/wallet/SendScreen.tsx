import { ContactPicker } from '@/components/contacts/ContactPicker';
import { QRScanner } from '@/components/QRScanner';
import { BatchImportSheet } from '@/components/send/BatchImportSheet';
import { makeRecipientId } from '@/components/send/MultiRecipientEditor';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { TokenSelector } from '@/components/ui/TokenSelector';
import { TransactionReceipt } from '@/components/ui/TransactionReceipt';
import { TreasuryBootstrapSheet } from '@/components/ui/TreasuryBootstrapSheet';
import { VelaButton } from '@/components/ui/VelaButton';
import { fadeInDown } from '@/constants/entering';
import { color } from '@/constants/theme';
import { styles } from './SendScreen.styles';
import { ConfirmStep } from './ConfirmStep';
import { EnterDetailsStep } from './EnterDetailsStep';
import { useSendController } from './useSendController';
import { tokenChainId, tokenLogoURLs } from '@/models/types';
import { BATCH_MAX_RECIPIENTS } from '@/services/batch-send';
import { saveContact } from '@/services/contacts';
import { parseEIP681 } from '@/services/eip681';
import { resolveTokenAmount } from '@/services/fiat-convert';
import { AlertCircle, ArrowLeft, Globe, X } from 'lucide-react-native';
import React from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';



export default function SendScreen() {
  const c = useSendController();
  const {
    t,
    router,
    locked,
    activeAccount,
    address,
    dc,
    step,
    lockError,
    resolvingLock,
    addingNetwork,
    addNetworkMsg,
    tokens,
    loading,
    selectedToken,
    recipient,
    setRecipient,
    amount,
    pickerTarget,
    multiSelect,
    showScanner,
    setShowScanner,
    txStatus,
    txHash,
    userOpHash,
    receiptTransfers,
    receiptKind,
    receiptFailed,
    inputInUsd,
    treasuryBootstrap,
    setTreasuryBootstrap,
    handleConfirm,
    showContactPicker,
    setShowContactPicker,
    showBatchImport,
    setShowBatchImport,
    recipientIdentity,
    handleAddNetwork,
    refreshTokens,
    seedSplitRecipients,
    applyPickedAddress,
    handleSelectToken,
    handleBack,
    tokenMultiSelect,
  } = c;

  const renderSelectToken = () => (
    <Animated.View style={styles.stepContainer} entering={fadeInDown(0, 300)}>
      <Text style={styles.stepTitle}>{t('send.selectTokenTitle')}</Text>
      <TokenSelector
        tokens={tokens}
        loading={loading}
        onSelect={handleSelectToken}
        onAddChanged={refreshTokens}
        initialChainId={multiSelect.chainId}
        multiSelect={tokenMultiSelect}
      />
    </Animated.View>
  );



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
          {step === 'enter-details' && <EnterDetailsStep c={c} />}
          {step === 'confirm' && <ConfirmStep c={c} />}
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

      {/* Relayer float depleted on this network — community bootstrap ask
          (non-refundable treasury contribution), shown instead of the generic
          error / personal top-up surfaces. */}
      <TreasuryBootstrapSheet
        visible={!!treasuryBootstrap}
        status={treasuryBootstrap}
        onClose={() => setTreasuryBootstrap(null)}
        onRetry={() => {
          setTreasuryBootstrap(null);
          // This sheet can now open from the amount screen. After funding the
          // relayer, return through the normal pre-confirm flow rather than
          // submitting directly from enter-details.
          if (step === 'enter-details') {
            void c.handleContinue();
          } else {
            void handleConfirm();
          }
        }}
      />

      <ContactPicker
        visible={showContactPicker}
        onClose={() => setShowContactPicker(false)}
        onSelect={(addr) => applyPickedAddress(addr)}
        onSelectGroup={locked || pickerTarget ? undefined : (addrs) =>
          seedSplitRecipients(addrs.map((a) => ({ id: makeRecipientId(), address: a, amount: '' })))}
        onScan={locked ? undefined : () => setShowScanner(true)}
        myAddress={address}
      />

      {selectedToken && (
        <BatchImportSheet
          visible={showBatchImport}
          onClose={() => setShowBatchImport(false)}
          token={selectedToken}
          currencyCode={dc.code}
          currencySymbol={dc.symbol}
          onApply={seedSplitRecipients}
          maxRecipients={BATCH_MAX_RECIPIENTS}
        />
      )}
    </ScreenContainer>
  );
}
