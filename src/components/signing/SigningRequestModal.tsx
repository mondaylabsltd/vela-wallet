/**
 * Production wrapper — wires the dApp connection context to <SigningSheet>.
 */
import React from 'react';
import { AppModal } from '@/components/ui/AppModal';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet } from '@/models/wallet-state';
import { BundlerFundingView } from '@/components/ui/BundlerFundingModal';
import { requestChainId as reqChainId, requestDApp } from '@/models/dapp-request-routing';
import { SigningSheet } from './SigningSheet';

export function SigningRequestModal() {
  const {
    incomingRequest, isSigning, isSubmitting, signError, pendingOpHash, chainId, dappInfo,
    approveRequest, rejectRequest, dismissRequest,
    fundingNeeded, handleFundingComplete, handleFundingCancel,
  } = useDAppConnection();
  const { activeAccount } = useWallet();

  if (!incomingRequest) return null;

  return (
    // A single native sheet. When the gas account needs funding we SWAP the
    // sheet's content to the funding view instead of stacking a second AppModal
    // over it — iOS won't present a second native modal atop a presented one, so
    // a stacked funding modal was invisible and tapping Approve did nothing
    // (docs/KNOWN-BUGS.md BUG-1). Swipe-to-dismiss over the funding view cancels
    // the pending request (handleFundingCancel), matching the funding "取消".
    //
    // Swipe-dismiss routing: once submitting (isSubmitting) or already submitted
    // (pendingOpHash), the tx is committed → DISMISS (op proceeds, real result
    // delivered), never reject — a "cancelled" tx must not still broadcast + send a
    // contradictory success (BUG-2). Only a pre-submit swipe rejects (4001).
    <AppModal
      visible={true}
      onClose={
        fundingNeeded
          ? handleFundingCancel
          : signError || pendingOpHash || isSubmitting
            ? dismissRequest
            : rejectRequest
      }
    >
      {fundingNeeded ? (
        <BundlerFundingView
          funding={fundingNeeded}
          onFunded={handleFundingComplete}
          onCancel={handleFundingCancel}
          dappVariant
        />
      ) : (
        /* Per-request chain/identity for a Safari-extension sign (F3/F4): sign +
           display against the ORIGIN's granted chain and identity, never a
           concurrent WalletPair session's global chainId/dappInfo. Ordinary
           requests carry no __chainId/__dapp → fall back to the global state. */
        <SigningSheet
          request={incomingRequest}
          chainId={reqChainId(incomingRequest, chainId)}
          account={activeAccount ?? null}
          dappInfo={requestDApp(incomingRequest, dappInfo)}
          isSigning={isSigning}
          signError={signError}
          pendingOpHash={pendingOpHash}
          onApprove={approveRequest}
          onReject={rejectRequest}
          onDismiss={dismissRequest}
        />
      )}
    </AppModal>
  );
}
