/**
 * useSweepSelection — shared token multi-select ("sweep") state, used by BOTH the
 * Send token picker and the Home assets sheet so the interaction lives in exactly
 * one place (no copy-pasted selection logic).
 *
 * Multi-select is single-chain by design — a batch UserOp is one chain, so the
 * caller only enables checkboxes once a specific network is filtered, and
 * `onNetworkChange` clears the selection when the network changes. Pair this with
 * `TokenSelector`'s `multiSelect` prop and `batch-send`'s builders.
 */
import { useCallback, useState } from 'react';
import { tokenId, type APIToken } from '@/models/types';
import { selectAllValuable } from '@/services/batch-send';

export function useSweepSelection() {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [chainId, setChainId] = useState<number | null>(null);

  const toggle = useCallback((tk: APIToken) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      const id = tokenId(tk);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }, []);

  // Master "全选" — all held/priced/non-spam tokens in the (chain-scoped) view.
  const isAllSelected = useCallback(
    (visible: APIToken[]) => {
      const valuable = selectAllValuable(visible);
      return valuable.length > 0 && valuable.every((tk) => selectedIds.has(tokenId(tk)));
    },
    [selectedIds],
  );

  const toggleAll = useCallback((visible: APIToken[]) => {
    const valuable = selectAllValuable(visible);
    if (valuable.length === 0) return;
    setSelectedIds((prev) => {
      const allOn = valuable.every((tk) => prev.has(tokenId(tk)));
      const next = new Set(prev);
      valuable.forEach((tk) => (allOn ? next.delete(tokenId(tk)) : next.add(tokenId(tk))));
      return next;
    });
  }, []);

  // Network filter changed — a batch is one chain, so clear and re-lock.
  const onNetworkChange = useCallback((id: number | null) => {
    setSelectedIds(new Set());
    setChainId(id);
  }, []);

  const reset = useCallback(() => {
    setSelectedIds(new Set());
    setChainId(null);
  }, []);

  const selectedTokens = useCallback(
    (tokens: APIToken[]) => tokens.filter((tk) => selectedIds.has(tokenId(tk))),
    [selectedIds],
  );

  return {
    selectedIds,
    chainId,
    count: selectedIds.size,
    toggle,
    isAllSelected,
    toggleAll,
    onNetworkChange,
    reset,
    selectedTokens,
  };
}

export type SweepSelection = ReturnType<typeof useSweepSelection>;
