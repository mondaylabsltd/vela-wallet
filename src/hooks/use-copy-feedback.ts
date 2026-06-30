/**
 * useCopyFeedback — copy a string to the clipboard and flip `copied` true for
 * `resetMs`, then back to false.
 *
 * Replaces the hand-rolled `useState(false)` + `setTimeout(() => setCopied(false), …)`
 * pattern that was duplicated across ~8 components (TokenRow, ReceiveScreen,
 * SettingsScreen, BundlerFundingModal, …). Most of those copies leaked the pending
 * timer on unmount — copy an address then close the sheet fast and React warned
 * about a state update on an unmounted component. This clears the timer both on
 * unmount and when you copy again before the previous reset fires.
 *
 * Haptics stay at the call site (they vary: hapticLight vs hapticSuccess), e.g.
 *   const { copied, copy } = useCopyFeedback();
 *   const onCopy = () => { hapticSuccess(); copy(address); };
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { copyToClipboard } from '@/services/platform';

export function useCopyFeedback(resetMs = 1500) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);

  const copy = useCallback(async (value: string) => {
    await copyToClipboard(value);
    setCopied(true);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), resetMs);
  }, [resetMs]);

  return { copied, copy };
}
