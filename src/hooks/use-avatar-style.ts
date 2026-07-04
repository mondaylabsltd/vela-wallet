/**
 * useAvatarStyle — the chosen avatar style, live. Backed by the avatar-style
 * service's listener set, so switching in Settings repaints every avatar on
 * screen immediately (Home header, account switcher, contacts, previews).
 */
import { useSyncExternalStore } from 'react';
import { getAvatarStyle, subscribeAvatarStyle, type AvatarStyle } from '@/services/avatar-style';

export function useAvatarStyle(): AvatarStyle {
  return useSyncExternalStore(subscribeAvatarStyle, getAvatarStyle, getAvatarStyle);
}
