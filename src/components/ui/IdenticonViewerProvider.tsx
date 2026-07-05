/**
 * IdenticonViewerProvider — one app-level host for the "tap an identicon to see
 * it big" viewer, reachable from any avatar via {@link useIdenticonViewer}.
 *
 * Why a provider instead of a sheet per screen: the same viewer is opened from
 * the Home avatar, the account switcher, the contacts list and the recipient
 * picker — several of which are THEMSELVES modals. Hosting one sheet here keeps
 * a single instance and one open() entry point.
 *
 * Why mount-on-open (not a persistent, always-rendered sheet): most of those
 * callers open the viewer while another AppModal is already up. On the web,
 * AppModal stacks by DOM insertion order, so a sheet mounted at app start would
 * render BEHIND a modal that mounted later. Mounting only when a target is set
 * appends the viewer's container last → it always sits on top. A short delayed
 * unmount preserves the native/web slide-out animation on close.
 */
import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { IdenticonViewerSheet } from '@/components/ui/IdenticonViewerSheet';
import { isAddress } from '@/models/types';

type OpenFn = (name: string, address: string) => void;

const IdenticonViewerContext = createContext<OpenFn>(() => {});

/** Open the shared identicon viewer for an address. No-op outside the provider. */
export function useIdenticonViewer(): OpenFn {
  return useContext(IdenticonViewerContext);
}

/** Matches AppModal's web exit transition (300ms) + a little slack. */
const EXIT_MS = 320;

export function IdenticonViewerProvider({ children }: { children: React.ReactNode }) {
  const [target, setTarget] = useState<{ name: string; address: string } | null>(null);
  const [visible, setVisible] = useState(false);
  const exitTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const open = useCallback<OpenFn>((name, address) => {
    // Only real addresses have a meaningful identicon; ignore anything else.
    if (!isAddress(address)) return;
    if (exitTimer.current) { clearTimeout(exitTimer.current); exitTimer.current = null; }
    setTarget({ name, address });
    setVisible(true);
  }, []);

  const close = useCallback(() => {
    setVisible(false);
    if (exitTimer.current) clearTimeout(exitTimer.current);
    // Keep the sheet mounted through its exit animation, then unmount so the
    // next open re-appends a fresh (top-most) container on the web.
    exitTimer.current = setTimeout(() => { setTarget(null); exitTimer.current = null; }, EXIT_MS);
  }, []);

  useEffect(() => () => { if (exitTimer.current) clearTimeout(exitTimer.current); }, []);

  return (
    <IdenticonViewerContext.Provider value={open}>
      {children}
      {target !== null && (
        <IdenticonViewerSheet
          visible={visible}
          name={target.name}
          address={target.address}
          onClose={close}
        />
      )}
    </IdenticonViewerContext.Provider>
  );
}
