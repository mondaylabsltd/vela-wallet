/**
 * Cross-platform utilities for APIs that differ between native and web.
 *
 * Each helper gracefully degrades on web so callers don't need
 * Platform.OS checks scattered throughout the codebase.
 */

import { Platform } from 'react-native';

// ---------------------------------------------------------------------------
// Alert
// ---------------------------------------------------------------------------

type AlertButton = { text?: string; onPress?: () => void; style?: string };

/**
 * Show an alert dialog. Uses native Alert on iOS/Android, styled in-app modal on web.
 */
export function showAlert(
  title: string,
  message?: string,
  buttons?: AlertButton[],
): void {
  if (Platform.OS === 'web') {
    // Use in-app styled alert if AlertProvider is mounted
    const { getGlobalShowAlert } = require('@/components/ui/AppAlert');
    const globalShow = getGlobalShowAlert();
    if (globalShow) {
      globalShow(title, message, buttons);
      return;
    }
    // Fallback: browser alert (AlertProvider not yet mounted)
    if (buttons && buttons.length > 1) {
      const ok = window.confirm(`${title}${message ? '\n\n' + message : ''}`);
      if (ok) {
        const action = buttons.find(b => b.style !== 'cancel') ?? buttons[buttons.length - 1];
        action?.onPress?.();
      } else {
        const cancel = buttons.find(b => b.style === 'cancel');
        cancel?.onPress?.();
      }
    } else {
      window.alert(`${title}${message ? '\n\n' + message : ''}`);
      buttons?.[0]?.onPress?.();
    }
  } else {
    const { Alert } = require('react-native');
    Alert.alert(title, message, buttons);
  }
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

/**
 * Copy text to clipboard. Uses expo-clipboard on native, navigator.clipboard on web.
 */
export async function copyToClipboard(text: string): Promise<void> {
  if (Platform.OS === 'web') {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Fallback for older browsers / insecure contexts
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
    }
  } else {
    const Clipboard = await import('expo-clipboard');
    await Clipboard.setStringAsync(text);
  }
}

// ---------------------------------------------------------------------------
// Haptics
// ---------------------------------------------------------------------------

/**
 * Trigger a success notification haptic. No-op on web.
 */
export function hapticSuccess(): void {
  if (Platform.OS === 'web') return;
  import('expo-haptics').then(H =>
    H.notificationAsync(H.NotificationFeedbackType.Success),
  ).catch(() => {});
}

/**
 * Trigger a light impact haptic. No-op on web.
 */
export function hapticLight(): void {
  if (Platform.OS === 'web') return;
  import('expo-haptics').then(H =>
    H.impactAsync(H.ImpactFeedbackStyle.Light),
  ).catch(() => {});
}

/**
 * Trigger an error notification haptic (failed tx, rejected sign, invalid input).
 * Distinct from success so failure *feels* different. No-op on web.
 */
export function hapticError(): void {
  if (Platform.OS === 'web') return;
  import('expo-haptics').then(H =>
    H.notificationAsync(H.NotificationFeedbackType.Error),
  ).catch(() => {});
}

/**
 * Trigger a warning notification haptic (blocking validation, "are you sure").
 * No-op on web.
 */
export function hapticWarning(): void {
  if (Platform.OS === 'web') return;
  import('expo-haptics').then(H =>
    H.notificationAsync(H.NotificationFeedbackType.Warning),
  ).catch(() => {});
}

/**
 * Trigger a selection-change haptic (segmented toggle, picker, tier switch).
 * Lighter than an impact — the iOS "tick" feel. No-op on web.
 */
export function hapticSelection(): void {
  if (Platform.OS === 'web') return;
  import('expo-haptics').then(H =>
    H.selectionAsync(),
  ).catch(() => {});
}

// ---------------------------------------------------------------------------
// App visibility (replaces AppState)
// ---------------------------------------------------------------------------

/**
 * Returns true if the app / tab is currently in the foreground.
 * Uses AppState on native, document.visibilityState on web.
 */
export function isAppActive(): boolean {
  if (Platform.OS === 'web') {
    return typeof document !== 'undefined' && document.visibilityState === 'visible';
  }
  const { AppState } = require('react-native');
  return AppState.currentState === 'active';
}

// ---------------------------------------------------------------------------
// Open URL (replaces Linking)
// ---------------------------------------------------------------------------

/**
 * Open a URL. Uses window.open on web, Linking.openURL on native.
 */
export function openURL(url: string): void {
  if (Platform.OS === 'web') {
    window.open(url, '_blank', 'noopener,noreferrer');
  } else {
    const { Linking } = require('react-native');
    Linking.openURL(url);
  }
}

// ---------------------------------------------------------------------------
// Open browser (replaces expo-web-browser)
// ---------------------------------------------------------------------------

/**
 * Open a URL in an in-app browser — the page stays inside Vela, it does not
 * kick the user out to the system browser app.
 *
 * - iOS: SFSafariViewController presented as a PAGE_SHEET card (rounded top,
 *   swipe-down to dismiss, the previous screen peeking behind) — the
 *   "opens inside the app" feel of Telegram / Slack.
 * - Android: a Chrome Custom Tab, themed to the app, that returns to Vela on
 *   Back — also in-app, not a separate browser app.
 * - Web: a new tab. Third-party pages (block explorers, GitHub, docs) send
 *   X-Frame-Options / CSP frame-ancestors and refuse to be iframed, so an
 *   in-app iframe would render blank for exactly the links we open here. A new
 *   tab is the correct web idiom (Telegram / Slack web behave the same).
 *
 * The chrome colors read the live theme tokens, so the browser follows the
 * active light/dark mode. Use this for viewing web content; use openURL() for
 * deep links meant to LEAVE the app (ethereum: / wallet: URIs, mailto:, tel:).
 */
export async function openBrowser(url: string): Promise<void> {
  if (Platform.OS === 'web') {
    window.open(url, '_blank', 'noopener,noreferrer');
    return;
  }
  const WebBrowser = await import('expo-web-browser');
  // Read live tokens (mutated on theme change) so the browser matches light/dark.
  const { color } = require('@/constants/theme');
  await WebBrowser.openBrowserAsync(url, {
    // iOS: card presentation — the Telegram-style in-app sheet.
    presentationStyle: WebBrowser.WebBrowserPresentationStyle.PAGE_SHEET,
    dismissButtonStyle: 'close',
    // Theme-aware chrome: iOS bar tint + control tint, Android toolbar color.
    toolbarColor: color.bg.base,
    controlsColor: color.accent.base,
    // Collapse the bar on scroll for more content room.
    enableBarCollapsing: true,
    // Android niceties.
    showTitle: true,
    enableDefaultShareMenuItem: true,
  });
}
