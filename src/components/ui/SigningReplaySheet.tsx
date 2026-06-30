/**
 * SigningReplaySheet — "look back at exactly what I signed".
 *
 * Tapping a Connections-panel record whose original request was captured
 * (LocalTransaction.signedRequest) re-opens the SAME signing panel the user saw
 * when they approved it — descriptor-resolved intent, fields, the lot — rendered
 * read-only (no approve/reject, no live gas/simulation). For an op still awaiting
 * its on-chain receipt it doubles as "return to the in-flight status" after the
 * original sheet was closed.
 *
 * Records that predate request capture have no `signedRequest`; the host falls
 * back to the metadata ConnectionEventDetailSheet for those.
 */
import React from 'react';
import { AppModal } from '@/components/ui/AppModal';
import { SigningSheet } from '@/components/SigningRequestModal';
import { deserializeAssetSim } from '@/services/tx-simulation';
import type { LocalTransaction } from '@/services/storage';
import type { BLEIncomingRequest } from '@/models/types';

interface Props {
  visible: boolean;
  tx: LocalTransaction | null;
  onClose: () => void;
}

export function SigningReplaySheet({ visible, tx, onClose }: Props) {
  const req = tx?.signedRequest;
  const request: BLEIncomingRequest | null = req
    ? { id: tx!.id, method: req.method, params: (req.params ?? []) as any[], origin: tx!.dappOrigin ?? '' }
    : null;
  // The "what moved" preview captured at sign time — rehydrated for the read-only
  // replay (the live sheet can't re-simulate a past tx against current state).
  const replaySim = tx?.assetChanges ? deserializeAssetSim(tx.assetChanges) : null;

  return (
    <AppModal visible={visible && request !== null} onClose={onClose}>
      {request && tx && (
        <SigningSheet
          request={request}
          chainId={tx.chainId}
          account={null}
          dappInfo={{ name: tx.dappOrigin }}
          isSigning={false}
          signError={null}
          // Still-pending op → keep the "submitted, waiting" banner so reopening
          // from the panel shows its in-flight status.
          pendingOpHash={tx.status === 'pending' ? (tx.userOpHash || null) : null}
          onApprove={() => {}}
          onReject={onClose}
          onDismiss={onClose}
          readOnly
          replaySim={replaySim}
        />
      )}
    </AppModal>
  );
}
