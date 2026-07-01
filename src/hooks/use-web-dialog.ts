/**
 * useWebDialog — gives web modals/alerts the dialog contract that react-native's
 * <Modal> provides natively but the DOM-portal path does not:
 *
 *   - Escape closes the dialog
 *   - Tab is trapped inside the dialog (focus can't leak to the page behind)
 *   - focus moves into the dialog on open and is restored to the opener on close
 *   - background page scroll is locked while open (ref-counted so a stacked
 *     AppAlert above an AppModal doesn't unlock the page early)
 *   - role="dialog" + aria-modal="true" are set on the container element
 *
 * No-op on native (iOS/Android <Modal> already handles all of this).
 *
 * Usage:
 *   const ref = useWebDialog(visible, onClose);
 *   return <div ref={ref}>…</div>;   // or ref={ref} on a react-native-web <View>
 */
import { useEffect, useRef } from 'react';
import { Platform } from 'react-native';

// Ref-counted body scroll lock, shared across all open dialogs.
let scrollLockCount = 0;
let savedOverflow = '';

function lockScroll() {
  if (typeof document === 'undefined') return;
  if (scrollLockCount === 0) {
    savedOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
  }
  scrollLockCount += 1;
}

function unlockScroll() {
  if (typeof document === 'undefined') return;
  scrollLockCount = Math.max(0, scrollLockCount - 1);
  if (scrollLockCount === 0) document.body.style.overflow = savedOverflow;
}

const FOCUSABLE =
  'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])';

export function useWebDialog(open: boolean, onClose?: () => void, role: 'dialog' | 'alertdialog' = 'dialog') {
  const containerRef = useRef<any>(null);

  // Read the latest onClose/role from the keydown handler without re-running the
  // whole effect when the caller passes a fresh onClose on each render. Otherwise a
  // parent that re-renders while the dialog is open (e.g. Home's account switcher
  // refreshing every account's balance) would repeatedly tear down and re-run the
  // body scroll-lock + focus setup, making the background flicker/shift.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const roleRef = useRef(role);
  roleRef.current = role;

  useEffect(() => {
    if (Platform.OS !== 'web' || typeof document === 'undefined' || !open) return;

    lockScroll();

    const el = containerRef.current as HTMLElement | null;
    const prevFocused = document.activeElement as HTMLElement | null;

    if (el) {
      el.setAttribute('role', roleRef.current);
      el.setAttribute('aria-modal', 'true');
      if (!el.hasAttribute('tabindex')) el.setAttribute('tabindex', '-1');
    }

    const visibleFocusables = (): HTMLElement[] =>
      el
        ? (Array.from(el.querySelectorAll(FOCUSABLE)) as HTMLElement[]).filter(
            (n) => n.offsetParent !== null || n === document.activeElement,
          )
        : [];

    // Move focus into the dialog once its content has mounted.
    const focusTimer = setTimeout(() => {
      if (!el) return;
      if (el.contains(document.activeElement)) return;
      (visibleFocusables()[0] ?? el).focus?.();
    }, 0);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onCloseRef.current?.();
        return;
      }
      if (e.key === 'Tab' && el) {
        const focusables = visibleFocusables();
        if (focusables.length === 0) {
          e.preventDefault();
          el.focus?.();
          return;
        }
        const first = focusables[0];
        const last = focusables[focusables.length - 1];
        const active = document.activeElement as HTMLElement | null;
        if (!el.contains(active)) {
          e.preventDefault();
          first.focus();
        } else if (!e.shiftKey && active === last) {
          e.preventDefault();
          first.focus();
        } else if (e.shiftKey && active === first) {
          e.preventDefault();
          last.focus();
        }
      }
    };

    document.addEventListener('keydown', onKey, true);

    return () => {
      document.removeEventListener('keydown', onKey, true);
      clearTimeout(focusTimer);
      unlockScroll();
      // Restore focus to whatever opened the dialog.
      prevFocused?.focus?.();
    };
    // Depend only on `open`: onClose/role are read via refs so an unstable onClose
    // can't tear down and re-run the scroll-lock while the dialog stays open.
  }, [open]);

  return containerRef;
}
