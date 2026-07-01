/**
 * In-app alert dialog — replaces browser window.alert/confirm with a styled modal.
 *
 * Usage:
 *   1. Wrap your app with <AlertProvider>
 *   2. Import { showAlert } from '@/services/platform' — same API as before
 *
 * On native (iOS/Android), continues to use the native Alert.alert().
 * On web, renders a custom modal that matches the app's design.
 */

import React, { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';
import { View, Text, Pressable, Platform } from 'react-native';
import { color, createStyles, inter, radius, shadow, space, text } from '@/constants/theme';
import { useWebDialog } from '@/hooks/use-web-dialog';

interface AlertButton {
  text?: string;
  onPress?: () => void;
  style?: string;
}

interface AlertState {
  visible: boolean;
  title: string;
  message?: string;
  buttons: AlertButton[];
}

type ShowAlertFn = (title: string, message?: string, buttons?: AlertButton[]) => void;

const AlertContext = createContext<ShowAlertFn>(() => {});

/** Global ref so platform.ts can call it without React hooks */
let _globalShowAlert: ShowAlertFn | null = null;

export function getGlobalShowAlert(): ShowAlertFn | null {
  return _globalShowAlert;
}

export function AlertProvider({ children }: { children: React.ReactNode }) {
  const [alert, setAlert] = useState<AlertState>({ visible: false, title: '', buttons: [] });

  const show: ShowAlertFn = useCallback((title, message, buttons) => {
    setAlert({
      visible: true,
      title,
      message,
      buttons: buttons ?? [{ text: 'OK' }],
    });
  }, []);

  // Register global ref
  React.useEffect(() => {
    _globalShowAlert = show;
    return () => { _globalShowAlert = null; };
  }, [show]);

  const dismiss = useCallback((btn?: AlertButton) => {
    setAlert(prev => ({ ...prev, visible: false }));
    btn?.onPress?.();
  }, []);

  // Escape-to-close, focus trap, focus restore + scroll lock (web only).
  const dialogRef = useWebDialog(alert.visible, () => dismiss(), 'alertdialog');

  // Create a persistent DOM container above all modals (z-index > AppModal's 99999)
  const portalRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (Platform.OS !== 'web' || typeof document === 'undefined') return;
    const el = document.createElement('div');
    el.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;z-index:9999999;pointer-events:none;';
    document.body.appendChild(el);
    portalRef.current = el;
    return () => { el.remove(); };
  }, []);

  // Toggle pointer-events when alert is visible
  useEffect(() => {
    if (portalRef.current) {
      portalRef.current.style.pointerEvents = alert.visible ? 'auto' : 'none';
    }
  }, [alert.visible]);

  const alertContent = Platform.OS === 'web' && alert.visible ? (
    <View style={styles.overlay}>
      <Pressable style={styles.backdrop} onPress={() => dismiss()} accessibilityLabel={alert.title} />
      <View ref={dialogRef} style={styles.card}>
        <Text style={styles.title} accessibilityRole="header">{alert.title}</Text>
        {alert.message ? <Text style={styles.message}>{alert.message}</Text> : null}
        <View style={styles.buttonRow}>
          {alert.buttons.map((btn, i) => {
            const isDestructive = btn.style === 'destructive';
            const isCancel = btn.style === 'cancel';
            const isPrimary = !isCancel && !isDestructive && alert.buttons.length > 1 && i === alert.buttons.length - 1;
            return (
              <Pressable
                key={i}
                accessibilityRole="button"
                accessibilityLabel={btn.text ?? 'OK'}
                style={[
                  styles.button,
                  isPrimary && styles.buttonPrimary,
                  isDestructive && styles.buttonDestructive,
                ]}
                onPress={() => dismiss(btn)}
              >
                <Text style={[
                  styles.buttonText,
                  isCancel && styles.buttonTextCancel,
                  isPrimary && styles.buttonTextPrimary,
                  isDestructive && styles.buttonTextDestructive,
                ]}>
                  {btn.text ?? 'OK'}
                </Text>
              </Pressable>
            );
          })}
        </View>
      </View>
    </View>
  ) : null;

  // Render alert via DOM portal so it's above all AppModals
  const portalRendered = Platform.OS === 'web' && portalRef.current && alertContent
    ? require('react-dom').createPortal(alertContent, portalRef.current)
    : null;

  return (
    <AlertContext.Provider value={show}>
      {children}
      {portalRendered}
    </AlertContext.Provider>
  );
}

export function useAppAlert(): ShowAlertFn {
  return useContext(AlertContext);
}

const styles = createStyles(() => ({
  overlay: {
    position: 'absolute' as const,
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    justifyContent: 'center',
    alignItems: 'center',
    zIndex: 999999,
  },
  backdrop: {
    position: 'absolute' as const,
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(0,0,0,0.35)',
  },
  card: {
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    padding: space['2xl'],
    maxWidth: 340,
    width: '85%',
    ...shadow.lg,
  },
  title: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.md,
  },
  message: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    lineHeight: 22,
    marginBottom: space.xl,
  },
  buttonRow: {
    flexDirection: 'row',
    gap: space.md,
    justifyContent: 'flex-end',
  },
  button: {
    paddingVertical: space.md,
    paddingHorizontal: space.xl,
    borderRadius: radius.lg,
    minWidth: 70,
    alignItems: 'center',
  },
  buttonPrimary: {
    backgroundColor: color.accent.base,
  },
  buttonDestructive: {
    backgroundColor: color.error.base,
  },
  buttonText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
  buttonTextCancel: {
    color: color.fg.subtle,
  },
  buttonTextPrimary: {
    color: color.fg.inverse,
  },
  buttonTextDestructive: {
    color: color.fg.inverse,
  },
}));
